// Joschys_Tree – Dunkles/helles Theme und Treemap-Farbpalette – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

use eframe::egui;

/// Standard: dunkel.
pub fn apply(ctx: &egui::Context, dark: bool) {
    ctx.set_theme(if dark {
        egui::Theme::Dark
    } else {
        egui::Theme::Light
    });
}

/// Farbpalette für die Treemap (Auftrag E füllt sie).
#[allow(dead_code)]
pub fn treemap_palette() -> &'static [egui::Color32] {
    &[]
}
