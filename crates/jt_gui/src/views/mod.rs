// Joschys_Tree – Ansichten der Tabs – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

use eframe::egui;

use crate::app::{AppState, Tab};
use crate::settings::Settings;

pub mod delta;
pub mod details;
pub mod duplicates;
pub mod file_types;
pub mod search;
pub mod top_files;
pub mod tree_view;
pub mod treemap;

/// Zeigt den aktiven Tab (Skelett).
pub fn show(ui: &mut egui::Ui, state: &mut AppState, settings: &Settings) {
    let _ = settings;
    ui.horizontal(|ui| {
        for (tab, label) in [
            (Tab::Tree, "Baum"),
            (Tab::Treemap, "Treemap"),
            (Tab::TopFiles, "Top 100"),
            (Tab::FileTypes, "Dateitypen"),
            (Tab::Search, "Suche"),
            (Tab::Duplicates, "Duplikate"),
            (Tab::Delta, "Vergleich/Verlauf"),
        ] {
            ui.selectable_value(&mut state.tab, tab, label);
        }
    });
    ui.separator();
    ui.label(jt_core::BRANDING);
}
