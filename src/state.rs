//! GUI-Zustände und die Aktionen, die ein Frame erzeugen kann.

use crate::actions::Action;
use crate::workflow::Workflow;
use std::path::PathBuf;

pub enum AppView {
    Dashboard,
    Editor(EditorState),
}

pub struct EditorState {
    pub path: Option<PathBuf>,
    pub name: String,
    pub workflow: Workflow,
    pub selected: Option<usize>,
    pub tab: InspectorTab,
    pub drag_from: Option<usize>,
}

impl EditorState {
    pub fn new_unsaved() -> Self {
        Self {
            path: None,
            name: String::new(),
            workflow: Workflow::default(),
            selected: None,
            tab: InspectorTab::Settings,
            drag_from: None,
        }
    }

    pub fn from_file(path: PathBuf, workflow: Workflow) -> Self {
        let name = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        Self {
            path: Some(path),
            name,
            workflow,
            selected: None,
            tab: InspectorTab::Settings,
            drag_from: None,
        }
    }
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum InspectorTab {
    Step,
    Settings,
}

/// Was die Positionserfassung erfassen soll (Feld des ausgewählten Schritts).
#[derive(PartialEq, Eq, Clone, Copy, Debug, Default)]
pub enum CaptureSlot {
    /// Klick/Bewegen: X, Y – oder Drag: Von-Ecke
    #[default]
    Primary,
    /// Drag: Nach-Ecke
    Secondary,
}

/// Die Taste, die gerade aufgenommen werden soll.
pub struct KeyCapture {
    pub step: usize,
    pub hotkey: bool,
}

/// Alles, was die UI-Functions an das App-Objekt zurückmelden.
#[derive(Default)]
pub struct FrameOutcome {
    pub new_workflow: bool,
    pub open_path: Option<PathBuf>,
    pub start_path: Option<PathBuf>,
    pub delete_path: Option<PathBuf>,
    pub back: bool,
    pub save: bool,
    pub start_editor: bool,
    pub stop: bool,
    pub toggle_theme: bool,
    pub add_step: Option<Action>,
    pub delete_step: Option<usize>,
    pub duplicate_step: Option<usize>,
    pub move_up: Option<usize>,
    pub move_down: Option<usize>,
    pub drop_step: Option<(usize, usize)>,
    pub capture_position: bool,
    pub capture_slot: CaptureSlot,
    /// Bereich als PNG aufnehmen (Schritt-Index)
    pub capture_image: Option<usize>,
    /// Bereich per Klick & Ziehen aufziehen (Schritt-Index)
    pub capture_image_drag: Option<usize>,
    pub key_capture: Option<(usize, bool)>,
    pub select_step: Option<usize>,
}
