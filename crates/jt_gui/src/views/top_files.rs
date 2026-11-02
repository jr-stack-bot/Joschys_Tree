// Joschys_Tree – Top-100-Dateien – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

use eframe::egui::{self, Sense, Ui};
use egui_extras::{Column, TableBuilder};
use jt_core::format::bytes;
use jt_core::{Key, NodeId, tr};

use crate::app::AppState;
use crate::theme::Theme;
use crate::views::{cell, fmt_time, no_tree, row_interaction};
use crate::widgets::{self, TableFx};

/// Zwischenspeicher der Top-100 (wird nur bei Baumänderung neu berechnet).
#[derive(Default)]
pub struct TopState {
    gen_: u64,
    ids: Vec<NodeId>,
}

pub fn show(ui: &mut Ui, state: &mut AppState) {
    let lang = state.lang();
    let allow_delete = state.deletion_allowed();
    let Some(tree) = state.tree.clone() else {
        no_tree(ui, lang);
        return;
    };
    if state.top.gen_ != state.tree_gen {
        state.top.ids = jt_core::stats::top_files(&tree, 100, None);
        state.top.gen_ = state.tree_gen;
    }
    let t = Theme::of(ui);
    widgets::section(ui, tr(lang, Key::TopTitle), t.accent);
    let ids = &state.top.ids;
    let selected = state.selected;
    let actions = &mut state.actions;
    let mut fx = TableFx::begin(ui, "jt_top_table");
    TableBuilder::new(ui)
        .id_salt("jt_top_table")
        .striped(true)
        .resizable(true)
        .sense(Sense::click())
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::initial(40.0))
        .column(Column::initial(90.0))
        .column(Column::initial(90.0))
        .column(Column::initial(130.0))
        .column(Column::remainder().at_least(200.0).clip(true))
        .header(24.0, |mut h| {
            for k in [
                Key::ColRank,
                Key::ColSize,
                Key::ColAllocated,
                Key::ColModified,
                Key::ColPath,
            ] {
                h.col(|ui| {
                    widgets::header_cell(ui, tr(lang, k), None);
                });
            }
        })
        .body(|body| {
            body.rows(22.0, ids.len(), |mut row| {
                let i = row.index();
                let id = ids[i];
                let n = tree.node(id);
                let is_sel = selected == Some(id);
                row.set_selected(is_sel);
                let (main, weak) = if is_sel {
                    (t.sel_text, t.sel_text)
                } else {
                    (t.text, t.text2)
                };
                row.col(|ui| {
                    fx.mark(ui, u64::from(id), is_sel);
                    ui.add_space(6.0);
                    cell(ui, (i + 1).to_string(), t.dim);
                });
                row.col(|ui| {
                    cell(ui, bytes(n.size, lang), main);
                });
                row.col(|ui| {
                    cell(ui, bytes(n.allocated, lang), weak);
                });
                row.col(|ui| {
                    cell(ui, fmt_time(n.modified, lang), weak);
                });
                row.col(|ui| {
                    cell(ui, tree.full_path(id), main);
                });
                row_interaction(&row.response(), lang, &tree, id, actions, allow_delete);
            });
        });
    fx.end(ui);
}
