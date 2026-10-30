// Joschys_Tree – Top-100-Dateien – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

use eframe::egui::{self, Sense, Ui};
use egui_extras::{Column, TableBuilder};
use jt_core::format::bytes;
use jt_core::{Key, NodeId, tr};

use crate::app::AppState;
use crate::views::{fmt_time, no_tree, row_interaction};

/// Zwischenspeicher der Top-100 (wird nur bei Baumänderung neu berechnet).
#[derive(Default)]
pub struct TopState {
    gen_: u64,
    ids: Vec<NodeId>,
}

pub fn show(ui: &mut Ui, state: &mut AppState) {
    let lang = state.lang();
    let Some(tree) = state.tree.clone() else {
        no_tree(ui, lang);
        return;
    };
    if state.top.gen_ != state.tree_gen {
        state.top.ids = jt_core::stats::top_files(&tree, 100, None);
        state.top.gen_ = state.tree_gen;
    }
    ui.heading(tr(lang, Key::TopTitle));
    let ids = &state.top.ids;
    let selected = state.selected;
    let actions = &mut state.actions;
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
        .header(22.0, |mut h| {
            for k in [
                Key::ColRank,
                Key::ColSize,
                Key::ColAllocated,
                Key::ColModified,
                Key::ColPath,
            ] {
                h.col(|ui| {
                    ui.strong(tr(lang, k));
                });
            }
        })
        .body(|body| {
            body.rows(20.0, ids.len(), |mut row| {
                let i = row.index();
                let id = ids[i];
                let n = tree.node(id);
                row.set_selected(selected == Some(id));
                row.col(|ui| {
                    ui.label((i + 1).to_string());
                });
                row.col(|ui| {
                    ui.label(bytes(n.size, lang));
                });
                row.col(|ui| {
                    ui.label(bytes(n.allocated, lang));
                });
                row.col(|ui| {
                    ui.label(fmt_time(n.modified, lang));
                });
                row.col(|ui| {
                    ui.label(tree.full_path(id));
                });
                row_interaction(&row.response(), lang, &tree, id, actions);
            });
        });
}
