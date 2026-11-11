//! Design: Farben, abgerundete Ecken, hell/dunkel.
//!
//! Farbwelt nach dem App-Logo: warmes Kohlschwarz/Anthrazit als Basis,
//! kräftiges Orange als Akzent.

use eframe::egui::{self, Color32, Rounding, Stroke, Vec2};

// ---------------------------------------------------------------------------
// Basispalette (aus dem Logo)
// ---------------------------------------------------------------------------

/// Kräftiges Orange aus dem Logo
pub const ACCENT: Color32 = Color32::from_rgb(244, 90, 34);
/// = unmultiplied(ACCENT, 80) – Werte hier bereits korrekt vormultipliziert
pub const ACCENT_SOFT: Color32 = Color32::from_rgba_premultiplied(76, 28, 11, 80);
/// Dunkler Modus: warmes Kohlschwarz
const DARK_BG: Color32 = Color32::from_rgb(30, 29, 27);
/// Dunkler Modus: Eingabefelder/Extremflächen (fast Logo-Hintergrund)
const DARK_BG_EXTREME: Color32 = Color32::from_rgb(23, 22, 21);
/// Dunkler Modus: dezente Karten-Füllung (Hover/Hover-Zeilen)
const DARK_BG_FAINT: Color32 = Color32::from_rgb(46, 44, 42);

/// Heller Modus: warmes Papier
const LIGHT_BG: Color32 = Color32::from_rgb(245, 242, 238);
/// Heller Modus: Flächen
const LIGHT_BG_PANEL: Color32 = Color32::from_rgb(240, 236, 231);
/// Heller Modus: Eingabefelder
const LIGHT_BG_EXTREME: Color32 = Color32::from_rgb(233, 229, 224);

pub const SUCCESS: Color32 = Color32::from_rgb(46, 168, 88);
/// = unmultiplied(SUCCESS, 65)
pub const SUCCESS_SOFT: Color32 = Color32::from_rgba_premultiplied(12, 43, 22, 65);
/// Rot klar vom Orange-Akzent getrennt (mehr Blauanteil, dunkler)
pub const DANGER: Color32 = Color32::from_rgb(226, 62, 62);
/// Ablege-Ziel: neutraler Grauton (bleibt klar vom orangen Akzent getrennt)
/// = unmultiplied(160, 160, 160, 90)
pub const TARGET_SOFT: Color32 = Color32::from_rgba_premultiplied(56, 56, 56, 90);

/// Gut lesbare Sekundärfarbe (Hinweise) je nach Hell-/Dunkelmodus.
pub fn muted(ui: &egui::Ui) -> Color32 {
    if ui.visuals().dark_mode {
        Color32::from_rgb(178, 174, 168)
    } else {
        Color32::from_rgb(100, 96, 92)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ThemeKind {
    Dark,
    Light,
}

impl ThemeKind {
    pub fn toggled(self) -> Self {
        match self {
            ThemeKind::Dark => ThemeKind::Light,
            ThemeKind::Light => ThemeKind::Dark,
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            ThemeKind::Dark => "☀ Hell",
            ThemeKind::Light => "🌙 Dunkel",
        }
    }
}

pub fn apply(ctx: &egui::Context, kind: ThemeKind) {
    let dark = kind == ThemeKind::Dark;
    let mut v = match kind {
        ThemeKind::Dark => egui::Visuals::dark(),
        ThemeKind::Light => egui::Visuals::light(),
    };

    // Flächen in der warmen Logo-Basis
    v.panel_fill = if dark { DARK_BG } else { LIGHT_BG };
    v.window_fill = if dark { DARK_BG } else { LIGHT_BG };
    v.extreme_bg_color = if dark { DARK_BG_EXTREME } else { LIGHT_BG_EXTREME };
    v.faint_bg_color = if dark { DARK_BG_FAINT } else { LIGHT_BG_PANEL };

    // Auswahl & aktive Widgets in Akzentfarbe
    v.selection.bg_fill = ACCENT;
    // Text von ausgewählten Labels in Weiß (sonst: orange Schrift auf orangem Grund)
    v.selection.stroke = Stroke::new(1.0_f32, Color32::WHITE);
    v.widgets.hovered.bg_fill = match kind {
        ThemeKind::Dark => Color32::from_rgba_unmultiplied(255, 255, 255, 22),
        ThemeKind::Light => Color32::from_rgba_unmultiplied(0, 0, 0, 20),
    };
    v.widgets.active.bg_fill = Color32::from_rgba_unmultiplied(244, 90, 34, 110);
    v.window_rounding = Rounding::same(10.0);
    ctx.set_visuals(v);

    ctx.style_mut(|s| {
        s.spacing.item_spacing = Vec2::new(8.0, 6.0);
        s.spacing.button_padding = Vec2::new(8.0, 4.0);
        s.spacing.interact_size.y = 26.0;
        s.visuals.widgets.inactive.rounding = Rounding::same(6.0);
        s.visuals.widgets.hovered.rounding = Rounding::same(6.0);
        s.visuals.widgets.active.rounding = Rounding::same(6.0);
        s.visuals.widgets.noninteractive.rounding = Rounding::same(6.0);
    });
}
