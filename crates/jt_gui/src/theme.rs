// Joschys_Tree – Dunkles/helles Theme und Treemap-Farbpalette – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

use eframe::egui::{self, Color32};

/// Standard: dunkel.
pub fn apply(ctx: &egui::Context, dark: bool) {
    ctx.set_theme(if dark {
        egui::Theme::Dark
    } else {
        egui::Theme::Light
    });
}

/// Farbpalette für die Treemap (gut unterscheidbar in hell und dunkel).
pub fn treemap_palette() -> &'static [Color32] {
    const P: [Color32; 16] = [
        Color32::from_rgb(0x4e, 0x79, 0xa7),
        Color32::from_rgb(0xf2, 0x8e, 0x2b),
        Color32::from_rgb(0xe1, 0x57, 0x59),
        Color32::from_rgb(0x76, 0xb7, 0xb2),
        Color32::from_rgb(0x59, 0xa1, 0x4f),
        Color32::from_rgb(0xed, 0xc9, 0x48),
        Color32::from_rgb(0xb0, 0x7a, 0xa1),
        Color32::from_rgb(0xff, 0x9d, 0xa7),
        Color32::from_rgb(0x9c, 0x75, 0x5f),
        Color32::from_rgb(0xba, 0xb0, 0xac),
        Color32::from_rgb(0x2f, 0x9e, 0xd8),
        Color32::from_rgb(0x8c, 0xd1, 0x7d),
        Color32::from_rgb(0xd3, 0x72, 0xc0),
        Color32::from_rgb(0xc9, 0xa2, 0x27),
        Color32::from_rgb(0x6b, 0x8e, 0x23),
        Color32::from_rgb(0xa0, 0x52, 0x2d),
    ];
    &P
}

/// Stabile Farbe je Dateiendung (FNV-1a über die Endung).
pub fn color_for_ext(ext: &str) -> Color32 {
    if ext.is_empty() {
        return Color32::from_gray(0x80);
    }
    let mut h: u32 = 0x811c_9dc5;
    for b in ext.bytes() {
        h ^= u32::from(b.to_ascii_lowercase());
        h = h.wrapping_mul(0x0100_0193);
    }
    let p = treemap_palette();
    p[(h as usize) % p.len()]
}

/// Hintergrundfarbe für Ordner-Kacheln in der Treemap.
pub fn dir_color(dark: bool) -> Color32 {
    if dark {
        Color32::from_gray(0x30)
    } else {
        Color32::from_gray(0xd8)
    }
}

/// Farbe der Prozent-Balken.
pub fn bar_color(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(0x2f, 0x9e, 0xd8)
    } else {
        Color32::from_rgb(0x1f, 0x6f, 0xb0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ext_color_is_stable_and_case_insensitive() {
        assert_eq!(color_for_ext("JPG"), color_for_ext("jpg"));
        assert_eq!(color_for_ext(""), Color32::from_gray(0x80));
        assert_eq!(treemap_palette().len(), 16);
    }
}
