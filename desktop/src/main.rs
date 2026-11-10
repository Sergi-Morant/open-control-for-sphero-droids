// En Windows no se abre la consola en la versión de producción.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod backend;
mod catalog;
mod controllers;
mod droid;
mod gamepad;
mod kind;
mod protocol;
mod theme;
mod translate;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1100.0, 760.0]).with_min_inner_size([820.0, 560.0]).with_title("Open control for Sphero Droids"),
        ..Default::default()
    };
    eframe::run_native("Open control for Sphero Droids", options, Box::new(|cc| Ok(Box::new(app::App::new(cc)))))
}
