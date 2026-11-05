// Joschys_Tree – Suche und Filter-Panel – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

use eframe::egui::{self, Sense, Ui};
use egui_extras::{Column, TableBuilder};
use jt_core::filter::{AgeField, Filter};
use jt_core::format::bytes;
use jt_core::{Key, NodeId, tr};

use crate::app::{AppState, SEARCH_LIMIT, fill};
use crate::theme::Theme;
use crate::views::{cell, fmt_time, no_tree, row_interaction};
use crate::widgets::{self, TableFx};

/// Eingabefelder als Text, damit leere Felder "kein Filter" bedeuten.
#[derive(Default)]
pub struct SearchState {
    pub name: String,
    pub min_mib: String,
    pub max_mib: String,
    pub older_days: String,
    pub newer_days: String,
    pub age_field: AgeField,
    pub exts: String,
    pub files_only: bool,
    pub include_hidden: bool,
    pub results: Vec<NodeId>,
    pub searched: bool,
}

impl SearchState {
    /// Baut den Kern-Filter aus den Eingaben (Größen in MiB, Kommazahl erlaubt).
    pub fn to_filter(&self) -> Filter {
        let mib = |s: &str| -> Option<u64> {
            let v: f64 = s.trim().replace(',', ".").parse().ok()?;
            (v >= 0.0).then_some((v * 1024.0 * 1024.0) as u64)
        };
        let days = |s: &str| s.trim().parse::<u32>().ok();
        let name = self.name.trim();
        Filter {
            name_glob: (!name.is_empty()).then(|| name.to_string()),
            min_size: mib(&self.min_mib),
            max_size: mib(&self.max_mib),
            older_than_days: days(&self.older_days),
            newer_than_days: days(&self.newer_days),
            age_field: self.age_field,
            extensions: self
                .exts
                .split([',', ';', ' '])
                .map(|e| e.trim().trim_start_matches('.').to_lowercase())
                .filter(|e| !e.is_empty())
                .collect(),
            files_only: self.files_only,
            include_hidden: self.include_hidden,
        }
    }
}

pub fn show(ui: &mut Ui, state: &mut AppState) {
    let lang = state.lang();
    let Some(tree) = state.tree.clone() else {
        no_tree(ui, lang);
        return;
    };
    let now = state.now;
    let s = &mut state.search;
    let mut run = false;
    egui::Grid::new("jt_search_grid")
        .num_columns(4)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            ui.label(tr(lang, Key::FltName));
            run |= enter(ui.text_edit_singleline(&mut s.name), ui);
            ui.label(tr(lang, Key::FltExtensions));
            run |= enter(ui.text_edit_singleline(&mut s.exts), ui);
            ui.end_row();
            ui.label(format!(
                "{} ({})",
                tr(lang, Key::FltMinSize),
                tr(lang, Key::UnitMibSuffix).trim()
            ));
            run |= enter(ui.text_edit_singleline(&mut s.min_mib), ui);
            ui.label(format!(
                "{} ({})",
                tr(lang, Key::FltMaxSize),
                tr(lang, Key::UnitMibSuffix).trim()
            ));
            run |= enter(ui.text_edit_singleline(&mut s.max_mib), ui);
            ui.end_row();
            ui.label(tr(lang, Key::FltOlderThan));
            run |= enter(ui.text_edit_singleline(&mut s.older_days), ui);
            ui.label(tr(lang, Key::FltNewerThan));
            run |= enter(ui.text_edit_singleline(&mut s.newer_days), ui);
            ui.end_row();
            ui.label(tr(lang, Key::FltAgeField));
            ui.horizontal(|ui| {
                widgets::choice(
                    ui,
                    &mut s.age_field,
                    AgeField::Modified,
                    tr(lang, Key::FltAgeModified),
                );
                widgets::choice(
                    ui,
                    &mut s.age_field,
                    AgeField::Accessed,
                    tr(lang, Key::FltAgeAccessed),
                );
                widgets::choice(
                    ui,
                    &mut s.age_field,
                    AgeField::Created,
                    tr(lang, Key::FltAgeCreated),
                );
            });
            ui.checkbox(&mut s.files_only, tr(lang, Key::FltFilesOnly));
            ui.checkbox(&mut s.include_hidden, tr(lang, Key::FltIncludeHidden));
            ui.end_row();
        });
    ui.horizontal(|ui| {
        if widgets::primary_button(ui, tr(lang, Key::FltApply), true).clicked() {
            run = true;
        }
        if widgets::tool_button(ui, tr(lang, Key::FltClear), true).clicked() {
            *s = SearchState::default();
        }
        if s.searched {
            ui.colored_label(
                Theme::of(ui).accent_text,
                fill(
                    tr(lang, Key::FltResults),
                    &[(
                        "{n}",
                        &jt_core::format::number(s.results.len() as u64, lang),
                    )],
                ),
            );
            if s.results.len() >= SEARCH_LIMIT {
                ui.colored_label(ui.visuals().warn_fg_color, tr(lang, Key::FltLimitReached));
            }
        }
    });
    if run {
        let f = s.to_filter();
        s.results = jt_core::filter::search(&tree, tree.root(), &f, now, SEARCH_LIMIT);
        s.searched = true;
    }
    ui.add_space(4.0);
    let t = Theme::of(ui);
    let results = &s.results;
    let selected = state.selected;
    let mut cx = crate::menu_cx!(state, &tree);
    let mut fx = TableFx::begin(ui, "jt_search_table");
    TableBuilder::new(ui)
        .id_salt("jt_search_table")
        .striped(true)
        .resizable(true)
        .sense(Sense::click())
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::initial(200.0).clip(true))
        .column(Column::initial(90.0))
        .column(Column::initial(130.0))
        .column(Column::remainder().at_least(200.0).clip(true))
        .header(24.0, |mut h| {
            for k in [Key::ColName, Key::ColSize, Key::ColModified, Key::ColPath] {
                h.col(|ui| {
                    widgets::header_cell(ui, tr(lang, k), None);
                });
            }
        })
        .body(|body| {
            body.rows(22.0, results.len(), |mut row| {
                let id = results[row.index()];
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
                    cell(ui, &*n.name, main);
                });
                row.col(|ui| {
                    cell(ui, bytes(n.size, lang), main);
                });
                row.col(|ui| {
                    cell(ui, fmt_time(n.modified, lang), weak);
                });
                row.col(|ui| {
                    let prot = cx.prot.protection(&tree, id);
                    crate::views::prot_badge(ui, prot, cx.basket.contains(id), lang);
                    cell(ui, tree.full_path(id), weak);
                });
                row_interaction(&row.response(), &mut cx, id);
            });
        });
    fx.end(ui);
}

fn enter(resp: egui::Response, ui: &Ui) -> bool {
    resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_from_inputs() {
        let s = SearchState {
            name: " *.jpg ".into(),
            min_mib: "1,5".into(),
            exts: ".JPG, png;gif".into(),
            older_days: "30".into(),
            newer_days: "x".into(),
            ..Default::default()
        };
        let f = s.to_filter();
        assert_eq!(f.name_glob.as_deref(), Some("*.jpg"));
        assert_eq!(f.min_size, Some(1_572_864));
        assert_eq!(f.max_size, None);
        assert_eq!(f.extensions, vec!["jpg", "png", "gif"]);
        assert_eq!(f.older_than_days, Some(30));
        assert_eq!(f.newer_than_days, None);
    }
}
