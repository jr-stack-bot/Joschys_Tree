// Joschys_Tree – Dateitypen-Statistik und Alters-Histogramm – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

use eframe::egui::{self, Sense, Ui, vec2};
use egui_extras::{Column, TableBuilder};
use jt_core::format::{bytes, number};
use jt_core::stats::TypeStat;
use jt_core::{Key, tr};

use crate::app::AppState;
use crate::theme::{FileCat, Theme, bar_gradient, bar_level};
use crate::views::{cell, fmt_percent, no_tree};
use crate::widgets::{self, BarBatch, TableFx};

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

    let t = Theme::of(ui);
    // Alters-Histogramm
    widgets::section(ui, tr(lang, Key::AgeTitle), t.warn);
    let age_total: u64 = ts.ages.iter().sum::<u64>().max(1);
    ui.horizontal_wrapped(|ui| {
        for (k, v) in AGE_KEYS.iter().zip(ts.ages) {
            ui.vertical(|ui| {
                ui.set_width(138.0);
                cell(ui, tr(lang, *k), t.text2);
                let frac = v as f32 / age_total as f32;
                let (rect, _) = ui.allocate_exact_size(vec2(130.0, 10.0), Sense::hover());
                let p = ui.painter();
                p.rect_filled(rect, 0.0, t.row);
                let mut f = rect;
                f.set_width(rect.width() * frac);
                let (a, b) = bar_gradient(t, bar_level(frac));
                widgets::paint_gradient(p, f, a, b);
                p.rect_stroke(
                    rect,
                    0.0,
                    egui::Stroke::new(1.0, t.line),
                    egui::StrokeKind::Inside,
                );
                cell(
                    ui,
                    format!("{} ({})", bytes(v, lang), fmt_percent(frac, lang)),
                    t.text,
                );
            });
        }
    });
    ui.add_space(6.0);
    widgets::section(ui, tr(lang, Key::TypesTitle), t.accent);
    let stats = &ts.stats;
    let mut fx = TableFx::begin(ui, "jt_types_table");
    let mut bars = BarBatch::default();
    TableBuilder::new(ui)
        .id_salt("jt_types_table")
        .striped(true)
        .resizable(true)
        .sense(Sense::hover())
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::initial(140.0).clip(true))
        .column(Column::initial(80.0))
        .column(Column::initial(90.0))
        .column(Column::initial(90.0))
        .column(Column::remainder().at_least(100.0))
        .header(24.0, |mut h| {
            for k in [
                Key::ColExtension,
                Key::ColFiles,
                Key::ColSize,
                Key::ColAllocated,
                Key::ColPercent,
            ] {
                h.col(|ui| {
                    widgets::header_cell(ui, tr(lang, k), None);
                });
            }
        })
        .body(|body| {
            body.rows(22.0, stats.len(), |mut row| {
                let i = row.index();
                let s = &stats[i];
                row.col(|ui| {
                    fx.mark(ui, i as u64, false);
                    ui.add_space(6.0);
                    let (r, _) = ui.allocate_exact_size(vec2(8.0, 8.0), Sense::hover());
                    ui.painter()
                        .rect_filled(r, 0.0, FileCat::from_ext(&s.ext).color());
                    if s.ext.is_empty() {
                        cell(ui, tr(lang, Key::NoExtension), t.text2);
                    } else {
                        cell(ui, format!(".{}", s.ext), t.text);
                    }
                });
                row.col(|ui| {
                    cell(ui, number(s.files, lang), t.text2);
                });
                row.col(|ui| {
                    cell(ui, bytes(s.size, lang), t.text);
                });
                row.col(|ui| {
                    cell(ui, bytes(s.allocated, lang), t.text2);
                });
                row.col(|ui| {
                    widgets::percent_bar(ui, s.size as f32 / total as f32, lang, true, &mut bars);
                });
            });
        });
    bars.flush(ui);
    fx.end(ui);
}
