//! Einfache Bildsuche (Template Matching) – nach dem Vorbild von
//! `pyautogui.locateOnScreen`. Sucht ein kleines Referenzbild in einem
//! Screenshot und liefert die Position mit Konfidenz zurück.
//!
//! Vorgehen:
//! 1. **Grobsuche** auf heruntergerechneten Graustufen (schnell) – dabei werden
//!    die Top-K Kandidaten behalten, nicht nur der beste Treffer. Das ist
//!    wichtig, wenn es sehr ähnliche Elemente gibt (z. B. zwei gleiche
//!    Ordner-Icons mit unterschiedlicher Beschriftung).
//! 2. **Feinsuche** jedes Kandidaten in voller Auflösung (Graustufen-SAD).
//! 3. **Konfidenz** = Anteil der Pixel, die im RGB-Vergleich innerhalb einer
//!    Toleranz farbgleich sind. Das ist deutlich strikter als ein
//!    durchschnittlicher Grauwert-Unterschied und verwirft „fast gleich“-
//!    Doppelgänger zuverlässig.

use image::RgbaImage;
use std::path::Path;

/// Minimale Übereinstimmung, damit ein Fund akzeptiert wird (0.0–1.0).
pub const MIN_CONFIDENCE: f32 = 0.90;

/// Maximal erlaubter Kanal-Unterschied pro Pixel, um als „gleich“ zu zählen.
const TOLERANCE: i32 = 24;

/// Wie viele Grob-Kandidaten verfeinert werden.
const TOP_K: usize = 12;

// ---------------------------------------------------------------------------
// Datenstrukturen
// ---------------------------------------------------------------------------

pub struct Gray {
    pub w: usize,
    pub h: usize,
    pub data: Vec<u8>,
}

/// RGB-Daten (3 Bytes pro Pixel) – für den strikten Farbabgleich.
pub struct Rgb {
    pub w: usize,
    pub h: usize,
    pub rgb: Vec<u8>,
}

impl Gray {
    /// Box-Downsampling um Faktor `f` (für die Grobsuche).
    pub fn downsample(&self, f: usize) -> Gray {
        let w = self.w / f;
        let h = self.h / f;
        let mut data = vec![0u8; w * h];
        let area = (f * f) as u32;
        for y in 0..h {
            for x in 0..w {
                let mut sum = 0u32;
                for dy in 0..f {
                    let row = (y * f + dy) * self.w + x * f;
                    for dx in 0..f {
                        sum += self.data[row + dx] as u32;
                    }
                }
                data[y * w + x] = (sum / area) as u8;
            }
        }
        Gray { w, h, data }
    }

    /// Summe der absoluten Grauwert-Differenzen; bricht früh ab, wenn `limit`
    /// überschritten ist (schnelles „nein“).
    fn sad_at(&self, other: &Gray, x: usize, y: usize, limit: u64) -> u64 {
        let mut sad = 0u64;
        for ty in 0..other.h {
            let srow = (y + ty) * self.w + x;
            let trow = ty * other.w;
            for tx in 0..other.w {
                let d = self.data[srow + tx] as i32 - other.data[trow + tx] as i32;
                sad += d.unsigned_abs() as u64;
                if sad > limit {
                    return sad;
                }
            }
        }
        sad
    }
}

impl Rgb {
    /// Aus rohen RGBA-Bytes (z. B. direkt aus dem Screenshot).
    pub fn from_raws(w: usize, h: usize, rgba: &[u8]) -> Rgb {
        let mut rgb = Vec::with_capacity(w * h * 3);
        for px in rgba.chunks_exact(4) {
            rgb.extend_from_slice(&px[..3]);
        }
        Rgb { w, h, rgb }
    }

    pub fn from_rgba(img: &RgbaImage) -> Rgb {
        let (w, h) = (img.width() as usize, img.height() as usize);
        let mut rgba = Vec::with_capacity(w * h * 4);
        for p in img.pixels() {
            rgba.extend_from_slice(&[p[0], p[1], p[2], 255]);
        }
        Rgb::from_raws(w, h, &rgba)
    }

    /// Lädt ein PNG (oder anderes Bildformat) als Farb-Template.
    pub fn load(path: &Path) -> Result<Rgb, String> {
        let img = image::open(path)
            .map_err(|e| format!("Bild konnte nicht geladen werden ({}): {e}", path.display()))?;
        Ok(Rgb::from_rgba(&img.to_rgba8()))
    }

    /// Luminanz-Graustufen (für die schnelle Positionsuche).
    pub fn gray(&self) -> Gray {
        let data = self
            .rgb
            .chunks_exact(3)
            .map(|p| ((p[0] as u32 * 299 + p[1] as u32 * 587 + p[2] as u32 * 114) / 1000) as u8)
            .collect();
        Gray {
            w: self.w,
            h: self.h,
            data,
        }
    }

    /// Anteil der Pixel (0.0–1.0), die an Position (x, y) farblich innerhalb
    /// der Toleranz mit dem Template übereinstimmen.
    fn inlier_fraction(&self, tmpl: &Rgb, x: usize, y: usize) -> f32 {
        let n = tmpl.w * tmpl.h;
        let mut inliers = 0u64;
        for ty in 0..tmpl.h {
            let srow = ((y + ty) * self.w + x) * 3;
            let trow = ty * tmpl.w * 3;
            for tx in 0..tmpl.w {
                let s = &self.rgb[srow + tx * 3..srow + tx * 3 + 3];
                let t = &tmpl.rgb[trow + tx * 3..trow + tx * 3 + 3];
                let dm = (s[0] as i32 - t[0] as i32)
                    .abs()
                    .max((s[1] as i32 - t[1] as i32).abs())
                    .max((s[2] as i32 - t[2] as i32).abs());
                if dm <= TOLERANCE {
                    inliers += 1;
                }
            }
        }
        inliers as f32 / n as f32
    }
}

// ---------------------------------------------------------------------------
// Suche
// ---------------------------------------------------------------------------

pub struct MatchResult {
    /// Position der linken oberen Ecke des Templates (in Pixeln des Screenshots).
    pub x: usize,
    pub y: usize,
    /// Übereinstimmung von 0.0 (kein) bis 1.0 (exakt).
    pub score: f32,
}

/// Durchsucht `screen` nach `tmpl` (Grobsuche + Verfeinerung + Farbabgleich).
pub fn find(screen: &Rgb, tmpl: &Rgb) -> Option<MatchResult> {
    if tmpl.w == 0
        || tmpl.h == 0
        || tmpl.w > screen.w
        || tmpl.h > screen.h
        || screen.w == 0
        || screen.h == 0
    {
        return None;
    }

    let screen_gray = screen.gray();
    let tmpl_gray = tmpl.gray();

    // ---- 1. Grobsuche: Top-K Kandidaten auf heruntergerechneten Graustufen
    let f = (tmpl.w.min(tmpl.h) / 10).clamp(1, 8);
    let s = screen_gray.downsample(f);
    let t = tmpl_gray.downsample(f);
    if t.w > s.w || t.h > s.h {
        return None;
    }

    // Korrekte Grobsuche (Kandidaten mit laufender Top-K-Begrenzung)
    let mut cands: Vec<(u64, usize, usize)> = Vec::with_capacity(TOP_K + 1);
    let mut limit = u64::MAX;
    for y in 0..=(s.h - t.h) {
        for x in 0..=(s.w - t.w) {
            let sad = s.sad_at(&t, x, y, limit);
            if cands.len() < TOP_K || sad < limit {
                cands.push((sad, x, y));
                cands.sort_by_key(|c| c.0);
                if cands.len() > TOP_K {
                    cands.pop();
                }
                limit = cands.last().map(|c| c.0).unwrap_or(u64::MAX);
            }
        }
    }

    // ---- 2+3. Jeden Kandidaten verfeinern und per Farbe bewerten
    let mut best: Option<(usize, usize, f32)> = None;
    for (_, cx, cy) in cands {
        let bx = cx * f;
        let by = cy * f;
        let x0 = bx.saturating_sub(f);
        let y0 = by.saturating_sub(f);
        let x1 = (bx + f + 1).min(screen.w - tmpl.w);
        let y1 = (by + f + 1).min(screen.h - tmpl.h);

        let mut bs = (x0, y0, u64::MAX);
        for y in y0..=y1 {
            for x in x0..=x1 {
                let sad = screen_gray.sad_at(&tmpl_gray, x, y, bs.2);
                if sad < bs.2 {
                    bs = (x, y, sad);
                }
            }
        }
        let conf = screen.inlier_fraction(tmpl, bs.0, bs.1);
        if best.map(|b| conf > b.2).unwrap_or(true) {
            best = Some((bs.0, bs.1, conf));
        }
    }

    best.map(|(x, y, score)| MatchResult { x, y, score })
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    /// Screenshot mit eindeutigem Muster-Baustein – Template muss genau dort gefunden werden.
    #[test]
    fn finds_template_position() {
        let mut screen = RgbaImage::new(300, 200);
        for (x, y, p) in screen.enumerate_pixels_mut() {
            let v = ((x + y) % 200) as u8;
            *p = Rgba([v, v, v, 255]);
        }
        let mut tmpl = RgbaImage::new(60, 40);
        for (x, y, p) in tmpl.enumerate_pixels_mut() {
            *p = Rgba([((x * 7 + y * 13) % 256) as u8, 0, ((x * 3) % 256) as u8, 255]);
        }
        for (x, y, p) in tmpl.enumerate_pixels() {
            screen.put_pixel(50 + x, 30 + y, *p);
        }

        let s = Rgb::from_rgba(&screen);
        let t = Rgb::from_rgba(&tmpl);
        let m = find(&s, &t).expect("Template muss gefunden werden");
        assert_eq!((m.x, m.y), (50, 30));
        assert!(m.score > 0.95, "score={}", m.score);
    }

    #[test]
    fn no_match_for_random_noise() {
        // deterministisches Pseudo-Rauschen
        let mut state = 12345u32;
        let mut rnd = || {
            state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            (state >> 24) as u8
        };
        let mut screen = RgbaImage::new(200, 200);
        for (_, _, p) in screen.enumerate_pixels_mut() {
            *p = Rgba([rnd(), rnd(), rnd(), 255]);
        }
        let mut tmpl = RgbaImage::new(30, 30);
        for (_, _, p) in tmpl.enumerate_pixels_mut() {
            *p = Rgba([rnd(), rnd(), rnd(), 255]);
        }
        let s = Rgb::from_rgba(&screen);
        let t = Rgb::from_rgba(&tmpl);
        let m = find(&s, &t).unwrap();
        assert!(
            m.score < MIN_CONFIDENCE,
            "Rauschen darf nicht als Fund durchgehen: score={}",
            m.score
        );
    }

    #[test]
    fn template_larger_than_screen_is_none() {
        let screen = Rgb { w: 10, h: 10, rgb: vec![0; 10 * 10 * 3] };
        let tmpl = Rgb { w: 20, h: 5, rgb: vec![0; 20 * 5 * 3] };
        assert!(find(&screen, &tmpl).is_none());
    }

    /// Zwei fast gleiche Bereiche (gleiche obere Hälfte, veränderte untere
    /// Viertel) – die exakte Stelle muss gewinnen, nicht der Doppelgänger.
    #[test]
    fn picks_exact_match_over_lookalike() {        let mut screen = RgbaImage::new(400, 300);
        for (x, y, p) in screen.enumerate_pixels_mut() {
            let v = ((x + y) % 200) as u8;
            *p = Rgba([v, v, v, 255]);
        }
        let mut tmpl = RgbaImage::new(80, 60);
        for (x, y, p) in tmpl.enumerate_pixels_mut() {
            *p = Rgba([((x * 7 + y * 13) % 256) as u8, ((x * 3) % 256) as u8, 128, 255]);
        }
        // Exakte Kopie bei (50, 30)
        for (x, y, p) in tmpl.enumerate_pixels() {
            screen.put_pixel(50 + x, 30 + y, *p);
        }
        // Doppelgänger bei (220, 150): obere 3/4 identisch, unteres Viertel
        // nur leicht verschoben (wie leicht unterschiedliche Beschriftung)
        for (x, y, p) in tmpl.enumerate_pixels() {
            let mut q = *p;
            if y >= 45 {
                q = Rgba([q[0].saturating_add(30), q[1], q[2], 255]);
            }
            screen.put_pixel(220 + x, 150 + y, q);
        }

        let s = Rgb::from_rgba(&screen);
        let t = Rgb::from_rgba(&tmpl);
        let m = find(&s, &t).expect("Muss gefunden werden");
        assert_eq!((m.x, m.y), (50, 30), "Doppelgänger darf nicht gewinnen");
        assert!(m.score >= MIN_CONFIDENCE, "score={}", m.score);
    }

    /// Geschwindigkeit bei realistischer Bildschirmgröße (nur im Release-Build
    /// garantiert – im Debug-Build ist alles ~20× langsamer).
    #[test]
    fn search_speed_full_hd() {
        use std::time::Instant;
        let mut screen = RgbaImage::new(1920, 1080);
        for (x, y, p) in screen.enumerate_pixels_mut() {
            let v = ((x * 13 + y * 7) % 200) as u8;
            *p = Rgba([v, v, v, 255]);
        }
        let mut tmpl = RgbaImage::new(160, 120);
        for (x, y, p) in tmpl.enumerate_pixels_mut() {
            *p = Rgba([((x * 7 + y * 13) % 256) as u8, ((x * 3) % 256) as u8, 128, 255]);
        }
        for (x, y, p) in tmpl.enumerate_pixels() {
            screen.put_pixel(1700 + x, 900 + y, *p);
        }
        let s = Rgb::from_rgba(&screen);
        let t = Rgb::from_rgba(&tmpl);
        let start = Instant::now();
        let m = find(&s, &t).expect("Muss gefunden werden");
        let dt = start.elapsed();
        eprintln!("SPEED: find() auf 1920×1080: {dt:?} bei ({}, {}), score={}", m.x, m.y, m.score);
        assert_eq!((m.x, m.y), (1700, 900));
        if !cfg!(debug_assertions) {
            assert!(dt.as_millis() < 1000, "Suche zu langsam: {dt:?}");
        }
    }
}
