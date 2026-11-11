//! Workflow-Struktur, Speicherung und Datei-Verwaltung.

use crate::actions::{validate_steps, Action};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub const WORKFLOW_DIR: &str = "workflows";

fn default_start_delay() -> f64 {
    1.0
}

fn default_step_delay() -> f64 {
    0.1
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct Workflow {
    /// Verzögerung vor dem ersten Schritt (wie `pyautogui.PAUSE` beim Start).
    #[serde(default = "default_start_delay")]
    pub start_delay: f64,
    /// Pause zwischen zwei Aktionen (wie `pyautogui.PAUSE`).
    #[serde(default = "default_step_delay")]
    pub step_delay: f64,
    #[serde(default)]
    pub steps: Vec<Action>,
}

impl Default for Workflow {
    fn default() -> Self {
        Self {
            start_delay: default_start_delay(),
            step_delay: default_step_delay(),
            steps: vec![],
        }
    }
}

impl Workflow {
    pub fn validate(&self) -> Result<(), String> {
        validate_steps(&self.steps)
    }
}

// ---------------------------------------------------------------------------
// Dateien
// ---------------------------------------------------------------------------

pub struct FileEntry {
    pub path: PathBuf,
    pub name: String,
    pub modified: Option<SystemTime>,
    pub step_count: usize,
}

pub fn list_workflows() -> Vec<FileEntry> {
    let _ = fs::create_dir_all(WORKFLOW_DIR);
    let mut out = Vec::new();
    if let Ok(entries) = fs::read_dir(WORKFLOW_DIR) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let name = path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let modified = fs::metadata(&path).ok().and_then(|m| m.modified().ok());
            let step_count = fs::read_to_string(&path)
                .ok()
                .and_then(|s| serde_json::from_str::<Workflow>(&s).ok())
                .map(|w| w.steps.len())
                .unwrap_or(0);
            out.push(FileEntry {
                path,
                name,
                modified,
                step_count,
            });
        }
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

pub fn load_workflow(path: &Path) -> Result<Workflow, String> {
    let json = fs::read_to_string(path).map_err(|e| format!("Lesefehler: {e}"))?;
    serde_json::from_str(&json).map_err(|e| format!("Ungültiges JSON: {e}"))
}

pub fn save_workflow(path: &Path, workflow: &Workflow) -> Result<(), String> {
    let json = serde_json::to_string_pretty(workflow).map_err(|e| format!("{e}"))?;
    fs::write(path, json).map_err(|e| format!("Schreibfehler: {e}"))
}

pub fn delete_workflow(path: &Path) -> Result<(), String> {
    fs::remove_file(path).map_err(|e| format!("Löschen fehlgeschlagen: {e}"))
}

/// Macht einen Namen dateisystem-tauglich; leere Namen werden aufgefangen.
pub fn safe_file_name(name: &str) -> String {
    let cleaned: String = name
        .trim()
        .chars()
        .map(|c| {
            if matches!(
                c,
                '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '\n' | '\r' | '\t'
            ) {
                '_'
            } else {
                c
            }
        })
        .collect();
    let cleaned = cleaned.trim();
    if cleaned.is_empty() {
        "Unbenannt".to_string()
    } else {
        cleaned.to_string()
    }
}

pub fn relative_time(t: Option<SystemTime>) -> String {
    let Some(t) = t else {
        return "unbekannt".to_string();
    };
    let secs = SystemTime::now()
        .duration_since(t)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    match secs {
        0..=59 => "gerade eben".to_string(),
        60..=3599 => format!("vor {} Min.", secs / 60),
        3600..=86399 => format!("vor {} Std.", secs / 3600),
        86400..=604799 => format!("vor {} Tagen", secs / 86400),
        _ => format!("vor {} Wochen", secs / 604800),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::{Action, MouseButton};

    /// Alte Dateien (ohne die neuen Felder) müssen weiterhin laden.
    #[test]
    fn loads_legacy_json() {
        let legacy = r#"{
            "start_delay": 1.0,
            "step_delay": 0.1,
            "steps": [
                { "type": "Click", "x": 125, "y": 146 },
                { "type": "Sleep", "duration_sec": 0.5 },
                { "type": "Typewrite", "text": "Hallo" },
                { "type": "LoopStart", "count": 2 },
                { "type": "LoopEnd" }
            ]
        }"#;
        let wf: Workflow = serde_json::from_str(legacy).expect("Legacy-JSON muss laden");
        assert_eq!(wf.start_delay, 1.0);
        assert_eq!(wf.steps.len(), 5);
        match &wf.steps[0] {
            Action::Click {
                x,
                y,
                button,
                clicks,
                ..
            } => {
                assert_eq!((*x, *y), (125, 146));
                assert_eq!(*button, MouseButton::Left);
                assert_eq!(*clicks, 1);
            }
            other => panic!("Erwartet Click, bekam {other:?}"),
        }
        assert!(wf.validate().is_ok());
    }

    #[test]
    fn roundtrip_new_actions() {
        let wf = Workflow {
            start_delay: 2.0,
            step_delay: 0.2,
            steps: vec![
                Action::Click {
                    x: 1,
                    y: 2,
                    button: MouseButton::Right,
                    clicks: 2,
                    interval_sec: 0.1,
                },
                Action::Hotkey {
                    keys: vec![
                        crate::actions::KeySpec::Control,
                        crate::actions::KeySpec::Char('c'),
                    ],
                },
                Action::Scroll { amount: -5 },
                Action::FindImage {
                    x1: 10,
                    y1: 20,
                    x2: 110,
                    y2: 70,
                    image: "workflows/images/test.png".into(),
                    mode: crate::actions::FindMode::DoubleClick,
                    timeout_sec: 5.0,
                    duration_sec: 0.2,
                },
            ],
        };
        let json = serde_json::to_string_pretty(&wf).unwrap();
        let back: Workflow = serde_json::from_str(&json).unwrap();
        assert_eq!(wf, back);
    }

    #[test]
    fn validates_loop_balance() {
        let mut wf = Workflow::default();
        assert!(wf.validate().is_err()); // leer

        wf.steps.push(Action::LoopStart { count: 2 });
        assert!(wf.validate().is_err()); // kein Ende

        wf.steps.push(Action::LoopEnd);
        assert!(wf.validate().is_ok());

        wf.steps.push(Action::LoopEnd);
        assert!(wf.validate().is_err()); // zu viele Enden
    }

    #[test]
    fn loop_depths_indent_blocks() {
        use crate::actions::loop_depths;
        let steps = vec![
            Action::Sleep { duration_sec: 0.1 },
            Action::LoopStart { count: 2 },
            Action::LoopStart { count: 3 },
            Action::Sleep { duration_sec: 0.1 },
            Action::LoopEnd,
            Action::LoopEnd,
        ];
        assert_eq!(loop_depths(&steps), vec![0, 0, 1, 2, 1, 0]);
    }

    #[test]
    fn safe_names_are_filesystem_friendly() {
        assert_eq!(safe_file_name("  Mein/Ablauf  "), "Mein_Ablauf");
        assert_eq!(safe_file_name("   "), "Unbenannt");
        assert_eq!(safe_file_name("normaler name"), "normaler name");
    }
}
