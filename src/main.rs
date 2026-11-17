#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod auth;
mod config;
mod install;
mod java;
mod launch;
mod loaders;
mod modrinth;
mod mojang;
mod net;
mod ui;
mod ui_dialogs;
mod ui_pages;

fn main() -> eframe::Result<()> {
    let opts = eframe::NativeOptions {
        viewport: egui_viewport(),
        ..Default::default()
    };
    eframe::run_native(
        "RLauncher",
        opts,
        Box::new(|cc| Ok(Box::new(app::App::new(cc)))),
    )
}

fn egui_viewport() -> eframe::egui::ViewportBuilder {
    eframe::egui::ViewportBuilder::default()
        .with_inner_size([440.0, 210.0])
        .with_decorations(false)
        .with_resizable(false)
        .with_position([600.0, 320.0])
        .with_title("RLauncher")
}
