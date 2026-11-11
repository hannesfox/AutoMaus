# ⚡ AutoMAUS

**Desktop-Automatisierung im pyautogui-Stil – aufgenommen statt programmiert.**
Von Gschwendtner Johannes.

![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Windows-blue)
![Rust](https://img.shields.io/badge/Rust-2021-orange)
![Tests](https://img.shields.io/badge/tests-10%2F10-brightgreen)

AutoMAUS zeichnet Maus- und Tastatur-Aktionen zu wiederverwendbaren Abläufen
auf und spielt sie wieder. Geschrieben in **Rust** mit
[egui/eframe](https://github.com/emilk/egui) und
[enigo](https://github.com/enigo-rs/enigo) – inspiriert von Python
[`pyautogui`](https://pyautogui.readthedocs.io/).

## ✨ Features

- 🖱️ **Alle pyautogui-Aktionen**: Klicken, Bewegen, Ziehen, Scrollen, Tippen, Tasten, Hotkeys, Pausen – und verschachtelte Schleifen (0 = endlos)
- 🔍 **Bild finden**: Bereich am Bildschirm aufziehen (wie beim Screenshot-Tool), AutoMAUS sucht das Muster per präzisem Farb-Pixelvergleich – selbst sehr ähnliche Elemente (z. B. zwei gleiche Ordner-Icons) werden zuverlässig unterschieden, dann klicken / doppelklicken / anfahren
- 🛡️ **Sicher**: Failsafe wie bei pyautogui (Maus in eine Bildschirmecke = Sofort-Stopp), 🛑-Notbremse, `Esc`
- 🧭 **Übersichtlich**: Dashboard-Liste mit Suche, Drag-&-Drop-Schrittliste, Inspektor, hell/dunkel-Design im warmen Logo-Look

## 🚀 Starten

```bash
cargo run --release
```

> **macOS-Berechtigungen:** Für Maus/Tastatur *Bedienungshilfen* erlauben
> (Systemeinstellungen → Datenschutz & Sicherheit), für 🔍 Bild finden zusätzlich
> *Bildschirmaufzeichnung*.

## ⬇️ Fertige Pakete

Bei jedem Release-Tag (`v*`) baut GitHub Actions automatisch:
**Windows** (`.exe` als Zip) und **macOS** (Universal-`.app` für Apple Silicon + Intel, mit App-Icon).

## ⌨️ Tastenkürzel

| Kürzel | Aktion |
|---|---|
| `Strg+S` / `⌘S` | Speichern |
| `F5` | Ablauf starten |
| `Esc` | Ausführung stoppen |
| `Strg+N` | Neuer Ablauf |

## 🧪 Tests

```bash
cargo test
```

Abläufe liegen als JSON unter `workflows/`, Referenzbilder unter `workflows/images/`.
