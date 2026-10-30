// Joschys_Tree – Dateitypen-Statistik und Alters-Histogramm – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

use eframe::egui::{self, Sense, Ui, vec2};
use egui_extras::{Column, TableBuilder};
use jt_core::format::{bytes, number};
use jt_core::stats::TypeStat;
use jt_core::{Key, tr};

use crate::app::AppState;
use crate::theme;
use crate::views::{fmt_percent, no_tree, percent_bar};

const AGE_KEYS: [Key; 6] = [
    Key::AgeLt1M,
    Key::AgeLt3M,
    Key::AgeLt6M,
    Key::AgeLt1Y,
    Key::AgeLt3Y,
    Key::AgeGe3Y,
];

#[derive(Default)]
pub struct TypesState {
    gen_: u64,
    stats: Vec<TypeStat>,
    ages: [u64; 6],
}

pub fn show(ui: &mut Ui, state: &mut AppState) {
    let lang = state.lang();
    let Some(tree) = state.tree.clone() else {
        no_tree(ui, lang);
        return;
    };
    let ts = &mut state.types;
    if ts.gen_ != state.tree_gen {
        ts.stats = jt_core::stats::file_types(&tree, tree.root());
        ts.ages = jt_core::stats::age_histogram(&tree, tree.root(), state.now);
        ts.gen_ = state.tree_gen;
    }
    let total = tree.node(tree.root()).size.max(1);

    // Alters-Histogramm
    ui.strong(tr(lang, Key::AgeTitle));
    let age_total: u64 = ts.ages.iter().sum::<u64>().max(1);
    ui.horizontal_wrapped(|ui| {
        for (k, v) in AGE_KEYS.iter().zip(ts.ages) {
            ui.vertical(|ui| {
                ui.set_width(130.0);
                ui.label(tr(lang, *k));
                let (rect, _) = ui.allocate_exact_size(vec2(120.0, 14.0), Sense::hover());
                let p = ui.painter();
                p.rect_filled(rect, 2.0, ui.visuals().extreme_bg_color);
                let mut f = rect;
                f.set_width(rect.width() * (v as f32 / age_total as f32));
                p.rect_filled(f, 2.0, theme::bar_color(ui.visuals().dark_mode));
                ui.small(format!(
                    "{} ({})",
                    bytes(v, lang),
                    fmt_percent(v as f32 / age_total as f32, lang)
                ));
            });
        }
    });
    ui.separator();
    ui.strong(tr(lang, Key::TypesTitle));
    let stats = &ts.stats;
    TableBuilder::new(ui)
        .id_salt("jt_types_table")
        .striped(true)
        .resizable(true)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::initial(140.0).clip(true))
        .column(Column::initial(80.0))
        .column(Column::initial(90.0))
        .column(Column::initial(90.0))
        .column(Column::remainder().at_least(100.0))
        .header(22.0, |mut h| {
            for k in [
                Key::ColExtension,
                Key::ColFiles,
                Key::ColSize,
                Key::ColAllocated,
                Key::ColPercent,
            ] {
                h.col(|ui| {
                    ui.strong(tr(lang, k));
                });
            }
        })
        .body(|body| {
            body.rows(20.0, stats.len(), |mut row| {
                let s = &stats[row.index()];
                row.col(|ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
                    ui.painter()
                        .rect_filled(r, 2.0, theme::color_for_ext(&s.ext));
                    if s.ext.is_empty() {
                        ui.label(tr(lang, Key::NoExtension));
                    } else {
                        ui.label(format!(".{}", s.ext));
                    }
                });
                row.col(|ui| {
                    ui.label(number(s.files, lang));
                });
                row.col(|ui| {
                    ui.label(bytes(s.size, lang));
                });
                row.col(|ui| {
                    ui.label(bytes(s.allocated, lang));
                });
                row.col(|ui| percent_bar(ui, s.size as f32 / total as f32, lang));
            });
        });
}
