// Joschys_Tree – Programmstart (eframe), Fenstertitel Joschys_Tree @ Joschy – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

#![windows_subsystem = "windows"]

mod app;
mod dialogs;
mod footer;
mod settings;
mod theme;
mod views;

/// Fenstertitel; Auftrag E ergänzt " – <Ziel>".
pub const WINDOW_TITLE: &str = "Joschys_Tree @ Joschy";

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title(WINDOW_TITLE)
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([640.0, 400.0]),
        ..Default::default()
    };
    eframe::run_native(
        WINDOW_TITLE,
        options,
        Box::new(|cc| Ok(Box::new(app::JtApp::new(cc)))),
    )
}
