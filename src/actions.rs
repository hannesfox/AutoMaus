//! Datenmodell der Aktionen (nach dem Vorbild von pyautogui).

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Maustaste
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub enum MouseButton {
    #[default]
    Left,
    Right,
    Middle,
}

impl MouseButton {
    pub fn label(self) -> &'static str {
        match self {
            MouseButton::Left => "Links",
            MouseButton::Right => "Rechts",
            MouseButton::Middle => "Mitte",
        }
    }

    pub fn all() -> [MouseButton; 3] {
        [MouseButton::Left, MouseButton::Right, MouseButton::Middle]
    }

    pub fn to_enigo(self) -> enigo::Button {
        match self {
            MouseButton::Left => enigo::Button::Left,
            MouseButton::Right => enigo::Button::Right,
            MouseButton::Middle => enigo::Button::Middle,
        }
    }
}

// ---------------------------------------------------------------------------
// Tasten (für KeyPress / Hotkey)
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub enum KeySpec {
    Enter,
    Tab,
    Escape,
    Space,
    Backspace,
    Delete,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    F(u8),
    Control,
    Shift,
    Alt,
    Meta,
    Char(char),
}

impl Default for KeySpec {
    fn default() -> Self {
        KeySpec::Enter
    }
}

impl KeySpec {
    pub fn label(&self) -> String {
        match self {
            KeySpec::Enter => "Enter".into(),
            KeySpec::Tab => "Tab".into(),
            KeySpec::Escape => "Esc".into(),
            KeySpec::Space => "Leertaste".into(),
            KeySpec::Backspace => "Rücktaste".into(),
            KeySpec::Delete => "Entf".into(),
            KeySpec::Up => "↑".into(),
            KeySpec::Down => "↓".into(),
            KeySpec::Left => "←".into(),
            KeySpec::Right => "→".into(),
            KeySpec::Home => "Pos1".into(),
            KeySpec::End => "Ende".into(),
            KeySpec::PageUp => "Bild auf".into(),
            KeySpec::PageDown => "Bild ab".into(),
            KeySpec::F(n) => format!("F{n}"),
            KeySpec::Control => "Strg".into(),
            KeySpec::Shift => "Umschalt".into(),
            KeySpec::Alt => "Option".into(),
            KeySpec::Meta => "Cmd".into(),
            KeySpec::Char(c) => c.to_uppercase().to_string(),
        }
    }

    pub fn to_enigo(&self) -> enigo::Key {
        match self {
            KeySpec::Enter => enigo::Key::Return,
            KeySpec::Tab => enigo::Key::Tab,
            KeySpec::Escape => enigo::Key::Escape,
            KeySpec::Space => enigo::Key::Space,
            KeySpec::Backspace => enigo::Key::Backspace,
            KeySpec::Delete => enigo::Key::Delete,
            KeySpec::Up => enigo::Key::UpArrow,
            KeySpec::Down => enigo::Key::DownArrow,
            KeySpec::Left => enigo::Key::LeftArrow,
            KeySpec::Right => enigo::Key::RightArrow,
            KeySpec::Home => enigo::Key::Home,
            KeySpec::End => enigo::Key::End,
            KeySpec::PageUp => enigo::Key::PageUp,
            KeySpec::PageDown => enigo::Key::PageDown,
            KeySpec::F(n) => match (*n).clamp(1, 12) {
                1 => enigo::Key::F1,
                2 => enigo::Key::F2,
                3 => enigo::Key::F3,
                4 => enigo::Key::F4,
                5 => enigo::Key::F5,
                6 => enigo::Key::F6,
                7 => enigo::Key::F7,
                8 => enigo::Key::F8,
                9 => enigo::Key::F9,
                10 => enigo::Key::F10,
                11 => enigo::Key::F11,
                _ => enigo::Key::F12,
            },
            KeySpec::Control => enigo::Key::Control,
            KeySpec::Shift => enigo::Key::Shift,
            KeySpec::Alt => enigo::Key::Alt,
            KeySpec::Meta => enigo::Key::Meta,
            KeySpec::Char(c) => enigo::Key::Unicode(*c),
        }
    }

    /// Häufig genutzte Tasten für die Auswahlliste im Inspektor.
    pub fn presets() -> Vec<KeySpec> {
        let mut v = vec![
            KeySpec::Enter,
            KeySpec::Tab,
            KeySpec::Escape,
            KeySpec::Space,
            KeySpec::Backspace,
            KeySpec::Delete,
            KeySpec::Up,
            KeySpec::Down,
            KeySpec::Left,
            KeySpec::Right,
            KeySpec::Home,
            KeySpec::End,
            KeySpec::PageUp,
            KeySpec::PageDown,
        ];
        v.extend((1..=12).map(KeySpec::F));
        v
    }
}

pub fn combo_label(keys: &[KeySpec]) -> String {
    if keys.is_empty() {
        "–".to_string()
    } else {
        keys.iter().map(|k| k.label()).collect::<Vec<_>>().join(" + ")
    }
}

// ---------------------------------------------------------------------------
// Bildsuche (»Auf Bild klicken«)
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub enum FindMode {
    /// Auf das gefundene Bild klicken
    #[default]
    Click,
    /// Auf das gefundene Bild doppelklicken
    DoubleClick,
    /// Die Maus nur darüber bewegen (anfahren)
    Move,
}

impl FindMode {
    pub fn label(self) -> &'static str {
        match self {
            FindMode::Click => "Klicken",
            FindMode::DoubleClick => "Doppelklick",
            FindMode::Move => "Anfahren",
        }
    }

    pub fn all() -> [FindMode; 3] {
        [FindMode::Click, FindMode::DoubleClick, FindMode::Move]
    }
}

// ---------------------------------------------------------------------------
// Aktionen
// ---------------------------------------------------------------------------

fn default_clicks() -> u32 {
    1
}

fn default_scroll() -> i32 {
    3
}

fn default_timeout() -> f64 {
    5.0
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(tag = "type")]
pub enum Action {
    Click {
        x: i32,
        y: i32,
        #[serde(default)]
        button: MouseButton,
        #[serde(default = "default_clicks")]
        clicks: u32,
        #[serde(default)]
        interval_sec: f64,
    },
    MoveTo {
        x: i32,
        y: i32,
        #[serde(default)]
        duration_sec: f64,
    },
    MoveRelative {
        #[serde(default)]
        dx: i32,
        #[serde(default)]
        dy: i32,
        #[serde(default)]
        duration_sec: f64,
    },
    Drag {
        #[serde(default)]
        from_x: i32,
        #[serde(default)]
        from_y: i32,
        #[serde(default)]
        to_x: i32,
        #[serde(default)]
        to_y: i32,
        #[serde(default)]
        button: MouseButton,
        #[serde(default)]
        duration_sec: f64,
    },
    Typewrite {
        text: String,
        #[serde(default)]
        interval_sec: f64,
    },
    KeyPress {
        #[serde(default)]
        key: KeySpec,
    },
    Hotkey {
        #[serde(default)]
        keys: Vec<KeySpec>,
    },
    Scroll {
        #[serde(default = "default_scroll")]
        amount: i32,
    },
    Sleep {
        duration_sec: f64,
    },
    /// Sucht ein gespeichertes Bild auf dem Bildschirm und klickt/fährt es an
    /// (wie pyautogui.locateOnScreen).
    FindImage {
        #[serde(default)]
        x1: i32,
        #[serde(default)]
        y1: i32,
        #[serde(default)]
        x2: i32,
        #[serde(default)]
        y2: i32,
        /// Pfad zum gespeicherten PNG (workflows/images/…)
        #[serde(default)]
        image: String,
        #[serde(default)]
        mode: FindMode,
        #[serde(default = "default_timeout")]
        timeout_sec: f64,
        #[serde(default)]
        duration_sec: f64,
    },
    LoopStart {
        count: u32,
    },
    LoopEnd,
}

/// Deutsche Zahl mit Komma (z. B. `1,0`).
pub fn secs(v: f64) -> String {
    format!("{v:.1}").replace('.', ",")
}

impl Action {
    pub fn icon(&self) -> &'static str {
        match self {
            Action::Click { .. } => "🖱",
            Action::MoveTo { .. } => "➡",
            Action::MoveRelative { .. } => "↔",
            Action::Drag { .. } => "✋",
            Action::Typewrite { .. } => "⌨",
            Action::KeyPress { .. } => "🔑",
            Action::Hotkey { .. } => "🎹",
            Action::Scroll { .. } => "📜",
            Action::Sleep { .. } => "⏳",
            Action::FindImage { .. } => "🔍",
            Action::LoopStart { .. } => "🔁",
            Action::LoopEnd => "🏁",
        }
    }

    pub fn label(&self) -> String {
        match self {
            Action::Click {
                x,
                y,
                button,
                clicks,
                ..
            } => {
                let kind = match (*button, *clicks) {
                    (MouseButton::Left, 1) => "Klick".to_string(),
                    (MouseButton::Left, 2) => "Doppelklick".to_string(),
                    (MouseButton::Left, n) => format!("{n}× Klick"),
                    (b, 1) => format!("{}-Klick", b.label()),
                    (b, n) => format!("{n}× {}-Klick", b.label()),
                };
                format!("{kind} ({x}, {y})")
            }
            Action::MoveTo {
                x,
                y,
                duration_sec,
            } => {
                let d = if *duration_sec > 0.0 {
                    format!(" · {} s", secs(*duration_sec))
                } else {
                    String::new()
                };
                format!("Maus bewegen ({x}, {y}){d}")
            }
            Action::MoveRelative {
                dx,
                dy,
                duration_sec,
            } => {
                let d = if *duration_sec > 0.0 {
                    format!(" · {} s", secs(*duration_sec))
                } else {
                    String::new()
                };
                format!("Maus relativ ({dx:+}, {dy:+}){d}")
            }
            Action::Drag {
                from_x,
                from_y,
                to_x,
                to_y,
                button,
                ..
            } => format!(
                "Ziehen {} ({from_x}, {from_y}) → ({to_x}, {to_y})",
                button.label().to_lowercase()
            ),
            Action::Typewrite { text, .. } => {
                let short: String = text
                    .replace('\n', " ⏎ ")
                    .chars()
                    .take(30)
                    .collect();
                if text.chars().count() > 30 {
                    format!("Tippen: '{short}…'")
                } else {
                    format!("Tippen: '{short}'")
                }
            }
            Action::KeyPress { key } => format!("Taste: {}", key.label()),
            Action::Hotkey { keys } => format!("Kombination: {}", combo_label(keys)),
            Action::Scroll { amount } => format!("Scrollen ({amount:+})"),
            Action::Sleep { duration_sec } => format!("Pause {} s", secs(*duration_sec)),
            Action::FindImage { image, mode, .. } => {
                let name = std::path::Path::new(image)
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default();
                if image.is_empty() {
                    format!("Bild finden → {}", mode.label())
                } else {
                    format!("Bild finden → {} ({name})", mode.label())
                }
            }
            Action::LoopStart { count } => {
                if *count == 0 {
                    "Schleife (endlos)".to_string()
                } else {
                    format!("Schleife ({count}×)")
                }
            }
            Action::LoopEnd => "Schleife Ende".to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// Validierung & Struktur-Hilfen
// ---------------------------------------------------------------------------

/// Prüft die Schleifen-Balance, bevor ein Ablauf gestartet wird.
pub fn validate_steps(steps: &[Action]) -> Result<(), String> {
    if steps.is_empty() {
        return Err("Der Ablauf enthält noch keine Schritte.".to_string());
    }
    let mut depth: i32 = 0;
    for (i, step) in steps.iter().enumerate() {
        match step {
            Action::LoopStart { .. } => depth += 1,
            Action::LoopEnd => {
                depth -= 1;
                if depth < 0 {
                    return Err(format!(
                        "Schritt {}: »Schleife Ende« ohne passenden Start.",
                        i + 1
                    ));
                }
            }
            _ => {}
        }
    }
    if depth > 0 {
        return Err("Es gibt einen Schleifen-Start ohne passendes Ende.".to_string());
    }
    Ok(())
}

/// Einrückungs-Tiefe je Schritt (für die Baum-Darstellung).
pub fn loop_depths(steps: &[Action]) -> Vec<usize> {
    let mut depths = Vec::with_capacity(steps.len());
    let mut depth: usize = 0;
    for step in steps {
        if matches!(step, Action::LoopEnd) {
            depth = depth.saturating_sub(1);
        }
        depths.push(depth);
        if matches!(step, Action::LoopStart { .. }) {
            depth += 1;
        }
    }
    depths
}
