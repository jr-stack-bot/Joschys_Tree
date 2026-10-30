// Joschys_Tree – Delta- und Verlaufsansicht – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

use eframe::egui::{self, Align2, Color32, FontId, Sense, Stroke, Ui, pos2, vec2};
use egui_extras::{Column, TableBuilder};
use jt_core::delta::{ChangeKind, DeltaEntry};
use jt_core::format::{bytes, number};
use jt_core::{Key, tr};

use crate::app::{Action, AppState, HistoryResult, fill};
use crate::views::{fmt_signed_bytes, fmt_time};

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
        ui.selectable_value(&mut ds.show_history, false, tr(lang, Key::DeltaTitle));
        ui.selectable_value(&mut ds.show_history, true, tr(lang, Key::HistTitle));
    });
    ui.separator();
    if !ds.show_history {
        ui.horizontal(|ui| {
            ui.label(tr(lang, Key::DeltaMinChange));
            ui.add(
                egui::DragValue::new(&mut ds.min_change_mib)
                    .range(0..=1_000_000)
                    .suffix(tr(lang, Key::UnitMibSuffix)),
            );
            if ui
                .add_enabled(
                    has_tree && !busy,
                    egui::Button::new(tr(lang, Key::DeltaChooseOld)),
                )
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
            if ui
                .add_enabled(!busy, egui::Button::new(tr(lang, Key::HistChooseFolder)))
                .clicked()
            {
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
    TableBuilder::new(ui)
        .id_salt("jt_delta_table")
        .striped(true)
        .resizable(true)
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
                    ui.strong(tr(lang, k));
                });
            }
        })
        .body(|body| {
            body.rows(20.0, entries.len(), |mut row| {
                let e = &entries[row.index()];
                row.col(|ui| {
                    ui.label(tr(lang, kind_key(e.kind)));
                });
                row.col(|ui| {
                    ui.label(bytes(e.old_size, lang));
                });
                row.col(|ui| {
                    ui.label(bytes(e.new_size, lang));
                });
                row.col(|ui| {
                    let d = e.delta();
                    let c = if d >= 0 {
                        Color32::from_rgb(0xe1, 0x57, 0x59)
                    } else {
                        Color32::from_rgb(0x59, 0xa1, 0x4f)
                    };
                    ui.colored_label(c, fmt_signed_bytes(d, lang));
                });
                row.col(|ui| {
                    let icon = if e.is_dir { "📁" } else { "📄" };
                    ui.label(format!("{icon} {}", e.path));
                });
            });
        });
}

fn history_view(ui: &mut Ui, lang: jt_core::Lang, h: &HistoryResult) {
    ui.label(fill(
        tr(lang, Key::HistSnapshots),
        &[("{n}", &h.snapshots.to_string())],
    ));
    ui.strong(&h.path);
    // Einfaches Liniendiagramm der Größe über die Zeit.
    let (resp, painter) = ui.allocate_painter(vec2(ui.available_width(), 160.0), Sense::hover());
    let r = resp.rect.shrink(8.0);
    painter.rect_filled(resp.rect, 4.0, ui.visuals().extreme_bg_color);
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
    let col = crate::theme::bar_color(ui.visuals().dark_mode);
    for w in pts.windows(2) {
        painter.line_segment([w[0], w[1]], Stroke::new(2.0, col));
    }
    for (p, hp) in pts.iter().zip(&h.series) {
        painter.circle_filled(*p, 3.0, col);
        painter.text(
            *p + vec2(0.0, -4.0),
            Align2::CENTER_BOTTOM,
            bytes(hp.size, lang),
            FontId::proportional(10.0),
            ui.visuals().text_color(),
        );
    }
    ui.separator();
    egui::Grid::new("jt_hist_points")
        .striped(true)
        .show(ui, |ui| {
            ui.strong(tr(lang, Key::ColTime));
            ui.strong(tr(lang, Key::ColSize));
            ui.strong(tr(lang, Key::ColAllocated));
            ui.strong(tr(lang, Key::ColFiles));
            ui.end_row();
            for p in &h.series {
                ui.label(fmt_time(p.time, lang));
                ui.label(bytes(p.size, lang));
                ui.label(bytes(p.allocated, lang));
                ui.label(number(p.files, lang));
                ui.end_row();
            }
        });
    ui.separator();
    ui.strong(tr(lang, Key::HistTopGrowers));
    egui::ScrollArea::vertical()
        .id_salt("jt_hist_growers")
        .show(ui, |ui| {
            egui::Grid::new("jt_hist_grow")
                .striped(true)
                .show(ui, |ui| {
                    ui.strong(tr(lang, Key::ColGrowth));
                    ui.strong(tr(lang, Key::ColPath));
                    ui.end_row();
                    for (p, g) in &h.growers {
                        ui.label(fmt_signed_bytes(*g, lang));
                        ui.label(p);
                        ui.end_row();
                    }
                });
        });
}
