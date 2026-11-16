//! Entry point of the Casualties Unknown save editor.
//!
//! Usage: `cu_save_editor [path-to-save]`. Without an argument the game's
//! save (`save.sv` in [`cu_save_editor::platform::save_dir`]) is opened if it
//! exists.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use cu_save_editor::app::EditorApp;

fn main() -> eframe::Result {
    cu_save_editor::diagnostics::install_panic_hook();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("CU Save Editor")
            .with_inner_size([1280.0, 820.0])
            .with_min_inner_size([900.0, 560.0]),
        centered: true,
        ..Default::default()
    };
    eframe::run_native(
        "Casualties Unknown Save Editor",
        options,
        Box::new(|cc| Ok(Box::new(EditorApp::new(cc)))),
    )
}
