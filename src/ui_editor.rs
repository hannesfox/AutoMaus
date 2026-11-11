//! Editor: Aktions-Leiste, Schrittliste (mit Schleifen-Baum) und Inspektor.

use crate::actions::{combo_label, Action, FindMode, KeySpec, MouseButton};
use crate::state::{CaptureSlot, EditorState, FrameOutcome, InspectorTab};
use crate::theme;
use crate::workflow::Workflow;
use eframe::egui::{
    self, Align, Align2, Color32, FontId, Layout, Pos2, Rect, RichText, Rounding, Sense, Stroke,
    Vec2,
};

// ---------------------------------------------------------------------------
// Linke Aktions-Leiste
// ---------------------------------------------------------------------------

pub fn icon_rail(ui: &mut egui::Ui, out: &mut FrameOutcome) {
    egui::ScrollArea::vertical()
        .id_source("action_rail")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(6.0);
                ui.label(RichText::new("AKTIONEN").color(crate::theme::muted(ui)).size(9.0));
                ui.add_space(6.0);

                rail_button(
                    ui,
                    "🖱",
                    "Mausklick an Position",
                    Action::Click {
                        x: 0,
                        y: 0,
                        button: MouseButton::Left,
                        clicks: 1,
                        interval_sec: 0.0,
                    },
                    out,
                );
                rail_button(
                    ui,
                    "➡",
                    "Maus zur Position bewegen",
                    Action::MoveTo {
                        x: 0,
                        y: 0,
                        duration_sec: 0.0,
                    },
                    out,
                );
                rail_button(
                    ui,
                    "↔",
                    "Maus relativ bewegen",
                    Action::MoveRelative {
                        dx: 0,
                        dy: 0,
                        duration_sec: 0.0,
                    },
                    out,
                );
                rail_button(
                    ui,
                    "✋",
                    "Ziehen (Drag & Drop)",
                    Action::Drag {
                        from_x: 0,
                        from_y: 0,
                        to_x: 0,
                        to_y: 0,
                        button: MouseButton::Left,
                        duration_sec: 0.5,
                    },
                    out,
                );
                rail_button(
                    ui,
                    "📜",
                    "Scrollen",
                    Action::Scroll { amount: 3 },
                    out,
                );

                ui.add_space(6.0);
                ui.separator();
                ui.add_space(6.0);

                rail_button(
                    ui,
                    "⌨",
                    "Text tippen",
                    Action::Typewrite {
                        text: String::new(),
                        interval_sec: 0.0,
                    },
                    out,
                );
                rail_button(
                    ui,
                    "🔑",
                    "Einzelne Taste drücken",
                    Action::KeyPress {
                        key: KeySpec::default(),
                    },
                    out,
                );
                rail_button(
                    ui,
                    "🎹",
                    "Tastenkombination (z. B. Strg+C)",
                    Action::Hotkey { keys: vec![] },
                    out,
                );
                rail_button(
                    ui,
                    "⏳",
                    "Pause",
                    Action::Sleep { duration_sec: 1.0 },
                    out,
                );

                ui.add_space(6.0);
                ui.separator();
                ui.add_space(6.0);

                rail_button(
                    ui,
                    "🔍",
                    "Bild finden und anklicken",
                    Action::FindImage {
                        x1: 0,
                        y1: 0,
                        x2: 0,
                        y2: 0,
                        image: String::new(),
                        mode: FindMode::Click,
                        timeout_sec: 5.0,
                        duration_sec: 0.0,
                    },
                    out,
                );

                ui.add_space(6.0);
                ui.separator();
                ui.add_space(6.0);

                rail_button(
                    ui,
                    "🔁",
                    "Schleife starten",
                    Action::LoopStart { count: 3 },
                    out,
                );
                rail_button(ui, "🏁", "Schleife beenden", Action::LoopEnd, out);
                ui.add_space(12.0);
            });
        });
}

fn rail_button(ui: &mut egui::Ui, icon: &str, tip: &str, action: Action, out: &mut FrameOutcome) {
    let resp = ui
        .add_sized(
            [46.0, 40.0],
            egui::Button::new(RichText::new(icon).size(17.0)),
        )
        .on_hover_text(tip);
    if resp.clicked() {
        out.add_step = Some(action);
    }
    ui.add_space(2.0);
}

// ---------------------------------------------------------------------------
// Mitte: Schrittliste
// ---------------------------------------------------------------------------

pub fn step_list(
    ui: &mut egui::Ui,
    editor: &mut EditorState,
    running_step: Option<usize>,
    out: &mut FrameOutcome,
) {
    let EditorState {
        path: _,
        name: _,
        workflow,
        selected,
        tab: _,
        drag_from,
    } = editor;

    ui.horizontal(|ui| {
        ui.heading("Schritte");
        ui.label(
            RichText::new(format!("({})", workflow.steps.len()))
                .color(crate::theme::muted(ui))
                .size(13.0),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(
                RichText::new("Ziehen zum Sortieren · Rechtsklick für Optionen")
                    .color(crate::theme::muted(ui))
                    .size(11.0),
            );
        });
    });
    ui.add_space(6.0);

    if workflow.steps.is_empty() {
        ui.add_space(40.0);
        ui.vertical_centered(|ui| {
            ui.label(RichText::new("🚀").size(44.0));
            ui.add_space(10.0);
            ui.label(RichText::new("Noch keine Schritte").size(15.0).strong());
            ui.add_space(4.0);
            ui.label(
                RichText::new("Füge links Aktionen hinzu, um deinen Ablauf zu bauen.").color(crate::theme::muted(ui)),
            );
        });
        return;
    }

    let depths = crate::actions::loop_depths(&workflow.steps);
    let n = workflow.steps.len();
    let mut hover_row: Option<usize> = None;
    let mut release_row: Option<usize> = None;

    egui::ScrollArea::vertical()
        .id_source("step_list")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for i in 0..n {
                let depth = depths[i];
                let step = workflow.steps[i].clone();
                let width = ui.available_width();
                let (rect, resp) =
                    ui.allocate_exact_size(Vec2::new(width, 32.0), Sense::click_and_drag());
                let painter = ui.painter();

                let is_selected = *selected == Some(i);
                let is_running = running_step == Some(i);
                let is_source = *drag_from == Some(i);
                let is_target = drag_from.is_some() && *drag_from != Some(i) && resp.hovered();

                if resp.hovered() && drag_from.is_some() {
                    hover_row = Some(i);
                }
                if resp.drag_stopped() {
                    release_row = Some(i);
                }

                // Hintergrund
                let fill = if is_running {
                    theme::SUCCESS_SOFT
                } else if is_selected {
                    theme::ACCENT_SOFT
                } else if is_target {
                    theme::TARGET_SOFT
                } else if resp.hovered() {
                    ui.visuals().widgets.hovered.bg_fill
                } else {
                    Color32::TRANSPARENT
                };
                if fill != Color32::TRANSPARENT {
                    painter.rect_filled(rect, Rounding::same(6.0), fill);
                }
                if is_source {
                    painter.rect_filled(
                        rect,
                        Rounding::same(6.0),
                        Color32::from_rgba_unmultiplied(128, 128, 128, 50),
                    );
                }

                // Auswahl-Balken
                if is_selected {
                    painter.rect_filled(
                        Rect::from_min_size(
                            Pos2::new(rect.min.x + 3.0, rect.min.y + 7.0),
                            Vec2::new(3.0, rect.height() - 14.0),
                        ),
                        Rounding::same(2.0),
                        theme::ACCENT,
                    );
                }

                // Einrückung + Schleifen-Führungslinie
                let indent = 12.0 + depth as f32 * 20.0;
                if depth > 0 {
                    let gx = rect.min.x + 12.0 + (depth as f32 - 1.0) * 20.0 + 10.0;
                    painter.line_segment(
                        [
                            Pos2::new(gx, rect.min.y + 3.0),
                            Pos2::new(gx, rect.max.y - 3.0),
                        ],
                        Stroke::new(1.0_f32, ui.visuals().weak_text_color()),
                    );
                }

                let cy = rect.center().y;
                let has_buttons = resp.hovered() || is_selected;
                let buttons_w = if has_buttons { 104.0 } else { 10.0 };

                // Textfarben: bei Auswahl/Ausführung explizit weiß (sonst dunkle
                // Schrift auf farbigem Auswahl-Hintergrund)
                let row_text = if is_selected || is_running {
                    Color32::WHITE
                } else {
                    ui.visuals().text_color()
                };

                // Nummer
                painter.text(
                    Pos2::new(rect.min.x + indent + 2.0, cy),
                    Align2::LEFT_CENTER,
                    format!("{}.", i + 1),
                    FontId::proportional(11.0),
                    row_text.gamma_multiply(0.7),
                );
                // Icon
                painter.text(
                    Pos2::new(rect.min.x + indent + 24.0, cy),
                    Align2::LEFT_CENTER,
                    step.icon(),
                    FontId::proportional(14.0),
                    row_text,
                );
                // Label
                let text_x = indent + 46.0;
                let max_w = (rect.width() - text_x - buttons_w - 10.0).max(40.0);
                let label = truncate(&step.label(), max_w);
                painter.text(
                    Pos2::new(rect.min.x + text_x, cy),
                    Align2::LEFT_CENTER,
                    label,
                    FontId::proportional(13.0),
                    row_text,
                );

                // Aktionen (nur bei Hover/Auswahl)
                let mut button_hit = false;
                if has_buttons {
                    let pointer = ui.input(|inp| inp.pointer.latest_pos());
                    let defs: [(&str, &str); 4] = [
                        ("⧉", "Duplizieren"),
                        ("↑", "Nach oben"),
                        ("↓", "Nach unten"),
                        ("🗑", "Löschen"),
                    ];
                    let bw = 24.0;
                    let base = rect.max.x - 8.0 - defs.len() as f32 * bw;
                    for (k, (glyph, _tip)) in defs.iter().enumerate() {
                        let br = Rect::from_min_size(
                            Pos2::new(base + k as f32 * bw, rect.min.y + 5.0),
                            Vec2::new(bw, rect.height() - 10.0),
                        );
                        let hov = pointer.is_some_and(|p| br.contains(p));
                        if hov {
                            painter.rect_filled(
                                br,
                                Rounding::same(4.0),
                                ui.visuals().widgets.hovered.bg_fill,
                            );
                        }
                        painter.text(
                            br.center(),
                            Align2::CENTER_CENTER,
                            *glyph,
                            FontId::proportional(12.5),
                            ui.visuals().text_color(),
                        );
                        if hov && resp.clicked() {
                            button_hit = true;
                            match k {
                                0 => out.duplicate_step = Some(i),
                                1 => out.move_up = Some(i),
                                2 => out.move_down = Some(i),
                                3 => out.delete_step = Some(i),
                                _ => {}
                            }
                        }
                    }
                }

                if !button_hit && resp.clicked() {
                    out.select_step = Some(i);
                }

                // Kontextmenü
                resp.context_menu(|ui| {
                    if ui.button("⧉  Duplizieren").clicked() {
                        out.duplicate_step = Some(i);
                        ui.close_menu();
                    }
                    if ui.button("▲  Nach oben").clicked() && i > 0 {
                        out.move_up = Some(i);
                        ui.close_menu();
                    }
                    if ui.button("▼  Nach unten").clicked() && i + 1 < n {
                        out.move_down = Some(i);
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("🗑  Löschen").clicked() {
                        out.delete_step = Some(i);
                        ui.close_menu();
                    }
                });

                // Drag & Drop
                if resp.drag_started() {
                    *drag_from = Some(i);
                }
            }

            // Ablage nach dem Ziehen
            if release_row.is_some() {
                if let (Some(f), Some(t)) = (*drag_from, hover_row) {
                    if f != t && t < n {
                        out.drop_step = Some((f, t));
                    }
                }
                *drag_from = None;
            }
        });
}

fn truncate(text: &str, max_w: f32) -> String {
    let max_chars = (max_w / 7.0) as usize;
    if text.chars().count() <= max_chars {
        text.to_string()
    } else {
        let cut: String = text.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{cut}…")
    }
}

// ---------------------------------------------------------------------------
// Rechts: Inspektor
// ---------------------------------------------------------------------------

pub fn inspector(
    ui: &mut egui::Ui,
    editor: &mut EditorState,
    preview: &mut Option<(String, egui::TextureHandle)>,
    running: bool,
    out: &mut FrameOutcome,
) {
    let EditorState {
        path: _,
        name: _,
        workflow,
        selected,
        tab,
        drag_from: _,
    } = editor;

    ui.horizontal(|ui| {
        ui.selectable_value(tab, InspectorTab::Step, "Schritt");
        ui.selectable_value(tab, InspectorTab::Settings, "Ablauf");
    });
    ui.separator();
    ui.add_space(4.0);

    match *tab {
        InspectorTab::Settings => settings_ui(ui, workflow),
        InspectorTab::Step => {
            let Some(idx) = selected.filter(|i| *i < workflow.steps.len()) else {
                placeholder(ui, "Wähle links einen Schritt aus, um ihn zu bearbeiten.");
                return;
            };
            step_ui(ui, idx, &mut workflow.steps[idx], preview, running, out);
        }
    }
}

fn placeholder(ui: &mut egui::Ui, text: &str) {
    ui.add_space(24.0);
    ui.vertical_centered(|ui| {
        ui.label(RichText::new("👈").size(34.0));
        ui.add_space(10.0);
        ui.label(RichText::new(text).color(crate::theme::muted(ui)));
    });
}

fn settings_ui(ui: &mut egui::Ui, workflow: &mut Workflow) {
    ui.strong("Ablauf-Einstellungen");
    ui.add_space(8.0);

    ui.horizontal(|ui| {
        ui.label("Start-Verzögerung (s):");
        ui.add(
            egui::DragValue::new(&mut workflow.start_delay)
                .speed(0.1)
                .clamp_range(0.0..=120.0),
        );
    });
    ui.horizontal(|ui| {
        ui.label("Pause zw. Aktionen (s):");
        ui.add(
            egui::DragValue::new(&mut workflow.step_delay)
                .speed(0.05)
                .clamp_range(0.0..=30.0),
        );
    });
    ui.add_space(4.0);
    ui.label(
        RichText::new("Entspricht pyautogui's »PAUSE« – Zeit vor dem Start bzw. zwischen zwei Aktionen.")
            .color(crate::theme::muted(ui))
            .size(11.0),
    );

    ui.add_space(14.0);
    ui.separator();
    ui.add_space(6.0);
    ui.strong("Struktur-Prüfung");
    ui.add_space(4.0);
    match workflow.validate() {
        Ok(()) => {
            ui.label(RichText::new("✓  Ablauf-Struktur ist gültig").color(theme::SUCCESS));
        }
        Err(e) => {
            ui.label(RichText::new(format!("⚠  {e}")).color(theme::DANGER));
        }
    }

    ui.add_space(14.0);
    ui.separator();
    ui.add_space(6.0);
    ui.strong("Tastenkürzel");
    ui.add_space(4.0);
    for line in [
        "Strg+S – Speichern",
        "F5 – Ablauf starten",
        "Esc – Notbremse (während der Ausführung)",
        "Strg+D – Schritt duplizieren",
        "Entf – Schritt löschen",
        "Alt+↑ / Alt+↓ – Schritt verschieben",
    ] {
        ui.label(RichText::new(format!("•  {line}")).color(crate::theme::muted(ui)).size(11.5));
    }
}

fn coords_ui(ui: &mut egui::Ui, x: &mut i32, y: &mut i32, out: &mut FrameOutcome) {
    ui.horizontal(|ui| {
        ui.label("X:");
        ui.add(egui::DragValue::new(x));
        ui.label("Y:");
        ui.add(egui::DragValue::new(y));
    });
    ui.add_space(6.0);
    if ui
        .button("📍  Position in 3 s erfassen")
        .on_hover_text("Danach den Mauszeiger an die gewünschte Stelle bewegen")
        .clicked()
    {
        out.capture_position = true;
        out.capture_slot = CaptureSlot::Primary;
    }
}

fn button_combo(ui: &mut egui::Ui, button: &mut MouseButton) {
    ui.label("Maustaste:");
    egui::ComboBox::from_id_source("click_button")
        .selected_text(button.label())
        .width(140.0)
        .show_ui(ui, |ui| {
            for b in MouseButton::all() {
                ui.selectable_value(button, b, b.label());
            }
        });
}

fn step_ui(
    ui: &mut egui::Ui,
    idx: usize,
    step: &mut Action,
    preview: &mut Option<(String, egui::TextureHandle)>,
    running: bool,
    out: &mut FrameOutcome,
) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(step.icon()).size(18.0));
        ui.label(RichText::new(step.label()).strong());
    });
    ui.separator();
    ui.add_space(4.0);

    match step {
        Action::Click {
            x,
            y,
            button,
            clicks,
            interval_sec,
        } => {
            coords_ui(ui, x, y, out);
            ui.add_space(8.0);
            button_combo(ui, button);
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("Anzahl:");
                ui.add(egui::DragValue::new(clicks).clamp_range(1..=10));
            });
            if *clicks > 1 {
                ui.horizontal(|ui| {
                    ui.label("Intervall (s):");
                    ui.add(
                        egui::DragValue::new(interval_sec)
                            .speed(0.05)
                            .clamp_range(0.0..=5.0),
                    );
                });
            }
        }

        Action::MoveTo {
            x,
            y,
            duration_sec,
        } => {
            coords_ui(ui, x, y, out);
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label("Dauer (s):");
                ui.add(
                    egui::DragValue::new(duration_sec)
                        .speed(0.05)
                        .clamp_range(0.0..=20.0),
                );
            });
            ui.label(
                RichText::new("0 = direkter Sprung · > 0 = weich animiert (wie pyautogui duration)")
                    .color(crate::theme::muted(ui))
                    .size(11.0),
            );
        }

        Action::MoveRelative {
            dx,
            dy,
            duration_sec,
        } => {
            ui.horizontal(|ui| {
                ui.label("Δ X:");
                ui.add(egui::DragValue::new(dx));
                ui.label("Δ Y:");
                ui.add(egui::DragValue::new(dy));
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label("Dauer (s):");
                ui.add(
                    egui::DragValue::new(duration_sec)
                        .speed(0.05)
                        .clamp_range(0.0..=20.0),
                );
            });
            ui.label(
                RichText::new("Relativ zur aktuellen Mausposition (wie pyautogui moveRel).")
                    .color(crate::theme::muted(ui))
                    .size(11.0),
            );
        }

        Action::Drag {
            from_x,
            from_y,
            to_x,
            to_y,
            button,
            duration_sec,
        } => {
            ui.strong("Von:");
            ui.horizontal(|ui| {
                ui.label("X:");
                ui.add(egui::DragValue::new(from_x));
                ui.label("Y:");
                ui.add(egui::DragValue::new(from_y));
            });
            if ui
                .button("📍  Von hier erfassen")
                .on_hover_text("Position in 3 s aufnehmen")
                .clicked()
            {
                out.capture_position = true;
                out.capture_slot = CaptureSlot::Primary;
            }
            ui.add_space(8.0);
            ui.strong("Nach:");
            ui.horizontal(|ui| {
                ui.label("X:");
                ui.add(egui::DragValue::new(to_x));
                ui.label("Y:");
                ui.add(egui::DragValue::new(to_y));
            });
            if ui
                .button("📍  Nach hier erfassen")
                .on_hover_text("Position in 3 s aufnehmen")
                .clicked()
            {
                out.capture_position = true;
                out.capture_slot = CaptureSlot::Secondary;
            }
            ui.add_space(8.0);
            button_combo(ui, button);
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("Zieh-Dauer (s):");
                ui.add(
                    egui::DragValue::new(duration_sec)
                        .speed(0.05)
                        .clamp_range(0.0..=20.0),
                );
            });
        }

        Action::Typewrite {
            text,
            interval_sec,
        } => {
            ui.label("Text:");
            ui.add(
                egui::TextEdit::multiline(text)
                    .desired_rows(5)
                    .desired_width(f32::INFINITY),
            );
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label("Zeichen-Verzögerung (s):");
                ui.add(
                    egui::DragValue::new(interval_sec)
                        .speed(0.01)
                        .clamp_range(0.0..=2.0),
                );
            });
            ui.label(
                RichText::new("Für Enter/Tab/⌘ die Aktionen »Taste« und »Kombination« nutzen.")
                    .color(crate::theme::muted(ui))
                    .size(11.0),
            );
        }

        Action::KeyPress { key } => {
            ui.label("Taste:");
            egui::ComboBox::from_id_source("keypress_preset")
                .selected_text(key.label())
                .width(150.0)
                .show_ui(ui, |ui| {
                    for k in KeySpec::presets() {
                        ui.selectable_value(key, k.clone(), k.label());
                    }
                });
            ui.add_space(6.0);
            if ui
                .button("🔑  Taste aufnehmen …")
                .on_hover_text("Gewünschte Taste am Keyboard drücken")
                .clicked()
            {
                out.key_capture = Some((idx, false));
            }
            ui.label(
                RichText::new("Drückt die Taste kurz (wie pyautogui.press).").color(crate::theme::muted(ui)).size(11.0),
            );
        }

        Action::Hotkey { keys } => {
            ui.label("Kombination:");
            if keys.is_empty() {
                ui.label(RichText::new("Noch keine Tasten aufgenommen.").color(crate::theme::muted(ui)));
            } else {
                ui.label(RichText::new(combo_label(keys)).strong());
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui
                    .button("🔑  Kombination aufnehmen …")
                    .on_hover_text("Tastenkombination drücken, z. B. ⌘+C")
                    .clicked()
                {
                    out.key_capture = Some((idx, true));
                }
                if ui
                    .add_enabled(!keys.is_empty(), egui::Button::new("✕ Leeren"))
                    .clicked()
                {
                    keys.clear();
                }
            });
            ui.label(
                RichText::new("Beispiel: Strg + C (wie pyautogui.hotkey).").color(crate::theme::muted(ui)).size(11.0),
            );
        }

        Action::Scroll { amount } => {
            ui.horizontal(|ui| {
                ui.label("Menge:");
                ui.add(egui::DragValue::new(amount).clamp_range(-100..=100));
            });
            ui.label(
                RichText::new("Positiv = nach oben (wie pyautogui.scroll(n)).").color(crate::theme::muted(ui)).size(11.0),
            );
        }

        Action::Sleep { duration_sec } => {
            ui.horizontal(|ui| {
                ui.label("Sekunden:");
                ui.add(
                    egui::DragValue::new(duration_sec)
                        .speed(0.1)
                        .clamp_range(0.0..=600.0),
                );
            });
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
            // Bereich per Klick & Ziehen aufziehen (wie beim macOS-Screenshot)
            if ui
                .add(
                    egui::Button::new("🖱️  Bereich aufziehen").fill(theme::ACCENT),
                )
                .on_hover_text(
                    "Am Startpunkt klicken, den Bereich aufziehen, loslassen. \
                     Esc bricht ab.",
                )
                .clicked()
            {
                out.capture_image_drag = Some(idx);
            }
            ui.label(
                RichText::new("Beim Loslassen wird der Bereich sofort als Bild gespeichert. \
                    Das AutoMAUS-Fenster darf den Bereich nicht überdecken.")
                    .color(crate::theme::muted(ui))
                    .size(11.0),
            );

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(4.0);
            ui.strong("Bereich (wird beim Aufziehen gesetzt):");
            ui.horizontal(|ui| {
                ui.label("X1:");
                ui.add(egui::DragValue::new(x1));
                ui.label("Y1:");
                ui.add(egui::DragValue::new(y1));
            });
            ui.horizontal(|ui| {
                ui.label("X2:");
                ui.add(egui::DragValue::new(x2));
                ui.label("Y2:");
                ui.add(egui::DragValue::new(y2));
            });
            if ui
                .button("📸  Bereich erneut aufnehmen")
                .on_hover_text("Nimmt den Bereich mit diesen Koordinaten noch einmal auf")
                .clicked()
            {
                out.capture_image = Some(idx);
            }

            // Vorschau – während der Ausführung ausgeblendet, sonst findet die
            // Suche das Bild in der eigenen App wieder
            if running {
                ui.add_space(8.0);
                ui.label(
                    RichText::new("🔍 Vorschau während der Ausführung ausgeblendet – \
                        damit die Suche das Bild nicht im AutoMAUS-Fenster findet.")
                        .color(crate::theme::muted(ui))
                        .size(11.0),
                );
            } else if !image.is_empty() {
                ui.add_space(8.0);
                show_preview(ui, image, preview);
                ui.label(
                    RichText::new(image.clone())
                        .color(crate::theme::muted(ui))
                        .size(10.0),
                );
            } else {
                ui.add_space(4.0);
                ui.label(
                    RichText::new("Noch kein Bild aufgenommen.")
                        .color(crate::theme::muted(ui)),
                );
            }

            ui.add_space(10.0);
            ui.label("Nach dem Fund:");
            egui::ComboBox::from_id_source("find_mode")
                .selected_text(mode.label())
                .width(140.0)
                .show_ui(ui, |ui| {
                    for m in FindMode::all() {
                        ui.selectable_value(mode, m, m.label());
                    }
                });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("Such-Timeout (s):");
                ui.add(
                    egui::DragValue::new(timeout_sec)
                        .speed(0.5)
                        .clamp_range(1.0..=60.0),
                );
            });
            ui.horizontal(|ui| {
                ui.label("Bewegungsdauer (s):");
                ui.add(
                    egui::DragValue::new(duration_sec)
                        .speed(0.05)
                        .clamp_range(0.0..=20.0),
                );
            });
            ui.label(
                RichText::new("Sucht das Bild laufend auf dem Bildschirm und \
                    klickt/anfährt die Mitte des Funds.")
                    .color(crate::theme::muted(ui))
                    .size(11.0),
            );
        }

        Action::LoopStart { count } => {
            ui.horizontal(|ui| {
                ui.label("Wiederholungen:");
                ui.add(egui::DragValue::new(count).clamp_range(0..=1_000_000));
            });
            ui.add_space(4.0);
            if *count == 0 {
                ui.label(
                    RichText::new("0 = endlos – nur über Stop oder Failsafe beenden.")
                        .color(theme::DANGER)
                        .size(11.5),
                );
            } else {
                ui.label(RichText::new("Schleifen können verschachtelt werden.").color(crate::theme::muted(ui)).size(11.0));
            }
        }

        Action::LoopEnd => {
            ui.label(RichText::new("Markiert das Ende des Schleifen-Blocks.").color(crate::theme::muted(ui)));
        }
    }
}

/// Zeigt das gespeicherte Referenzbild als Vorschau (max. 220 px breit).
fn show_preview(
    ui: &mut egui::Ui,
    path: &str,
    cached: &mut Option<(String, egui::TextureHandle)>,
) {
    let hit = cached
        .as_ref()
        .map(|(p, _)| p.as_str() == path)
        .unwrap_or(false);

    let tex = if hit {
        cached.as_ref().map(|(_, t)| t.clone())
    } else {
        match image::open(path) {
            Ok(img) => {
                let rgba = img.to_rgba8();
                let handle = ui.ctx().load_texture(
                    "findimage_preview",
                    egui::ColorImage::from_rgba_unmultiplied(
                        [rgba.width() as usize, rgba.height() as usize],
                        rgba.as_raw(),
                    ),
                    egui::TextureOptions::NEAREST,
                );
                *cached = Some((path.to_string(), handle));
                cached.as_ref().map(|(_, t)| t.clone())
            }
            Err(_) => {
                ui.label(
                    RichText::new("⚠ Vorschau nicht ladbar (Bild fehlt?).")
                        .color(theme::DANGER)
                        .size(11.0),
                );
                None
            }
        }
    };

    if let Some(tex) = tex {
        let size = tex.size_vec2();
        let scale = (220.0 / size.x).min(1.0);
        let resp = ui.add(egui::Image::new(&tex).fit_to_exact_size(size * scale));
        resp.on_hover_text("Das Referenzbild, nach dem auf dem Bildschirm gesucht wird");
    }
}
