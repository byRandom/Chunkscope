//! Chunkscope: an interactive seed map for Minecraft: Java Edition.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod biome_colors;
mod dimension;
mod finder_ui;
mod icons;
mod structures;
mod theme;
mod tiles;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Chunkscope")
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([960.0, 600.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Chunkscope",
        options,
        Box::new(|cc| Ok(Box::new(app::ChunkscopeApp::new(cc)))),
    )
}
