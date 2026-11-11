//! AutoMAUS by Gschwendtner Johannes – Desktop-Automatisierung (pyautogui-Stil, geschrieben in Rust/egui).

mod actions;
mod app;
mod imgmatch;
mod state;
mod theme;
mod ui_dashboard;
mod ui_editor;
mod ui_modals;
mod worker;
mod workflow;

use eframe::egui;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1120.0, 700.0])
            .with_min_inner_size([880.0, 560.0]),
        ..Default::default()
    };

    eframe::run_native(
        "AutoMAUS by Gschwendtner Johannes",
        options,
        Box::new(|cc| Box::new(app::WorkflowApp::new(&cc.egui_ctx))),
    )
}
