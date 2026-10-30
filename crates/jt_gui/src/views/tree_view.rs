// Joschys_Tree – Baumtabelle (virtuell, egui_extras::TableBuilder) – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashSet;

use eframe::egui::{self, Sense, Ui};
use egui_extras::{Column, TableBuilder};
use jt_core::aggregate::{SortKey, sorted_children};
use jt_core::format::{bytes, number};
use jt_core::{Key, NO_NODE, NodeId, Tree, tr};

use crate::app::{Action, AppState};
use crate::views::{age_days, fill_n, fmt_time, frac_of_parent, no_tree, percent_bar};

const ROW_H: f32 = 20.0;
const INDENT: f32 = 14.0;

/// Zustand der Baumtabelle. Die sichtbaren Zeilen werden nur bei Änderungen neu aufgebaut.
pub struct TreeViewState {
    pub expanded: HashSet<NodeId>,
    pub sort: SortKey,
    pub desc: bool,
    rows: Vec<(NodeId, u16)>,
    built_gen: u64,
    dirty: bool,
    scroll_to: Option<NodeId>,
}

impl Default for TreeViewState {
    fn default() -> Self {
        TreeViewState {
            expanded: HashSet::from([0]),
            sort: SortKey::Size,
            desc: true,
            rows: Vec::new(),
            built_gen: 0,
            dirty: true,
            scroll_to: None,
        }
    }
}

impl TreeViewState {
    pub fn invalidate(&mut self) {
        self.dirty = true;
    }

    /// Alle Vorfahren aufklappen und zur Zeile scrollen.
    pub fn reveal(&mut self, tree: &Tree, id: NodeId) {
        let mut p = tree.node(id).parent;
        while p != NO_NODE {
            self.expanded.insert(p);
            p = tree.node(p).parent;
        }
        self.scroll_to = Some(id);
        self.dirty = true;
    }

    fn toggle(&mut self, id: NodeId) {
        if !self.expanded.remove(&id) {
            self.expanded.insert(id);
        }
        self.dirty = true;
    }

    fn set_sort(&mut self, key: SortKey) {
        if self.sort == key {
            self.desc = !self.desc;
        } else {
            self.sort = key;
            self.desc = key != SortKey::Name;
        }
        self.dirty = true;
    }

    /// Flache Liste der sichtbaren Zeilen (Pre-Order, nur aufgeklappte Ordner).
    fn rebuild(&mut self, tree: &Tree, gen_: u64) {
        self.rows.clear();
        let mut stack: Vec<(NodeId, u16)> = vec![(tree.root(), 0)];
        while let Some((id, depth)) = stack.pop() {
            self.rows.push((id, depth));
            if tree.node(id).is_dir() && self.expanded.contains(&id) {
                let kids = sorted_children(tree, id, self.sort, self.desc);
                stack.extend(kids.into_iter().rev().map(|c| (c, depth.saturating_add(1))));
            }
        }
        self.built_gen = gen_;
        self.dirty = false;
    }

    #[cfg(test)]
    fn rows(&self) -> &[(NodeId, u16)] {
        &self.rows
    }
}

pub fn show(ui: &mut Ui, state: &mut AppState) {
    let lang = state.lang();
    let allow_delete = state.deletion_allowed();
    let Some(tree) = state.tree.clone() else {
        no_tree(ui, lang);
        return;
    };
    let tv = &mut state.tree_view;
    if tv.dirty || tv.built_gen != state.tree_gen {
        tv.rebuild(&tree, state.tree_gen);
    }
    let scroll_row = tv
        .scroll_to
        .take()
        .and_then(|id| tv.rows.iter().position(|(r, _)| *r == id));
    let now = state.now;
    let selected = state.selected;
    let actions = &mut state.actions;
    let mut toggles: Vec<NodeId> = Vec::new();
    let mut sort_click: Option<SortKey> = None;
    let (sort, desc) = (tv.sort, tv.desc);
    let rows = &tv.rows;
    let expanded = &tv.expanded;

    let mut table = TableBuilder::new(ui)
        .id_salt("jt_tree_table")
        .striped(true)
        .resizable(true)
        .sense(Sense::click())
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::initial(320.0).at_least(120.0).clip(true))
        .column(Column::initial(85.0))
        .column(Column::initial(85.0))
        .column(Column::initial(110.0))
        .column(Column::initial(75.0))
        .column(Column::initial(65.0))
        .column(Column::initial(120.0))
        .column(Column::initial(120.0))
        .column(Column::remainder().at_least(60.0));
    if let Some(r) = scroll_row {
        table = table.scroll_to_row(r, Some(egui::Align::Center));
    }
    table
        .header(ROW_H + 2.0, |mut h| {
            let mut head = |h: &mut egui_extras::TableRow, key: Key, sk: Option<SortKey>| {
                h.col(|ui| {
                    let mut text = tr(lang, key).to_string();
                    if sk == Some(sort) {
                        text.push_str(if desc { " ▼" } else { " ▲" });
                    }
                    match sk {
                        Some(k) => {
                            if ui.add(egui::Button::new(text).frame(false)).clicked() {
                                sort_click = Some(k);
                            }
                        }
                        None => {
                            ui.strong(text);
                        }
                    }
                });
            };
            head(&mut h, Key::ColName, Some(SortKey::Name));
            head(&mut h, Key::ColSize, Some(SortKey::Size));
            head(&mut h, Key::ColAllocated, Some(SortKey::Allocated));
            head(&mut h, Key::ColPercent, None);
            head(&mut h, Key::ColFiles, Some(SortKey::Files));
            head(&mut h, Key::ColFolders, None);
            head(&mut h, Key::ColModified, Some(SortKey::Modified));
            head(&mut h, Key::ColAccessed, None);
            head(&mut h, Key::ColAge, None);
        })
        .body(|body| {
            body.rows(ROW_H, rows.len(), |mut row| {
                let (id, depth) = rows[row.index()];
                let n = tree.node(id);
                row.set_selected(selected == Some(id));
                row.col(|ui| {
                    ui.add_space(depth as f32 * INDENT);
                    if n.is_dir() && n.first_child != NO_NODE {
                        let open = expanded.contains(&id);
                        if ui
                            .add(egui::Button::new(if open { "▾" } else { "▸" }).frame(false))
                            .clicked()
                        {
                            toggles.push(id);
                        }
                    } else {
                        ui.add_space(INDENT);
                    }
                    let icon = if n.is_dir() { "📁" } else { "📄" };
                    ui.label(format!("{icon} {}", n.name));
                });
                row.col(|ui| {
                    ui.label(bytes(n.size, lang));
                });
                row.col(|ui| {
                    ui.label(bytes(n.allocated, lang));
                });
                row.col(|ui| percent_bar(ui, frac_of_parent(&tree, id), lang));
                row.col(|ui| {
                    if n.is_dir() {
                        ui.label(number(u64::from(n.file_count), lang));
                    }
                });
                row.col(|ui| {
                    if n.is_dir() {
                        ui.label(number(u64::from(n.dir_count), lang));
                    }
                });
                row.col(|ui| {
                    ui.label(fmt_time(n.modified, lang));
                });
                row.col(|ui| {
                    ui.label(fmt_time(n.accessed, lang))
                        .on_hover_text(tr(lang, Key::DetAccessNote));
                });
                row.col(|ui| {
                    if let Some(d) = age_days(n, now) {
                        ui.label(fill_n(tr(lang, Key::AgeDays), d));
                    }
                });
                let resp = row.response();
                if resp.double_clicked() && n.is_dir() {
                    toggles.push(id);
                }
                if resp.clicked() {
                    actions.push(Action::Select(id));
                }
                resp.context_menu(|ui| {
                    if n.is_dir() && ui.button(tr(lang, Key::CtxShowInTreemap)).clicked() {
                        actions.push(Action::ShowInTreemap(id));
                        ui.close();
                    }
                    crate::views::node_menu(ui, lang, &tree, id, actions, false, allow_delete);
                });
            });
        });
    for id in toggles {
        tv.toggle(id);
    }
    if let Some(k) = sort_click {
        tv.set_sort(k);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jt_core::model::testutil::sample_tree;

    #[test]
    fn rows_follow_expansion_and_sort() {
        let t = sample_tree();
        let mut tv = TreeViewState::default();
        tv.rebuild(&t, 1);
        // Wurzel + 3 Kinder, nach Größe absteigend: pagefile, Windows, Users
        let names: Vec<&str> = tv.rows().iter().map(|(id, _)| &*t.node(*id).name).collect();
        assert_eq!(names, vec!["C:\\", "pagefile.sys", "Windows", "Users"]);
        let jpg = t.find_path("C:\\Users\\joschy\\Bild.JPG").unwrap();
        tv.reveal(&t, jpg);
        tv.rebuild(&t, 1);
        assert!(tv.rows().iter().any(|(id, d)| *id == jpg && *d == 3));
        tv.set_sort(SortKey::Name);
        tv.rebuild(&t, 1);
        let names: Vec<&str> = tv
            .rows()
            .iter()
            .take(3)
            .map(|(id, _)| &*t.node(*id).name)
            .collect();
        assert_eq!(names, vec!["C:\\", "pagefile.sys", "Users"]);
    }
}
