//! wasm-poke: Interactive WebAssembly function size explorer.
//!
//! This is the main entry point for the egui-based GUI application.

use eframe::egui;

mod gui;

use gui::WasmPokeApp;

fn main() -> eframe::Result<()> {
    env_logger::init();

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([800.0, 600.0]),
        ..Default::default()
    };

    eframe::run_native(
        "wasm-poke",
        native_options,
        Box::new(|cc| Ok(Box::new(WasmPokeApp::new(cc)))),
    )
}
