// Joschys_Tree – Duplikat-Ergebnisse – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use eframe::egui::{self, Sense, Ui};
use egui_extras::{Column, TableBuilder};
use jt_core::duplicates::DupGroup;
use jt_core::format::{bytes, number};
use jt_core::{Key, NodeId, Tree, tr};

use crate::app::{Action, AppState, fill, is_attached};
use crate::theme::Theme;
use crate::views::{cell, no_tree, row_interaction};
use crate::widgets::{self, TableFx};

/// Laufende Duplikat-Suche.
pub struct DupRun {
    pub cancel: Arc<AtomicBool>,
    pub done: u64,
    pub total: u64,
}

/// Eine Tabellenzeile: Gruppenkopf oder Datei.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DupRow {
    Group(usize),
    File(usize, NodeId),
}

#[derive(Default)]
pub struct DupState {
    pub running: Option<DupRun>,
    pub groups: Vec<DupGroup>,
    pub searched: bool,
    rows: Vec<DupRow>,
    gen_: u64,
}

impl DupState {
    pub fn set_results(&mut self, groups: Vec<DupGroup>, gen_: u64) {
        self.groups = groups;
        self.searched = true;
        self.gen_ = gen_;
        self.rebuild_rows();
    }

    /// Nach dem Löschen: entfernte Dateien aus den Gruppen nehmen.
    pub fn retain_attached(&mut self, tree: &Tree) {
        for g in &mut self.groups {
            g.files.retain(|&id| is_attached(tree, id));
        }
        self.groups.retain(|g| g.files.len() > 1);
        self.rebuild_rows();
    }

    fn rebuild_rows(&mut self) {
        self.rows.clear();
        for (gi, g) in self.groups.iter().enumerate() {
            self.rows.push(DupRow::Group(gi));
            self.rows
                .extend(g.files.iter().map(|&f| DupRow::File(gi, f)));
        }
    }
}

pub fn show(ui: &mut Ui, state: &mut AppState) {
    let lang = state.lang();
    let Some(tree) = state.tree.clone() else {
        no_tree(ui, lang);
        return;
    };
    let ds = &mut state.dups;
    let mut min_mib = state.settings.dup_min_size / (1024 * 1024);
    ui.horizontal(|ui| {
        ui.label(tr(lang, Key::DupMinSize));
        let changed = ui
            .add(
                egui::DragValue::new(&mut min_mib)
                    .range(0..=1_000_000)
                    .suffix(tr(lang, Key::UnitMibSuffix)),
            )
            .changed();
        if changed {
            state.settings.dup_min_size = min_mib * 1024 * 1024;
        }
        match &ds.running {
            Some(run) => {
                widgets::busy(ui);
                let p = if run.total == 0 {
                    0.0
                } else {
                    run.done as f32 / run.total as f32
                };
                let text = fill(
                    tr(lang, Key::DupProgress),
                    &[
                        ("{done}", &number(run.done, lang)),
                        ("{total}", &number(run.total, lang)),
                    ],
                );
                widgets::progress(ui, p, &text, 200.0);
                if widgets::danger_button(ui, tr(lang, Key::Cancel), true).clicked() {
                    state.actions.push(Action::CancelDuplicates);
                }
            }
            None => {
                if widgets::primary_button(ui, tr(lang, Key::DupStart), true).clicked() {
                    state.actions.push(Action::StartDuplicates);
                }
            }
        }
    });
    if ds.gen_ != state.tree_gen && ds.searched && ds.running.is_none() {
        // Baum hat sich geändert (z. B. Löschen): Zeilen bereits angepasst, nur Generation merken.
        ds.gen_ = state.tree_gen;
    }
    if ds.searched {
        let savings: u64 = ds.groups.iter().map(DupGroup::savings).sum();
        ui.horizontal(|ui| {
            if ds.groups.is_empty() {
                ui.label(tr(lang, Key::DupNone));
            } else {
                ui.label(fill(
                    tr(lang, Key::DupGroups),
                    &[("{n}", &number(ds.groups.len() as u64, lang))],
                ));
                ui.separator();
                ui.colored_label(
                    Theme::of(ui).accent_text,
                    fill(
                        tr(lang, Key::DupSavings),
                        &[("{size}", &bytes(savings, lang))],
                    ),
                );
            }
        });
    }
    ui.add_space(4.0);
    let t = Theme::of(ui);
    let rows = &ds.rows;
    let groups = &ds.groups;
    let selected = state.selected;
    let mut cx = crate::menu_cx!(state, &tree);
    let mut fx = TableFx::begin(ui, "jt_dup_table");
    TableBuilder::new(ui)
        .id_salt("jt_dup_table")
        .striped(true)
        .resizable(true)
        .sense(Sense::click())
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::initial(100.0))
        .column(Column::initial(100.0))
        .column(Column::remainder().at_least(200.0).clip(true))
        .header(24.0, |mut h| {
            for k in [Key::ColSize, Key::ColSavings, Key::ColPath] {
                h.col(|ui| {
                    widgets::header_cell(ui, tr(lang, k), None);
                });
            }
        })
        .body(|body| {
            body.rows(22.0, rows.len(), |mut row| match rows[row.index()] {
                DupRow::Group(gi) => {
                    let g = &groups[gi];
                    row.set_overline(gi > 0);
                    row.col(|ui| {
                        ui.add_space(6.0);
                        cell(ui, bytes(g.size, lang), t.text_hi);
                    });
                    row.col(|ui| {
                        cell(ui, bytes(g.savings(), lang), t.accent_text);
                    });
                    row.col(|ui| {
                        cell(
                            ui,
                            format!(
                                "{} {} · {} {}",
                                tr(lang, Key::ColGroup),
                                gi + 1,
                                g.files.len(),
                                tr(lang, Key::ColFiles)
                            ),
                            t.dim,
                        );
                    });
                }
                DupRow::File(_, id) => {
                    let is_sel = selected == Some(id);
                    row.set_selected(is_sel);
                    row.col(|ui| fx.mark(ui, u64::from(id), is_sel));
                    row.col(|_| {});
                    row.col(|ui| {
                        let prot = cx.prot.protection(&tree, id);
                        crate::views::prot_badge(ui, prot, cx.basket.contains(id), lang);
                        cell(
                            ui,
                            tree.full_path(id),
                            if is_sel { t.sel_text } else { t.text },
                        );
                    });
                    row_interaction(&row.response(), &mut cx, id);
                }
            });
        });
    fx.end(ui);
}

#[cfg(test)]
mod tests {
    use super::*;
    use jt_core::model::testutil::sample_tree;

    #[test]
    fn rows_and_retain() {
        let mut t = sample_tree();
        let a = t.find_path("C:\\Users\\readme.md").unwrap();
        let b = t.find_path("C:\\Users\\joschy\\notiz.txt").unwrap();
        let mut ds = DupState::default();
        ds.set_results(
            vec![DupGroup {
                size: 1,
                hash: 0,
                files: vec![a, b],
            }],
            1,
        );
        assert_eq!(ds.rows.len(), 3);
        crate::app::unlink_node(&mut t, b);
        ds.retain_attached(&t);
        assert!(ds.groups.is_empty());
        assert!(ds.rows.is_empty());
    }
}
