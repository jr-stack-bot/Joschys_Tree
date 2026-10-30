// Joschys_Tree – Statuszeile und Fußzeile mit Branding – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

use eframe::egui;

use crate::app::{AppState, can_delete};

/// Fußzeile: "Joschys_Tree @ Joschy · Admin/Nur-Lesen · Scan-Modus".
pub fn show(ui: &mut egui::Ui, state: &AppState) {
    let rights = if can_delete() { "Admin" } else { "Nur-Lesen" };
    let mode = match state.tree.as_ref().map(|t| t.meta.mode) {
        Some(jt_core::ScanMode::Mft) => "MFT",
        Some(jt_core::ScanMode::Walk) => "Walk",
        None => "–",
    };
    ui.horizontal(|ui| {
        ui.label(format!("Joschys_Tree {} · {rights} · {mode}", jt_core::BRANDING));
        if !state.status.is_empty() {
            ui.separator();
            ui.label(&state.status);
        }
    });
}
