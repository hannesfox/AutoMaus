//! Dialoge: Löschen bestätigen, ungespeicherte Änderungen, Taste aufnehmen.

use crate::actions::KeySpec;
use crate::state::KeyCapture;
use crate::theme;
use eframe::egui::{self, RichText};
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Löschen bestätigen
// ---------------------------------------------------------------------------

pub fn delete_confirm(ctx: &egui::Context, target: &mut Option<PathBuf>) -> Option<PathBuf> {
    let path = target.clone()?;
    let mut confirmed = false;
    let mut cancel = false;
    let name = path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    egui::Window::new("Ablauf löschen")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.add_space(4.0);
            ui.label(format!("Möchtest du »{name}« wirklich löschen?"));
            ui.label(RichText::new("Das lässt sich nicht rückgängig machen.").color(crate::theme::muted(ui)).size(11.0));
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                if ui
                    .add(egui::Button::new("🗑  Löschen").fill(theme::DANGER))
                    .on_hover_text("Endgültig entfernen")
                    .clicked()
                {
                    confirmed = true;
                }
                if ui.button("Abbrechen").clicked() {
                    cancel = true;
                }
            });
            ui.add_space(4.0);
        });

    if confirmed {
        *target = None;
        Some(path)
    } else {
        if cancel {
            *target = None;
        }
        None
    }
}

// ---------------------------------------------------------------------------
// Ungespeicherte Änderungen beim Zurück-Geht
// ---------------------------------------------------------------------------

pub enum UnsavedChoice {
    Discard,
    Save,
    Cancel,
}

pub fn unsaved_confirm(
    ctx: &egui::Context,
    open: &mut bool,
    name: &str,
) -> Option<UnsavedChoice> {
    if !*open {
        return None;
    }
    let mut choice = None;
    let display = if name.trim().is_empty() {
        "Unbenannt"
    } else {
        name
    };

    egui::Window::new("Ungespeicherte Änderungen")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.add_space(4.0);
            ui.label(format!("»{display}« hat Änderungen, die noch nicht gespeichert sind."));
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                if ui
                    .add(egui::Button::new("💾  Speichern").fill(theme::ACCENT))
                    .clicked()
                {
                    choice = Some(UnsavedChoice::Save);
                }
                if ui.button("Verwerfen").clicked() {
                    choice = Some(UnsavedChoice::Discard);
                }
                if ui.button("Abbrechen").clicked() {
                    choice = Some(UnsavedChoice::Cancel);
                }
            });
            ui.add_space(4.0);
        });

    if choice.is_some() {
        *open = false;
    }
    choice
}

// ---------------------------------------------------------------------------
// Taste / Tastenkombination aufnehmen
// ---------------------------------------------------------------------------

/// Liefert `(Schritt-Index, ist_hotkey, Tasten)`, wenn eine Aufnahme abgeschlossen ist.
pub fn key_capture_window(
    ctx: &egui::Context,
    target: &mut Option<KeyCapture>,
) -> Option<(usize, bool, Vec<KeySpec>)> {
    let (step, hotkey) = target.as_ref().map(|t| (t.step, t.hotkey))?;
    let mut captured: Option<Vec<KeySpec>> = None;
    let mut cancel = false;

    ctx.input(|i| {
        if i.key_pressed(egui::Key::Escape) {
            cancel = true;
            return;
        }
        let mut mods: Vec<KeySpec> = Vec::new();
        if i.modifiers.ctrl {
            mods.push(KeySpec::Control);
        }
        if i.modifiers.shift {
            mods.push(KeySpec::Shift);
        }
        if i.modifiers.alt {
            mods.push(KeySpec::Alt);
        }
        // Auf macOS spiegelt `command` auf anderen Systemen nur ctrl – daher nur hier.
        #[cfg(target_os = "macos")]
        if i.modifiers.command {
            mods.push(KeySpec::Meta);
        }

        let mut combo = mods.clone();
        for (k, spec) in key_map() {
            if i.key_pressed(k) {
                combo.push(spec);
                break;
            }
        }
        if combo.len() > mods.len() {
            captured = Some(combo);
        }
    });

    let mut close = false;
    egui::Window::new(if hotkey {
        "Tastenkombination aufnehmen"
    } else {
        "Taste aufnehmen"
    })
    .collapsible(false)
    .resizable(false)
    .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
    .show(ctx, |ui| {
        ui.add_space(4.0);
        ui.label("Drücke jetzt die gewünschte Taste(n) …");
        ui.add_space(6.0);
        ui.label(RichText::new("Esc = Abbrechen").color(crate::theme::muted(ui)));
        ui.add_space(14.0);
        if ui.button("Abbrechen").clicked() {
            close = true;
        }
        ui.add_space(4.0);
    });

    if cancel || close {
        *target = None;
        return None;
    }
    if let Some(keys) = captured {
        *target = None;
        return Some((step, hotkey, keys));
    }
    None
}

fn key_map() -> Vec<(egui::Key, KeySpec)> {
    use egui::Key;
    let mut v: Vec<(Key, KeySpec)> = Vec::new();

    const LETTERS: [Key; 26] = [
        Key::A,
        Key::B,
        Key::C,
        Key::D,
        Key::E,
        Key::F,
        Key::G,
        Key::H,
        Key::I,
        Key::J,
        Key::K,
        Key::L,
        Key::M,
        Key::N,
        Key::O,
        Key::P,
        Key::Q,
        Key::R,
        Key::S,
        Key::T,
        Key::U,
        Key::V,
        Key::W,
        Key::X,
        Key::Y,
        Key::Z,
    ];
    for (i, k) in LETTERS.into_iter().enumerate() {
        v.push((k, KeySpec::Char((b'a' + i as u8) as char)));
    }

    const DIGITS: [Key; 10] = [
        Key::Num0,
        Key::Num1,
        Key::Num2,
        Key::Num3,
        Key::Num4,
        Key::Num5,
        Key::Num6,
        Key::Num7,
        Key::Num8,
        Key::Num9,
    ];
    for (i, k) in DIGITS.into_iter().enumerate() {
        v.push((k, KeySpec::Char((b'0' + i as u8) as char)));
    }

    v.extend([
        (Key::Enter, KeySpec::Enter),
        (Key::Tab, KeySpec::Tab),
        (Key::Space, KeySpec::Space),
        (Key::Backspace, KeySpec::Backspace),
        (Key::Delete, KeySpec::Delete),
        (Key::ArrowUp, KeySpec::Up),
        (Key::ArrowDown, KeySpec::Down),
        (Key::ArrowLeft, KeySpec::Left),
        (Key::ArrowRight, KeySpec::Right),
        (Key::Home, KeySpec::Home),
        (Key::End, KeySpec::End),
        (Key::PageUp, KeySpec::PageUp),
        (Key::PageDown, KeySpec::PageDown),
    ]);

    const FKEYS: [Key; 12] = [
        Key::F1,
        Key::F2,
        Key::F3,
        Key::F4,
        Key::F5,
        Key::F6,
        Key::F7,
        Key::F8,
        Key::F9,
        Key::F10,
        Key::F11,
        Key::F12,
    ];
    for (i, k) in FKEYS.into_iter().enumerate() {
        v.push((k, KeySpec::F(i as u8 + 1)));
    }

    v
}
