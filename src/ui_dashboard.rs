//! Dashboard: Suche, Ablauf-Karten, Leer-Zustand.

use crate::state::FrameOutcome;
use crate::theme;
use crate::workflow::{relative_time, FileEntry};
use eframe::egui::{self, Align, Layout, RichText, Rounding, Sense, Vec2};

pub fn show(
    ui: &mut egui::Ui,
    files: &[FileEntry],
    search: &mut String,
    is_running: bool,
    out: &mut FrameOutcome,
) {
    // Kopfzeile: Titel, Suche, Neuer-Ablauf-Button
    ui.horizontal(|ui| {
        ui.heading("Abläufe");
        ui.add_space(12.0);
        ui.add(
            egui::TextEdit::singleline(search)
                .hint_text("🔍  Suchen …")
                .desired_width(220.0),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui
                .add_enabled(
                    !is_running,
                    egui::Button::new("➕  Neuer Ablauf").min_size(Vec2::new(140.0, 0.0)),
                )
                .on_hover_text("Strg+N")
                .clicked()
            {
                out.new_workflow = true;
            }
        });
    });
    ui.add_space(6.0);
    ui.separator();
    ui.add_space(8.0);

    let q = search.to_lowercase();
    let visible: Vec<&FileEntry> = files
        .iter()
        .filter(|f| q.is_empty() || f.name.to_lowercase().contains(&q))
        .collect();

    if visible.is_empty() {
        ui.add_space(50.0);
        ui.vertical_centered(|ui| {
            ui.label(RichText::new("📂").size(52.0));
            ui.add_space(12.0);
            ui.label(
                RichText::new(if files.is_empty() {
                    "Noch keine Abläufe"
                } else {
                    "Keine Treffer"
                })
                .size(17.0)
                .strong(),
            );
            ui.add_space(6.0);
            ui.label(
                RichText::new(if files.is_empty() {
                    "Erstelle mit »Neuer Ablauf« deinen ersten Automatisierungs-Ablauf."
                } else {
                    "Versuche einen anderen Suchbegriff."
                })
                .color(crate::theme::muted(ui)),
            );
        });
        return;
    }

    egui::ScrollArea::vertical()
        .id_source("workflow_list")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for entry in visible {
                row(ui, entry, is_running, out);
                ui.add_space(3.0);
                ui.separator();
            }
        });
}

/// Eine Ablauf-Zeile: Name + Info links, Aktionen rechts. Eine pro Zeile,
/// alles scrollbar, lange Namen werden mit … abgekürzt.
fn row(ui: &mut egui::Ui, entry: &FileEntry, is_running: bool, out: &mut FrameOutcome) {
    const H: f32 = 46.0;
    const BUTTONS_W: f32 = 290.0;

    let (rect, resp) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), H), Sense::hover());

    // Hover-Hintergrund
    if resp.hovered() {
        ui.painter().rect_filled(
            rect,
            Rounding::same(6.0),
            ui.visuals().widgets.hovered.bg_fill,
        );
    }

    ui.allocate_ui_at_rect(rect, |ui| {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.set_max_width((rect.width() - BUTTONS_W).max(80.0));
                ui.add(
                    egui::Label::new(RichText::new(&entry.name).size(15.0).strong())
                        .wrap(false)
                        .truncate(true),
                );
                ui.add(
                    egui::Label::new(
                        RichText::new(format!(
                            "📋 {} Schritte · 🕑 {}",
                            entry.step_count,
                            relative_time(entry.modified)
                        ))
                        .size(11.5)
                        .color(crate::theme::muted(ui)),
                    )
                    .wrap(false)
                    .truncate(true),
                );
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .add_enabled(!is_running, egui::Button::new("🗑"))
                    .on_hover_text("Löschen")
                    .clicked()
                {
                    out.delete_path = Some(entry.path.clone());
                }
                if ui
                    .add_enabled(!is_running, egui::Button::new("✏  Bearbeiten"))
                    .clicked()
                {
                    out.open_path = Some(entry.path.clone());
                }
                if ui
                    .add_enabled(
                        !is_running,
                        egui::Button::new("▶  Start").fill(theme::ACCENT),
                    )
                    .on_hover_text("F5 im Editor")
                    .clicked()
                {
                    out.start_path = Some(entry.path.clone());
                }
            });
        });
    });
}

