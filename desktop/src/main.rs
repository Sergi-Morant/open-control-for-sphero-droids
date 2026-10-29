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
mod translate;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([980.0, 720.0]).with_title("Sphero droids open control"),
        ..Default::default()
    };
    eframe::run_native("Sphero droids open control", options, Box::new(|cc| Ok(Box::new(app::App::new(cc)))))
}
