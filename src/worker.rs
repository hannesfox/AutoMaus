//! Hintergrund-Thread, der einen Workflow ausführt (pyautogui-Stil).

use crate::actions::{FindMode, Action};
use crate::imgmatch::{self, Rgb};
use crate::workflow::Workflow;
use eframe::egui;
use enigo::{Axis, Coordinate, Direction, Enigo, Keyboard, Mouse, Settings};
use screenshots::Screen;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc,
};
use std::thread;
use std::time::{Duration, Instant};

const STOPPED: &str = "Von Benutzer gestoppt.";
const FAILSAFE: &str = "Failsafe ausgelöst: Maus in der Bildschirmecke.";

pub enum WorkerMsg {
    Status(String),
    StepStarted(usize),
    Finished(bool, Option<String>),
}

pub struct WorkerState {
    pub should_stop: Arc<AtomicBool>,
    pub rx: mpsc::Receiver<WorkerMsg>,
}

/// Begrenzungen des Haupt-Bildschirms (für die Failsafe-Ecken).
#[derive(Clone, Copy)]
pub struct ScreenBounds {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[cfg(target_os = "macos")]
pub fn screen_bounds() -> ScreenBounds {
    use core_graphics::display::CGDisplay;
    let b = CGDisplay::main().bounds();
    ScreenBounds {
        left: b.origin.x as i32,
        top: b.origin.y as i32,
        right: (b.origin.x + b.size.width) as i32,
        bottom: (b.origin.y + b.size.height) as i32,
    }
}

#[cfg(not(target_os = "macos"))]
pub fn screen_bounds() -> ScreenBounds {
    ScreenBounds {
        left: 0,
        top: 0,
        right: i32::MAX,
        bottom: i32::MAX,
    }
}

fn failsafe_triggered(x: i32, y: i32, b: ScreenBounds) -> bool {
    const M: i32 = 10;
    (x <= b.left + M && y <= b.top + M)
        || (x >= b.right - M && y <= b.top + M)
        || (x <= b.left + M && y >= b.bottom - M)
        || (x >= b.right - M && y >= b.bottom - M)
}

/// Stopptaste/Failsafe prüfen – `Err` bedeutet Abbruch mit dieser Meldung.
fn check_abort(stop: &AtomicBool, bounds: ScreenBounds, enigo: &Enigo) -> Result<(), String> {
    if stop.load(Ordering::Relaxed) {
        return Err(STOPPED.to_string());
    }
    if let Ok((mx, my)) = enigo.location() {
        if failsafe_triggered(mx, my, bounds) {
            return Err(FAILSAFE.to_string());
        }
    }
    Ok(())
}

/// Bewegt die Maus – bei `duration > 0` weich animiert (wie `moveTo(duration=…)`).
fn move_smooth(
    enigo: &mut Enigo,
    from: (i32, i32),
    to: (i32, i32),
    duration: f64,
    stop: &AtomicBool,
    bounds: ScreenBounds,
) -> Result<(), String> {
    if duration <= 0.0 || from == to {
        enigo
            .move_mouse(to.0, to.1, Coordinate::Abs)
            .map_err(|e| format!("Mausbewegung fehlgeschlagen: {e:?}"))?;
        return Ok(());
    }
    let dx = (to.0 - from.0) as f64;
    let dy = (to.1 - from.1) as f64;
    let dist = (dx * dx + dy * dy).sqrt();
    let steps = ((dist / 6.0).ceil() as u64).clamp(1, 240);
    let step_time = Duration::from_secs_f64(duration / steps as f64);
    for i in 1..=steps {
        check_abort(stop, bounds, enigo)?;
        let t = i as f64 / steps as f64;
        let x = (from.0 as f64 + dx * t).round() as i32;
        let y = (from.1 as f64 + dy * t).round() as i32;
        enigo
            .move_mouse(x, y, Coordinate::Abs)
            .map_err(|e| format!("Mausbewegung fehlgeschlagen: {e:?}"))?;
        thread::sleep(step_time);
    }
    Ok(())
}

pub fn run_workflow(
    workflow: Workflow,
    should_stop: Arc<AtomicBool>,
    tx: mpsc::Sender<WorkerMsg>,
    ctx: egui::Context,
    bounds: ScreenBounds,
) {
    thread::spawn(move || {
        let send_msg = |msg: WorkerMsg| {
            let _ = tx.send(msg);
            ctx.request_repaint();
        };

        let start_delay = workflow.start_delay.max(0.0);
        send_msg(WorkerMsg::Status(format!(
            "Startet in {} s …",
            crate::actions::secs(start_delay)
        )));
        thread::sleep(Duration::from_secs_f64(start_delay));

        let mut enigo = match Enigo::new(&Settings::default()) {
            Ok(e) => e,
            Err(err) => {
                send_msg(WorkerMsg::Finished(
                    false,
                    Some(format!(
                        "Enigo-Fehler: {err:?} – macOS-Zugriff erlaubt? \
                         (Systemeinstellungen → Datenschutz → Bedienungshilfen)"
                    )),
                ));
                return;
            }
        };

        let step_delay = workflow.step_delay.max(0.0);
        let mut pc = 0usize;
        let mut loop_stack: Vec<(usize, u32, u32)> = Vec::new();

        while pc < workflow.steps.len() {
            if let Err(e) = check_abort(&should_stop, bounds, &enigo) {
                send_msg(WorkerMsg::Finished(false, Some(e)));
                return;
            }

            send_msg(WorkerMsg::StepStarted(pc));
            let step = &workflow.steps[pc];

            let result: Result<(), String> = (|| match step {
                Action::Click {
                    x,
                    y,
                    button,
                    clicks,
                    interval_sec,
                } => {
                    let cur = enigo.location().unwrap_or((*x, *y));
                    move_smooth(&mut enigo, cur, (*x, *y), 0.0, &should_stop, bounds)?;
                    thread::sleep(Duration::from_millis(50));
                    let n = (*clicks).max(1);
                    for i in 0..n {
                        enigo
                            .button(button.to_enigo(), Direction::Click)
                            .map_err(|e| format!("Klick fehlgeschlagen: {e:?}"))?;
                        if i + 1 < n {
                            thread::sleep(Duration::from_secs_f64(interval_sec.max(0.0)));
                        }
                    }
                    Ok(())
                }
                Action::MoveTo {
                    x,
                    y,
                    duration_sec,
                } => {
                    let cur = enigo.location().unwrap_or((*x, *y));
                    move_smooth(
                        &mut enigo,
                        cur,
                        (*x, *y),
                        *duration_sec,
                        &should_stop,
                        bounds,
                    )
                }
                Action::MoveRelative {
                    dx,
                    dy,
                    duration_sec,
                } => {
                    let cur = enigo
                        .location()
                        .map_err(|e| format!("Mausposition nicht lesbar: {e:?}"))?;
                    move_smooth(
                        &mut enigo,
                        cur,
                        (cur.0 + dx, cur.1 + dy),
                        *duration_sec,
                        &should_stop,
                        bounds,
                    )
                }
                Action::Drag {
                    from_x,
                    from_y,
                    to_x,
                    to_y,
                    button,
                    duration_sec,
                } => {
                    let cur = enigo.location().unwrap_or((*from_x, *from_y));
                    move_smooth(
                        &mut enigo,
                        cur,
                        (*from_x, *from_y),
                        0.0,
                        &should_stop,
                        bounds,
                    )?;
                    thread::sleep(Duration::from_millis(50));
                    enigo
                        .button(button.to_enigo(), Direction::Press)
                        .map_err(|e| format!("Ziehen starten fehlgeschlagen: {e:?}"))?;
                    thread::sleep(Duration::from_millis(30));
                    move_smooth(
                        &mut enigo,
                        (*from_x, *from_y),
                        (*to_x, *to_y),
                        *duration_sec,
                        &should_stop,
                        bounds,
                    )?;
                    thread::sleep(Duration::from_millis(50));
                    enigo
                        .button(button.to_enigo(), Direction::Release)
                        .map_err(|e| format!("Ziehen beenden fehlgeschlagen: {e:?}"))?;
                    Ok(())
                }
                Action::Typewrite {
                    text,
                    interval_sec,
                } => {
                    if *interval_sec > 0.0 {
                        for c in text.chars() {
                            check_abort(&should_stop, bounds, &enigo)?;
                            enigo
                                .text(&c.to_string())
                                .map_err(|e| format!("Tippen fehlgeschlagen: {e:?}"))?;
                            thread::sleep(Duration::from_secs_f64(*interval_sec));
                        }
                        Ok(())
                    } else {
                        enigo
                            .text(text)
                            .map_err(|e| format!("Tippen fehlgeschlagen: {e:?}"))
                    }
                }
                Action::KeyPress { key } => {
                    check_abort(&should_stop, bounds, &enigo)?;
                    enigo
                        .key(key.to_enigo(), Direction::Click)
                        .map_err(|e| format!("Taste drücken fehlgeschlagen: {e:?}"))
                }
                Action::Hotkey { keys } => {
                    check_abort(&should_stop, bounds, &enigo)?;
                    if keys.is_empty() {
                        return Ok(());
                    }
                    let (last, mods) = keys.split_last().unwrap();
                    for m in mods {
                        enigo
                            .key(m.to_enigo(), Direction::Press)
                            .map_err(|e| format!("Taste drücken fehlgeschlagen: {e:?}"))?;
                    }
                    enigo
                        .key(last.to_enigo(), Direction::Click)
                        .map_err(|e| format!("Taste drücken fehlgeschlagen: {e:?}"))?;
                    for m in mods.iter().rev() {
                        enigo
                            .key(m.to_enigo(), Direction::Release)
                            .map_err(|e| format!("Taste loslassen fehlgeschlagen: {e:?}"))?;
                    }
                    Ok(())
                }
                Action::Scroll { amount } => enigo
                    .scroll(*amount, Axis::Vertical)
                    .map_err(|e| format!("Scrollen fehlgeschlagen: {e:?}")),
                Action::Sleep { duration_sec } => {
                    thread::sleep(Duration::from_secs_f64(duration_sec.max(0.0)));
                    Ok(())
                }
                Action::FindImage {
                    x1,
                    y1,
                    x2,
                    y2,
                    image,
                    mode,
                    timeout_sec,
                    duration_sec,
                } => {
                    if image.is_empty() {
                        return Err(
                            "Kein Bild hinterlegt – erst einen Bereich aufnehmen.".to_string()
                        );
                    }
                    let tmpl = Rgb::load(std::path::Path::new(image))?;
                    let (ax1, ay1, ax2, ay2) =
                        (x1.min(x2), y1.min(y2), x1.max(x2), y1.max(y2));
                    let center = ((ax1 + ax2) / 2, (ay1 + ay2) / 2);
                    let screen = Screen::from_point(center.0, center.1).map_err(|e| {
                        format!("Bildschirm nicht ermittelbar: {e}")
                    })?;
                    let di = screen.display_info;
                    let deadline = Instant::now() + Duration::from_secs_f64(timeout_sec.max(0.5));
                    let mut best_score = 0.0f32;

                    send_msg(WorkerMsg::Status("Suche Bild auf dem Bildschirm …".to_string()));

                    loop {
                        check_abort(&should_stop, bounds, &enigo)?;
                        let shot = screen.capture().map_err(|e| {
                            format!(
                                "Screenshot fehlgeschlagen: {e} – macOS: \
                                 Bildschirmaufzeichnung erlaubt?"
                            )
                        })?;
                        let (sw, sh) = (shot.width(), shot.height());
                        let scale = sw as f32 / di.width as f32;
                        let raw = shot.into_raw();
                        let srgb = Rgb::from_raws(sw as usize, sh as usize, &raw);
                        if let Some(m) = imgmatch::find(&srgb, &tmpl) {
                            best_score = m.score;
                            if m.score >= imgmatch::MIN_CONFIDENCE {
                                // Fundposition (physische Pixel) → globale Punkte
                                let fx = di.x
                                    + ((m.x + tmpl.w as usize / 2) as f32 / scale).round()
                                        as i32;
                                let fy = di.y
                                    + ((m.y + tmpl.h as usize / 2) as f32 / scale).round()
                                        as i32;
                                let cur = enigo.location().unwrap_or((fx, fy));
                                move_smooth(
                                    &mut enigo,
                                    cur,
                                    (fx, fy),
                                    *duration_sec,
                                    &should_stop,
                                    bounds,
                                )?;
                                match mode {
                                    FindMode::Click | FindMode::DoubleClick => {
                                        thread::sleep(Duration::from_millis(50));
                                        let clicks = if *mode == FindMode::DoubleClick {
                                            2
                                        } else {
                                            1
                                        };
                                        for i in 0..clicks {
                                            enigo
                                                .button(enigo::Button::Left, Direction::Click)
                                                .map_err(|e| {
                                                    format!("Klick fehlgeschlagen: {e:?}")
                                                })?;
                                            if i + 1 < clicks {
                                                thread::sleep(Duration::from_millis(80));
                                            }
                                        }
                                    }
                                    FindMode::Move => {}
                                }
                                return Ok(());
                            }
                        }
                        if Instant::now() >= deadline {
                            return Err(format!(
                                "Bild nicht gefunden: {} (beste Übereinstimmung {:.0} %)",
                                image,
                                best_score * 100.0
                            ));
                        }
                        thread::sleep(Duration::from_millis(350));
                    }
                }
                Action::LoopStart { count } => {
                    loop_stack.push((pc, *count, 1));
                    send_msg(WorkerMsg::Status(if *count == 0 {
                        "Schleife (endlos)".to_string()
                    } else {
                        format!("Schleife 1/{count}")
                    }));
                    Ok(())
                }
                Action::LoopEnd => {
                    if let Some(mut frame) = loop_stack.pop() {
                        frame.2 += 1;
                        if frame.1 == 0 || frame.2 <= frame.1 {
                            let (start, target, cur) = frame;
                            send_msg(WorkerMsg::Status(if target == 0 {
                                format!("Schleife {cur}× …")
                            } else {
                                format!("Schleife {cur}/{target}")
                            }));
                            loop_stack.push((start, target, cur));
                            pc = start;
                        }
                    }
                    Ok(())
                }
            })();

            if let Err(e) = result {
                send_msg(WorkerMsg::Finished(false, Some(e)));
                return;
            }

            // Pause zwischen den Aktionen (nach einer Pause selbstverständlich nicht).
            if !matches!(workflow.steps[pc], Action::Sleep { .. }) {
                thread::sleep(Duration::from_secs_f64(step_delay));
            }
            pc += 1;
        }

        send_msg(WorkerMsg::Finished(true, None));
    });
}

