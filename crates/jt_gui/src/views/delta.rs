// Joschys_Tree – Delta- und Verlaufsansicht – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

use eframe::egui::{self, Align2, Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use egui_extras::{Column, TableBuilder};
use jt_core::delta::{ChangeKind, DeltaEntry};
use jt_core::format::{bytes, number};
use jt_core::{Key, tr};

use crate::app::{Action, AppState, HistoryResult, fill};
use crate::icons::{self, NodeIcon};
use crate::theme::{Theme, with_alpha};
use crate::views::{cell, fmt_signed_bytes, fmt_time};
use crate::widgets::{self, TableFx};

pub struct DeltaState {
    pub show_history: bool,
    pub source: String,
    pub entries: Vec<DeltaEntry>,
    pub compared: bool,
    /// Netto-Änderung der Wurzel (neu - alt).
    pub net: i64,
    pub min_change_mib: u64,
    pub depth: u32,
    pub history: Option<HistoryResult>,
}

impl Default for DeltaState {
    fn default() -> Self {
        DeltaState {
            show_history: false,
            source: String::new(),
            entries: Vec::new(),
            compared: false,
            net: 0,
            min_change_mib: 1,
            depth: 2,
            history: None,
        }
    }
}

impl DeltaState {
    pub fn set_compare(&mut self, source: String, entries: Vec<DeltaEntry>, net: i64) {
        self.source = source;
        self.net = net;
        self.entries = entries;
        self.compared = true;
        self.show_history = false;
    }
}

pub fn kind_key(k: ChangeKind) -> Key {
    match k {
        ChangeKind::Added => Key::DeltaAdded,
        ChangeKind::Removed => Key::DeltaRemoved,
        ChangeKind::Grown => Key::DeltaGrown,
        ChangeKind::Shrunk => Key::DeltaShrunk,
    }
}

pub fn show(ui: &mut Ui, state: &mut AppState) {
    let lang = state.lang();
    let busy = state.io_busy.is_some();
    let has_tree = state.tree.is_some();
    let ds = &mut state.delta;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 3.0;
        if widgets::tab_button(ui, !ds.show_history, tr(lang, Key::DeltaTitle)).clicked() {
            ds.show_history = false;
        }
        if widgets::tab_button(ui, ds.show_history, tr(lang, Key::HistTitle)).clicked() {
            ds.show_history = true;
        }
    });
    ui.add_space(4.0);
    if !ds.show_history {
        ui.horizontal(|ui| {
            ui.label(tr(lang, Key::DeltaMinChange));
            ui.add(
                egui::DragValue::new(&mut ds.min_change_mib)
                    .range(0..=1_000_000)
                    .suffix(tr(lang, Key::UnitMibSuffix)),
            );
            if widgets::primary_button(ui, tr(lang, Key::DeltaChooseOld), has_tree && !busy)
                .clicked()
            {
                state.actions.push(Action::ChooseCompare);
            }
            if !ds.source.is_empty() {
                ui.label(&ds.source);
            }
        });
        if !has_tree {
            ui.label(tr(lang, Key::ErrNoScan));
            return;
        }
        if ds.compared {
            if ds.entries.is_empty() {
                ui.label(tr(lang, Key::DeltaNone));
            } else {
                let net = ds.net;
                ui.label(fill(
                    tr(lang, Key::DeltaSummary),
                    &[
                        ("{n}", &number(ds.entries.len() as u64, lang)),
                        ("{size}", &fmt_signed_bytes(net, lang)),
                    ],
                ));
            }
        }
        delta_table(ui, lang, &ds.entries);
    } else {
        ui.horizontal(|ui| {
            ui.label(tr(lang, Key::HistDepth));
            ui.add(egui::DragValue::new(&mut ds.depth).range(1..=10));
            if widgets::primary_button(ui, tr(lang, Key::HistChooseFolder), !busy).clicked() {
                state.actions.push(Action::ChooseHistory);
            }
        });
        match &ds.history {
            None => {
                ui.label(tr(lang, Key::HistNeedTwo));
            }
            Some(h) => history_view(ui, lang, h),
        }
    }
}

fn delta_table(ui: &mut Ui, lang: jt_core::Lang, entries: &[DeltaEntry]) {
    let t = Theme::of(ui);
    let mut fx = TableFx::begin(ui, "jt_delta_table");
    TableBuilder::new(ui)
        .id_salt("jt_delta_table")
        .striped(true)
        .resizable(true)
        .sense(Sense::hover())
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::initial(100.0))
        .column(Column::initial(90.0))
        .column(Column::initial(90.0))
        .column(Column::initial(100.0))
        .column(Column::remainder().at_least(200.0).clip(true))
        .header(22.0, |mut h| {
            for k in [
                Key::ColChange,
                Key::ColOldSize,
                Key::ColNewSize,
                Key::ColDifference,
                Key::ColPath,
            ] {
                h.col(|ui| {
                    widgets::header_cell(ui, tr(lang, k), None);
                });
            }
        })
        .body(|body| {
            body.rows(20.0, entries.len(), |mut row| {
                let i = row.index();
                let e = &entries[i];
                row.col(|ui| {
                    fx.mark(ui, i as u64, false);
                    ui.add_space(6.0);
                    cell(ui, tr(lang, kind_key(e.kind)), t.text2);
                });
                row.col(|ui| {
                    cell(ui, bytes(e.old_size, lang), t.text2);
                });
                row.col(|ui| {
                    cell(ui, bytes(e.new_size, lang), t.text);
                });
                row.col(|ui| {
                    let d = e.delta();
                    let c = if d >= 0 { t.err } else { t.ok };
                    cell(ui, fmt_signed_bytes(d, lang), c);
                });
                row.col(|ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(16.0, 22.0), Sense::hover());
                    let r = Rect::from_center_size(r.center(), vec2(14.0, 13.0));
                    if e.is_dir {
                        icons::paint_node_icon(
                            ui.painter(),
                            r,
                            NodeIcon::Folder,
                            t.accent,
                            with_alpha(t.accent, 60),
                            None,
                            None,
                        );
                    } else {
                        icons::paint_node_icon(
                            ui.painter(),
                            r,
                            NodeIcon::File,
                            t.text2,
                            t.text2,
                            None,
                            None,
                        );
                    }
                    cell(ui, &*e.path, t.text);
                });
            });
        });
    fx.end(ui);
}

fn history_view(ui: &mut Ui, lang: jt_core::Lang, h: &HistoryResult) {
    ui.label(fill(
        tr(lang, Key::HistSnapshots),
        &[("{n}", &h.snapshots.to_string())],
    ));
    let t = Theme::of(ui);
    cell(ui, &*h.path, t.text_hi);
    // Einfaches Liniendiagramm der Größe über die Zeit.
    let (resp, painter) = ui.allocate_painter(vec2(ui.available_width(), 160.0), Sense::hover());
    let r = resp.rect.shrink2(vec2(40.0, 18.0));
    painter.rect_filled(resp.rect, 0.0, t.panel);
    painter.rect_stroke(resp.rect, 0.0, Stroke::new(1.0, t.line), StrokeKind::Inside);
    icons::paint_corner_brackets(&painter, resp.rect, 12.0, Stroke::new(2.0, t.accent));
    let max = h.series.iter().map(|p| p.size).max().unwrap_or(0).max(1);
    let n = h.series.len();
    let pts: Vec<egui::Pos2> = h
        .series
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let x = if n <= 1 {
                r.center().x
            } else {
                r.left() + r.width() * i as f32 / (n - 1) as f32
            };
            pos2(x, r.bottom() - r.height() * (p.size as f32 / max as f32))
        })
        .collect();
    let col = t.accent;
    for w in pts.windows(2) {
        if t.glow {
            painter.line_segment([w[0], w[1]], Stroke::new(5.0, with_alpha(col, 40)));
        }
        painter.line_segment([w[0], w[1]], Stroke::new(2.0, col));
    }
    for (p, hp) in pts.iter().zip(&h.series) {
        painter.rect_filled(Rect::from_center_size(*p, vec2(6.0, 6.0)), 0.0, col);
        painter.text(
            *p + vec2(0.0, -6.0),
            Align2::CENTER_BOTTOM,
            bytes(hp.size, lang),
            widgets::mono(11.0),
            t.text,
        );
    }
    ui.add_space(6.0);
    egui::Grid::new("jt_hist_points")
        .striped(true)
        .show(ui, |ui| {
            for k in [Key::ColTime, Key::ColSize, Key::ColAllocated, Key::ColFiles] {
                cell(ui, widgets::upper(tr(lang, k)), t.dim);
            }
            ui.end_row();
            for p in &h.series {
                ui.label(fmt_time(p.time, lang));
                ui.label(bytes(p.size, lang));
                ui.label(bytes(p.allocated, lang));
                ui.label(number(p.files, lang));
                ui.end_row();
            }
        });
    ui.add_space(6.0);
    widgets::section(ui, tr(lang, Key::HistTopGrowers), t.warn);
    egui::ScrollArea::vertical()
        .id_salt("jt_hist_growers")
        .show(ui, |ui| {
            egui::Grid::new("jt_hist_grow")
                .striped(true)
                .show(ui, |ui| {
                    cell(ui, widgets::upper(tr(lang, Key::ColGrowth)), t.dim);
                    cell(ui, widgets::upper(tr(lang, Key::ColPath)), t.dim);
                    ui.end_row();
                    for (p, g) in &h.growers {
                        ui.label(fmt_signed_bytes(*g, lang));
                        ui.label(p);
                        ui.end_row();
                    }
                });
        });
}
