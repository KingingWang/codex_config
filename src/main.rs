//! Binary entry point. All logic lives in the `codex_config` library crate so
//! that the GUI can also be driven from tests.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use codex_config::App;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1360.0, 880.0])
            .with_min_inner_size([1000.0, 660.0])
            .with_title("Codex 配置助手"),
        ..Default::default()
    };
    eframe::run_native(
        "Codex 配置助手",
        options,
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
}
