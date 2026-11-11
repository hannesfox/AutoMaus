//! App-Kern: Orchestrating von Panels, Shortcuts, Worker und Dialogen.

use crate::actions::Action;
use crate::state::{
    AppView, CaptureSlot, EditorState, FrameOutcome, InspectorTab, KeyCapture,
};
use crate::theme::{self, ThemeKind};
use crate::ui_dashboard;
use crate::ui_editor;
use crate::ui_modals::{self, UnsavedChoice};
use crate::workflow::{self, FileEntry, Workflow, WORKFLOW_DIR};
use crate::worker::{self, WorkerMsg, WorkerState};
use eframe::egui;
use enigo::{Enigo, Mouse, Settings};
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc,
};
use std::time::{Duration, Instant};

/// Logo als Wasserzeichen: zur Compile-Zeit eingebettet, beim ersten Frame
/// dekodiert und auf ~15 % Deckkraft reduziert.
fn logo_texture(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    let img = image::load_from_memory(include_bytes!("../automaus.png")).ok()?;
    let img = img.to_rgba8();
    let (w, h) = (img.width() as usize, img.height() as usize);
    // Alpha auf ~15 % skalieren (255 * 0.15 ≈ 38) – RGB-Werte bleiben unverändert,
    // da `from_rgba_unmultiplied` die Vormultiplikation selbst vornimmt.
    let pixels: Vec<u8> = img
        .pixels()
        .flat_map(|p| [p[0], p[1], p[2], (p[3] as u32 * 38 / 255) as u8])
        .collect();
    Some(ctx.load_texture(
        "automaus_logo",
        egui::ColorImage::from_rgba_unmultiplied([w, h], &pixels),
        egui::TextureOptions::LINEAR,
    ))
}

pub struct WorkflowApp {
    view: AppView,
    files: Vec<FileEntry>,
    worker: Option<WorkerState>,
    status_text: String,
    status_is_error: bool,
    capture_timer: Option<Instant>,
    capture_slot: CaptureSlot,
    /// Cache für die Bild-Vorschau: (Pfad, Textur)
    preview: Option<(String, egui::TextureHandle)>,
    /// Logo-Wasserzeichen fürs Dashboard (einmal dekodiert, gecacht)
    logo: Option<egui::TextureHandle>,
    /// Aktives »Bereich aufziehen« (Klicken & Ziehen) – falls vorhanden
    region_drag: Option<RegionDragState>,
    running_step: Option<usize>,
    run_total: usize,
    theme: ThemeKind,
    search: String,
    dirty: bool,
    confirm_delete: Option<PathBuf>,
    pending_back: bool,
    key_capture: Option<KeyCapture>,
}

/// Phase des Bereichs-Aufziehens (Klicken & Ziehen).
#[derive(PartialEq)]
enum DragPhase {
    /// Warten, bis der Benutzer die linke Maustaste drückt
    WaitPress,
    /// Taste gedrückt – Bereich wird aufgezogen
    Dragging,
}

/// Zustand für das Aufziehen eines Bild-Bereichs mit der Maus.
struct RegionDragState {
    step: usize,
    phase: DragPhase,
    /// Start der Aufnahme (kleine Schonfrist gegen den Button-Klick selbst)
    since: Instant,
    start: (i32, i32),
    cur: (i32, i32),
    enigo: Enigo,
}

/// Globale linke Maustaste gedrückt? (unabhängig, ob die App Fokus hat)
#[cfg(target_os = "macos")]
fn left_button_down() -> bool {
    // Boolean CGEventSourceButtonState(CGEventSourceStateID, CGMouseButton);
    // HIDSystemState = 1, linke Taste = 0.
    extern "C" {
        fn CGEventSourceButtonState(state_id: i32, button: u32) -> u8;
    }
    unsafe { CGEventSourceButtonState(1, 0) != 0 }
}

#[cfg(not(target_os = "macos"))]
fn left_button_down() -> bool {
    false
}

impl WorkflowApp {
    pub fn new(ctx: &egui::Context) -> Self {
        theme::apply(ctx, ThemeKind::Dark);
        Self {
            view: AppView::Dashboard,
            files: workflow::list_workflows(),
            worker: None,
            status_text: "Bereit".to_string(),
            status_is_error: false,
            capture_timer: None,
            capture_slot: CaptureSlot::Primary,
            preview: None,
            logo: None,
            region_drag: None,
            running_step: None,
            run_total: 0,
            theme: ThemeKind::Dark,
            search: String::new(),
            dirty: false,
            confirm_delete: None,
            pending_back: false,
            key_capture: None,
        }
    }

    // -----------------------------------------------------------------------
    // Hintergrund
    // -----------------------------------------------------------------------

    fn poll_capture(&mut self, ctx: &egui::Context) {
        let Some(target_time) = self.capture_timer else {
            return;
        };
        let now = Instant::now();

        if now < target_time {
            ctx.request_repaint();
            let remaining = (target_time - now).as_secs_f32();
            egui::Window::new("Maus-Erfassung")
                .title_bar(false)
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_TOP, [0.0, 40.0])
                .show(ctx, |ui| {
                    ui.add_space(10.0);
                    ui.heading(format!("⏳ Erfasse Position in {remaining:.1} s …"));
                    ui.label("Bewege den Mauszeiger jetzt an die gewünschte Stelle!");
                    ui.add_space(10.0);
                });
            return;
        }

        // Zeit abgelaufen → Position auslesen
        self.capture_timer = None;
        let slot = self.capture_slot;
        let mut msg = "Position konnte nicht gelesen werden.".to_string();
        let mut ok = false;

        if let Ok(enigo) = Enigo::new(&Settings::default()) {
            if let Ok((cx, cy)) = enigo.location() {
                if let AppView::Editor(ed) = &mut self.view {
                    if let Some(idx) = ed.selected.filter(|i| *i < ed.workflow.steps.len()) {
                        let applied = match &mut ed.workflow.steps[idx] {
                            Action::Click { x, y, .. } | Action::MoveTo { x, y, .. } => {
                                *x = cx;
                                *y = cy;
                                true
                            }
                            Action::Drag {
                                from_x,
                                from_y,
                                to_x,
                                to_y,
                                ..
                            } => match slot {
                                CaptureSlot::Primary => {
                                    *from_x = cx;
                                    *from_y = cy;
                                    true
                                }
                                CaptureSlot::Secondary => {
                                    *to_x = cx;
                                    *to_y = cy;
                                    true
                                }
                            },
                            _ => false,
                        };
                        if applied {
                            self.dirty = true;
                            ok = true;
                            msg = format!("📍 Position erfasst: ({cx}, {cy})");
                        }
                    }
                }
            }
        }
        self.status_text = msg;
        self.status_is_error = !ok;
    }

    /// Nimmt einen Bereich als PNG auf und legt ihn unter `workflows/images/` ab.
    /// Ohne `rect` wird der Bereich aus dem ausgewählten FindImage-Schritt gelesen,
    /// mit `rect` wird das Rechteck benutzt und in den Schritt zurückgeschrieben.
    fn capture_region_image(&mut self, ctx: &egui::Context, rect: Option<(i32, i32, i32, i32)>) {
        let message: String;
        let mut ok = false;

        'outer: {
            let AppView::Editor(ed) = &mut self.view else {
                message = "Kein Editor offen.".into();
                break 'outer;
            };
            let Some(idx) = ed.selected.filter(|i| *i < ed.workflow.steps.len()) else {
                message = "Kein Schritt ausgewählt.".into();
                break 'outer;
            };
            let (sel_idx, old_image) = match &ed.workflow.steps[idx] {
                Action::FindImage { x1, y1, x2, y2, image, .. } => {
                    ((*x1, *y1, *x2, *y2), image.clone())
                }
                _ => {
                    message = "Der ausgewählte Schritt ist kein »Bild finden«-Schritt.".into();
                    break 'outer;
                }
            };
            let (ax1, ay1, ax2, ay2) = match rect {
                Some(r) => r,
                None => sel_idx,
            };
            // Bei Aufziehen die Koordinaten in den Schritt zurückschreiben
            if rect.is_some() {
                if let Action::FindImage { x1, y1, x2, y2, .. } = &mut ed.workflow.steps[idx] {
                    *x1 = ax1;
                    *y1 = ay1;
                    *x2 = ax2;
                    *y2 = ay2;
                }
            }
            if (ax2 - ax1) < 8 || (ay2 - ay1) < 8 {
                message = "Der Bereich ist zu klein – mindestens 8×8 Pixel aufziehen.".into();
                break 'outer;
            }

            // Warnt, wenn das eigene Fenster den Bereich verdeckt.
            // Wichtig: `outer_rect` ist in globalen Monitorkoordinaten –
            // `i.screen_rect()` wäre nur lokal relativ zum Fenster (Ursprung 0,0)
            // und hätte hier fälschlich immer überlappt.
            if let Some(win) = ctx.input(|i| i.viewport().outer_rect) {
                let region = egui::Rect::from_min_max(
                    egui::pos2(ax1.min(ax2) as f32, ay1.min(ay2) as f32),
                    egui::pos2(ax1.max(ax2) as f32, ay1.max(ay2) as f32),
                );
                if win.intersects(region) {
                    message = "Das AutoMAUS-Fenster überdeckt den Bereich – bitte verschiebe das Fenster oder ziehe einen Bereich außerhalb des Fensters auf.".into();
                    break 'outer;
                }
            }

            let (rx1, ry1, rx2, ry2) = (ax1.min(ax2), ay1.min(ay2), ax1.max(ax2), ay1.max(ay2));
            let w = (rx2 - rx1) as u32;
            let h = (ry2 - ry1) as u32;

            let result = screenshots::Screen::from_point((rx1 + rx2) / 2, (ry1 + ry2) / 2)
                .map_err(|e| format!("Bildschirm nicht ermittelbar: {e}"))
                .and_then(|screen| {
                    // Koordinaten sind relativ zum Bildschirm-Ursprung
                    let di = screen.display_info;
                    screen
                        .capture_area(rx1 - di.x, ry1 - di.y, w, h)
                        .map_err(|e| {
                            format!(
                                "Screenshot fehlgeschlagen: {e} – macOS: \
                                 Bildschirmaufzeichnung erlaubt?"
                            )
                        })
                });

            match result {
                Ok(shot) => {
                    // Zielname: <Ablauf>_<n>.png – alten Pfad des Schritts wiederverwenden
                    let dir = std::path::Path::new(crate::workflow::WORKFLOW_DIR).join("images");
                    let _ = std::fs::create_dir_all(&dir);
                    let path = if old_image.is_empty() {
                        let base = workflow::safe_file_name(&ed.name);
                        let mut n = 1;
                        loop {
                            let candidate = dir.join(format!("{base}_{n}.png"));
                            if !candidate.exists() {
                                break candidate;
                            }
                            n += 1;
                        }
                    } else {
                        std::path::PathBuf::from(old_image)
                    };

                    match shot.save(&path) {
                        Ok(()) => {
                            let shown = path.display().to_string();
                            if let Action::FindImage { image, .. } =
                                &mut ed.workflow.steps[idx]
                            {
                                *image = shown.clone();
                            }
                            self.preview = None; // Vorschau-Cache ungültig machen
                            self.dirty = true;
                            ok = true;
                            message = format!("📸 Bereich gespeichert: {shown}");
                        }
                        Err(e) => {
                            message = format!("❌ Speichern fehlgeschlagen: {e}");
                        }
                    }
                }
                Err(e) => {
                    message = format!("❌ {e}");
                }
            }
        }

        self.status_text = message;
        self.status_is_error = !ok;
        ctx.request_repaint();
    }

    /// Verfolgt das »Bereich aufziehen«: globalen Mausdruck lesen, Start/Ziel
    /// aufnehmen und am Loslassen den Screenshot auslösen.
    fn poll_region_drag(&mut self, ctx: &egui::Context) {
        let Some(mut drag) = self.region_drag.take() else {
            return;
        };

        // Esc bricht ab
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.status_text = "Bereich-Aufziehen abgebrochen.".to_string();
            self.status_is_error = false;
            return;
        }

        let mut finished: Option<(i32, i32, i32, i32)> = None;
        match drag.phase {
            DragPhase::WaitPress => {
                if let Ok((x, y)) = drag.enigo.location() {
                    if left_button_down() && drag.since.elapsed() > Duration::from_millis(300) {
                        drag.phase = DragPhase::Dragging;
                        drag.start = (x, y);
                        drag.cur = (x, y);
                    }
                }
            }
            DragPhase::Dragging => {
                if let Ok((x, y)) = drag.enigo.location() {
                    drag.cur = (x, y);
                }
                if !left_button_down() {
                    let (sx, sy) = drag.start;
                    let (cx, cy) = drag.cur;
                    finished = Some((sx.min(cx), sy.min(cy), sx.max(cx), sy.max(cy)));
                }
            }
        }

        // Live-Hinweisfenster
        let mut cancel = false;
        egui::Window::new("Bereich aufziehen")
            .title_bar(false)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_TOP, [0.0, 40.0])
            .show(ctx, |ui| {
                ui.add_space(8.0);
                match drag.phase {
                    DragPhase::WaitPress => {
                        ui.heading("🖱️ Klicke an den Startpunkt und ziehe …");
                        ui.label("Die linke Maustaste gedrückt halten, den Bereich aufziehen und loslassen.");
                    }
                    DragPhase::Dragging => {
                        let (sx, sy) = drag.start;
                        let (cx, cy) = drag.cur;
                        ui.heading(format!(
                            "🖱️ {}×{} px",
                            (cx - sx).abs().max(1),
                            (cy - sy).abs().max(1)
                        ));
                        ui.label(format!(
                            "Von ({sx}, {sy}) bis ({cx}, {cy})"
                        ));
                    }
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("Esc = Abbrechen")
                            .color(crate::theme::muted(ui))
                            .size(11.0),
                    );
                    if ui.button("Abbrechen").clicked() {
                        cancel = true;
                    }
                });
                ui.add_space(4.0);
            });

        if cancel {
            self.status_text = "Bereich-Aufziehen abgebrochen.".to_string();
            self.status_is_error = false;
            return;
        }

        if let Some((ax1, ay1, ax2, ay2)) = finished {
            // Gültigkeit prüfen: Schritt muss noch ausgewählt & vom Typ FindImage sein
            let valid = matches!(
                &self.view,
                AppView::Editor(ed)
                    if ed.selected == Some(drag.step)
                        && matches!(
                            ed.workflow.steps.get(drag.step),
                            Some(Action::FindImage { .. })
                        )
            );
            if valid {
                self.capture_region_image(ctx, Some((ax1, ay1, ax2, ay2)));
            } else {
                self.status_text =
                    "❌ Schritt hat sich geändert – Bereich-Aufziehen abgebrochen.".to_string();
                self.status_is_error = true;
            }
            return;
        }

        // Weiter beobachten (~30 Hz reichen für flüssige Koordinaten)
        ctx.request_repaint_after(Duration::from_millis(33));
        self.region_drag = Some(drag);
    }

    fn poll_worker(&mut self) {
        let mut finished = None;
        if let Some(w) = &self.worker {
            while let Ok(msg) = w.rx.try_recv() {
                match msg {
                    WorkerMsg::Status(s) => {
                        self.status_text = s;
                        self.status_is_error = false;
                    }
                    WorkerMsg::StepStarted(idx) => {
                        self.running_step = Some(idx);
                    }
                    WorkerMsg::Finished(ok, err) => {
                        finished = Some((ok, err));
                    }
                }
            }
        }
        if let Some((ok, err)) = finished {
            self.worker = None;
            self.running_step = None;
            match (ok, err) {
                (true, _) => {
                    self.status_text = "✅ Erfolgreich abgeschlossen.".to_string();
                    self.status_is_error = false;
                }
                (false, Some(e)) => {
                    let gentle = e.contains("gestoppt") || e.contains("Failsafe");
                    self.status_text = if gentle {
                        format!("⏹ {e}")
                    } else {
                        format!("❌ {e}")
                    };
                    self.status_is_error = !gentle;
                }
                (false, None) => {
                    self.status_text = "Abgebrochen.".to_string();
                    self.status_is_error = true;
                }
            }
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context, out: &mut FrameOutcome) {
        let modal_open =
            self.confirm_delete.is_some() || self.pending_back || self.key_capture.is_some();
        let in_editor = matches!(self.view, AppView::Editor(_));
        let running = self.worker.is_some();
        let selected = match &self.view {
            AppView::Editor(ed) => ed.selected,
            AppView::Dashboard => None,
        };

        ctx.input_mut(|i| {
            if in_editor && i.consume_key(egui::Modifiers::COMMAND, egui::Key::S) {
                out.save = true;
            }
            if i.consume_key(egui::Modifiers::COMMAND, egui::Key::N) {
                out.new_workflow = true;
            }
            if in_editor && !running && i.consume_key(egui::Modifiers::NONE, egui::Key::F5) {
                out.start_editor = true;
            }
            if !modal_open && running && i.consume_key(egui::Modifiers::NONE, egui::Key::Escape) {
                out.stop = true;
            }
        });

        if !modal_open && in_editor && selected.is_some() && !ctx.wants_keyboard_input() {
            ctx.input_mut(|i| {
                if i.consume_key(egui::Modifiers::COMMAND, egui::Key::D) {
                    out.duplicate_step = selected;
                }
                if i.consume_key(egui::Modifiers::NONE, egui::Key::Delete) {
                    out.delete_step = selected;
                }
                if i.consume_key(egui::Modifiers::ALT, egui::Key::ArrowUp) {
                    out.move_up = selected;
                }
                if i.consume_key(egui::Modifiers::ALT, egui::Key::ArrowDown) {
                    out.move_down = selected;
                }
            });
        }
    }

    // -----------------------------------------------------------------------
    // Aktionen
    // -----------------------------------------------------------------------

    fn spawn(&mut self, ctx: &egui::Context, wf: Workflow) {
        let total = wf.steps.len();
        let (tx, rx) = mpsc::channel();
        let stop_flag = Arc::new(AtomicBool::new(false));
        let bounds = worker::screen_bounds();
        self.worker = Some(WorkerState {
            should_stop: Arc::clone(&stop_flag),
            rx,
        });
        self.run_total = total;
        self.running_step = None;
        self.status_text = "Starte Ablauf …".to_string();
        self.status_is_error = false;
        worker::run_workflow(wf, stop_flag, tx, ctx.clone(), bounds);
    }

    fn do_start_editor(&mut self, ctx: &egui::Context) {
        if self.worker.is_some() {
            return;
        }
        let AppView::Editor(ed) = &self.view else {
            return;
        };
        if let Err(e) = ed.workflow.validate() {
            self.status_text = format!("❌ {e}");
            self.status_is_error = true;
            return;
        }
        let wf = ed.workflow.clone();
        self.spawn(ctx, wf);
    }

    fn do_start_path(&mut self, ctx: &egui::Context, path: &Path) {
        if self.worker.is_some() {
            return;
        }
        let wf = match workflow::load_workflow(path) {
            Ok(w) => w,
            Err(e) => {
                self.status_text = format!("❌ Öffnen fehlgeschlagen: {e}");
                self.status_is_error = true;
                return;
            }
        };
        if let Err(e) = wf.validate() {
            self.status_text = format!("❌ {e}");
            self.status_is_error = true;
            return;
        }
        self.spawn(ctx, wf);
    }

    fn do_save(&mut self) {
        let AppView::Editor(ed) = &mut self.view else {
            return;
        };
        let path = ed.path.clone().unwrap_or_else(|| {
            let base = workflow::safe_file_name(&ed.name);
            let mut candidate = PathBuf::from(format!("{WORKFLOW_DIR}/{base}.json"));
            let mut n = 2;
            while candidate.exists() {
                candidate = PathBuf::from(format!("{WORKFLOW_DIR}/{base} ({n}).json"));
                n += 1;
            }
            candidate
        });
        match workflow::save_workflow(&path, &ed.workflow) {
            Ok(()) => {
                ed.path = Some(path.clone());
                self.dirty = false;
                self.files = workflow::list_workflows();
                self.status_text = format!("💾 Gespeichert: {}", path.display());
                self.status_is_error = false;
            }
            Err(e) => {
                self.status_text = format!("❌ Speichern fehlgeschlagen: {e}");
                self.status_is_error = true;
            }
        }
    }

    fn do_stop(&mut self) {
        if let Some(w) = &self.worker {
            w.should_stop.store(true, Ordering::Relaxed);
            self.status_text = "Stoppe …".to_string();
            self.status_is_error = false;
        }
    }

    fn apply(&mut self, ctx: &egui::Context, out: FrameOutcome) {
        if out.toggle_theme {
            self.theme = self.theme.toggled();
            theme::apply(ctx, self.theme);
        }

        if out.new_workflow {
            self.view = AppView::Editor(EditorState::new_unsaved());
            self.dirty = false;
            self.status_text =
                "Neuer Ablauf – gib ihm einen Namen und füge links Schritte hinzu.".to_string();
            self.status_is_error = false;
        }

        if let Some(path) = out.open_path {
            match workflow::load_workflow(&path) {
                Ok(wf) => {
                    self.view = AppView::Editor(EditorState::from_file(path.clone(), wf));
                    self.dirty = false;
                    self.status_text = format!("Geöffnet: {}", path.display());
                    self.status_is_error = false;
                }
                Err(e) => {
                    self.status_text = format!("❌ Öffnen fehlgeschlagen: {e}");
                    self.status_is_error = true;
                }
            }
        }

        if let Some(path) = out.delete_path {
            self.confirm_delete = Some(path);
        }

        if out.back {
            if self.dirty {
                self.pending_back = true;
            } else {
                self.view = AppView::Dashboard;
            }
        }

        if let AppView::Editor(ed) = &mut self.view {
            if let Some(a) = out.add_step {
                ed.workflow.steps.push(a);
                ed.selected = Some(ed.workflow.steps.len() - 1);
                ed.tab = InspectorTab::Step;
                self.dirty = true;
            }

            if let Some(i) = out.select_step.filter(|i| *i < ed.workflow.steps.len()) {
                ed.selected = Some(i);
                ed.tab = InspectorTab::Step;
            }

            if let Some(i) = out.delete_step.filter(|i| *i < ed.workflow.steps.len()) {
                ed.workflow.steps.remove(i);
                ed.selected = match ed.selected {
                    Some(s) if s > i => Some(s - 1),
                    Some(s) if s == i => {
                        if i < ed.workflow.steps.len() {
                            Some(i)
                        } else {
                            None
                        }
                    }
                    other => other,
                };
                if ed.selected.is_none() {
                    ed.tab = InspectorTab::Settings;
                }
                self.dirty = true;
            }

            if let Some(i) = out.duplicate_step.filter(|i| *i < ed.workflow.steps.len()) {
                let copy = ed.workflow.steps[i].clone();
                ed.workflow.steps.insert(i + 1, copy);
                ed.selected = Some(i + 1);
                ed.tab = InspectorTab::Step;
                self.dirty = true;
            }

            if let Some(i) = out.move_up.filter(|i| *i > 0 && *i < ed.workflow.steps.len()) {
                ed.workflow.steps.swap(i, i - 1);
                ed.selected = Some(i - 1);
                self.dirty = true;
            }

            if let Some(i) = out
                .move_down
                .filter(|i| *i + 1 < ed.workflow.steps.len())
            {
                ed.workflow.steps.swap(i, i + 1);
                ed.selected = Some(i + 1);
                self.dirty = true;
            }

            if let Some((f, t)) = out
                .drop_step
                .filter(|(f, t)| f != t && *f < ed.workflow.steps.len() && *t < ed.workflow.steps.len())
            {
                let step = ed.workflow.steps.remove(f);
                ed.workflow.steps.insert(t, step);
                ed.selected = Some(t);
                ed.tab = InspectorTab::Step;
                self.dirty = true;
            }
        }

        if out.capture_position {
            self.capture_timer = Some(Instant::now() + Duration::from_secs(3));
            self.capture_slot = out.capture_slot;
            self.status_text = "Bewege die Maus an die Zielposition …".to_string();
            self.status_is_error = false;
        }

        if let Some(_idx) = out.capture_image {
            self.capture_region_image(ctx, None);
        }

        if let Some(idx) = out.capture_image_drag {
            #[cfg(target_os = "macos")]
            {
                match Enigo::new(&Settings::default()) {
                    Ok(enigo) => {
                        self.region_drag = Some(RegionDragState {
                            step: idx,
                            phase: DragPhase::WaitPress,
                            since: Instant::now(),
                            start: (0, 0),
                            cur: (0, 0),
                            enigo,
                        });
                        self.status_text =
                            "Klicke an den Startpunkt und ziehe den Bereich auf (Esc = Abbrechen)."
                                .to_string();
                        self.status_is_error = false;
                    }
                    Err(e) => {
                        self.status_text =
                            format!("❌ Maus-Zugriff fehlgeschlagen: {e:?} – Bedienungshilfen erlaubt?");
                        self.status_is_error = true;
                    }
                }
            }
            #[cfg(not(target_os = "macos"))]
            {
                let _ = idx;
                self.status_text =
                    "❌ »Bereich aufziehen« ist derzeit nur auf macOS verfügbar – nutze die Eck-Erfassung."
                        .to_string();
                self.status_is_error = true;
            }
        }

        if let Some((step, hotkey)) = out.key_capture {
            self.key_capture = Some(KeyCapture { step, hotkey });
        }

        if out.save {
            self.do_save();
        }
        if out.stop {
            self.do_stop();
        }
        if out.start_editor {
            self.do_start_editor(ctx);
        }
        if let Some(path) = out.start_path {
            self.do_start_path(ctx, &path);
        }
    }
}

// ---------------------------------------------------------------------------
// eframe::App
// ---------------------------------------------------------------------------

impl eframe::App for WorkflowApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_capture(ctx);
        self.poll_region_drag(ctx);
        self.poll_worker();

        let mut out = FrameOutcome::default();
        self.shortcuts(ctx, &mut out);

        // Stand zu Beginn des Frames – zum Erkennen von Änderungen (Dirty-Flag)
        let (snap_name, snap_wf) = match &self.view {
            AppView::Editor(ed) => (ed.name.clone(), ed.workflow.clone()),
            AppView::Dashboard => (String::new(), Workflow::default()),
        };

        let theme = self.theme;
        let dirty = self.dirty;
        let running = self.worker.is_some();

        // Während der Ausführung regelmäßig neu zeichnen (Fortschritt, Spinner)
        if running {
            ctx.request_repaint_after(Duration::from_millis(100));
        }

        // ---- Toolbar ----
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("⚡").size(17.0).color(theme::ACCENT));
                ui.label(egui::RichText::new("AutoMAUS").size(16.0).strong());
                ui.label(
                    egui::RichText::new("by Gschwendtner Johannes")
                        .color(crate::theme::muted(ui))
                        .size(12.0),
                );
                ui.add_space(8.0);

                match &mut self.view {
                    AppView::Dashboard => {
                        ui.label(egui::RichText::new("Workflow Studio").color(crate::theme::muted(ui)));
                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                if ui
                                    .button(theme.icon())
                                    .on_hover_text("Design wechseln")
                                    .clicked()
                                {
                                    out.toggle_theme = true;
                                }
                            },
                        );
                    }
                    AppView::Editor(ed) => {
                        if ui
                            .button("‹  Zurück")
                            .on_hover_text("Zum Dashboard")
                            .clicked()
                        {
                            out.back = true;
                        }
                        ui.separator();
                        ui.add(
                            egui::TextEdit::singleline(&mut ed.name)
                                .desired_width(220.0)
                                .hint_text("Name des Ablaufs"),
                        );
                        if dirty {
                            ui.label(
                                egui::RichText::new("●").color(theme::DANGER),
                            )
                            .on_hover_text("Ungespeicherte Änderungen");
                        }
                        if ui
                            .button("💾  Speichern")
                            .on_hover_text("Strg+S")
                            .clicked()
                        {
                            out.save = true;
                        }
                        ui.separator();
                        if running {
                            if ui
                                .button("⏹  Stop")
                                .on_hover_text("Esc")
                                .clicked()
                            {
                                out.stop = true;
                            }
                        } else if ui
                            .button("▶  Starten")
                            .on_hover_text("F5")
                            .clicked()
                        {
                            out.start_editor = true;
                        }
                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                if ui
                                    .button(theme.icon())
                                    .on_hover_text("Design wechseln")
                                    .clicked()
                                {
                                    out.toggle_theme = true;
                                }
                            },
                        );
                    }
                }
            });
        });

        // ---- Statusleiste ----
        egui::TopBottomPanel::bottom("status_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if self.status_is_error {
                    ui.label(egui::RichText::new(&self.status_text).color(theme::DANGER));
                } else {
                    ui.label(&self.status_text);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if running {
                        if ui
                            .button("🛑  Stop (Notbremse)")
                            .on_hover_text("Esc")
                            .clicked()
                        {
                            out.stop = true;
                        }
                        let frac = if self.run_total > 0 {
                            (self.running_step.map_or(0, |s| s + 1) as f32)
                                / self.run_total as f32
                        } else {
                            0.0
                        };
                        ui.add(
                            egui::ProgressBar::new(frac)
                                .desired_width(150.0)
                                .show_percentage(),
                        );
                        if let Some(s) = self.running_step {
                            ui.label(
                                egui::RichText::new(format!("Schritt {}/{}", s + 1, self.run_total))
                                    .strong(),
                            );
                        }
                    } else {
                        ui.label(
                            egui::RichText::new(
                                "Notfall-Stopp: Maus in eine Bildschirmecke bewegen",
                            )
                            .color(crate::theme::muted(ui)),
                        );
                    }
                });
            });
        });

        // ---- Hauptbereich ----
        match &mut self.view {
            AppView::Dashboard => {
                egui::CentralPanel::default().show(ctx, |ui| {
                    // Logo-Wasserzeichen (15 % Deckkraft, zentriert) – wird
                    // zuerst gemalt, der Listeninhalt liegt darüber.
                    let logo_id = {
                        if self.logo.is_none() {
                            self.logo = logo_texture(ctx);
                        }
                        self.logo.as_ref().map(|t| t.id())
                    };
                    if let Some(id) = logo_id {
                        let avail = ui.available_rect_before_wrap();
                        // Quadratisches Logo, 45 % der kürzeren Seite –
                        // deutlich unter ¾ der UI-Fläche.
                        let side = 0.45 * avail.width().min(avail.height());
                        let rect = egui::Rect::from_center_size(
                            avail.center(),
                            egui::Vec2::splat(side),
                        );
                        ui.painter().image(
                            id,
                            rect,
                            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                            egui::Color32::WHITE,
                        );
                    }

                    ui_dashboard::show(
                        ui,
                        &self.files,
                        &mut self.search,
                        self.worker.is_some(),
                        &mut out,
                    );
                });
            }
            AppView::Editor(_) => {
                egui::SidePanel::left("action_rail")
                    .exact_width(58.0)
                    .show(ctx, |ui| {
                        ui_editor::icon_rail(ui, &mut out);
                    });

                egui::SidePanel::right("inspector")
                    .exact_width(300.0)
                    .show(ctx, |ui| {
                        if let AppView::Editor(ed) = &mut self.view {
                            ui_editor::inspector(ui, ed, &mut self.preview, running, &mut out);
                        }
                    });

                egui::CentralPanel::default().show(ctx, |ui| {
                    if let AppView::Editor(ed) = &mut self.view {
                        let running_step = self.running_step;
                        ui_editor::step_list(ui, ed, running_step, &mut out);
                    }
                });
            }
        }

        // ---- Dialoge ----
        if let Some(path) = ui_modals::delete_confirm(ctx, &mut self.confirm_delete) {
            match workflow::delete_workflow(&path) {
                Ok(()) => {
                    self.files = workflow::list_workflows();
                    self.status_text = format!("🗑 Gelöscht: {}", path.display());
                    self.status_is_error = false;
                }
                Err(e) => {
                    self.status_text = format!("❌ {e}");
                    self.status_is_error = true;
                }
            }
        }

        let current_name = match &self.view {
            AppView::Editor(ed) => ed.name.clone(),
            AppView::Dashboard => String::new(),
        };
        if let Some(choice) = ui_modals::unsaved_confirm(ctx, &mut self.pending_back, &current_name)
        {
            match choice {
                UnsavedChoice::Cancel => {}
                UnsavedChoice::Discard => {
                    self.dirty = false;
                    self.view = AppView::Dashboard;
                }
                UnsavedChoice::Save => {
                    self.do_save();
                    if !self.dirty {
                        self.view = AppView::Dashboard;
                    }
                }
            }
        }

        if let Some((idx, hotkey, keys)) =
            ui_modals::key_capture_window(ctx, &mut self.key_capture)
        {
            if let AppView::Editor(ed) = &mut self.view {
                if let Some(step) = ed.workflow.steps.get_mut(idx) {
                    let applied = match (step, hotkey) {
                        (Action::KeyPress { key }, false) => {
                            if let Some(k) = keys.last() {
                                *key = k.clone();
                                true
                            } else {
                                false
                            }
                        }
                        (Action::Hotkey { keys: combo }, true) => {
                            *combo = keys;
                            true
                        }
                        _ => false,
                    };
                    if applied {
                        self.dirty = true;
                        self.status_text = "🔑 Taste aufgenommen.".to_string();
                        self.status_is_error = false;
                    }
                }
            }
        }

        // Dirty-Flag aus dem Frame-Vergleich setzen (vor apply, weil Speichern es zurücksetzt)
        if let AppView::Editor(ed) = &self.view {
            if ed.name != snap_name || ed.workflow != snap_wf {
                self.dirty = true;
            }
        }

        self.apply(ctx, out);
    }
}
