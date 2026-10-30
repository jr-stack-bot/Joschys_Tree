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
use crate::views::{no_tree, row_interaction};

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
                ui.spinner();
                let p = if run.total == 0 {
                    0.0
                } else {
                    run.done as f32 / run.total as f32
                };
                ui.add(egui::ProgressBar::new(p).desired_width(200.0).text(fill(
                    tr(lang, Key::DupProgress),
                    &[
                        ("{done}", &number(run.done, lang)),
                        ("{total}", &number(run.total, lang)),
                    ],
                )));
                if ui.button(tr(lang, Key::Cancel)).clicked() {
                    state.actions.push(Action::CancelDuplicates);
                }
            }
            None => {
                if ui.button(tr(lang, Key::DupStart)).clicked() {
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
                ui.strong(fill(
                    tr(lang, Key::DupSavings),
                    &[("{size}", &bytes(savings, lang))],
                ));
            }
        });
    }
    ui.separator();
    let rows = &ds.rows;
    let groups = &ds.groups;
    let selected = state.selected;
    let actions = &mut state.actions;
    TableBuilder::new(ui)
        .id_salt("jt_dup_table")
        .striped(true)
        .resizable(true)
        .sense(Sense::click())
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::initial(100.0))
        .column(Column::initial(100.0))
        .column(Column::remainder().at_least(200.0).clip(true))
        .header(22.0, |mut h| {
            for k in [Key::ColSize, Key::ColSavings, Key::ColPath] {
                h.col(|ui| {
                    ui.strong(tr(lang, k));
                });
            }
        })
        .body(|body| {
            body.rows(20.0, rows.len(), |mut row| match rows[row.index()] {
                DupRow::Group(gi) => {
                    let g = &groups[gi];
                    row.col(|ui| {
                        ui.strong(bytes(g.size, lang));
                    });
                    row.col(|ui| {
                        ui.strong(bytes(g.savings(), lang));
                    });
                    row.col(|ui| {
                        ui.strong(format!(
                            "{} {} · {} {}",
                            tr(lang, Key::ColGroup),
                            gi + 1,
                            g.files.len(),
                            tr(lang, Key::ColFiles)
                        ));
                    });
                }
                DupRow::File(_, id) => {
                    row.set_selected(selected == Some(id));
                    row.col(|_| {});
                    row.col(|_| {});
                    row.col(|ui| {
                        ui.label(tree.full_path(id));
                    });
                    row_interaction(&row.response(), lang, &tree, id, actions);
                }
            });
        });
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
