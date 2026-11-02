// Joschys_Tree – Statuszeile und Fußzeile mit Branding – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

use eframe::egui::{self, Rect, Sense, Stroke, pos2, vec2};
use jt_core::format::{bytes, number};
use jt_core::{Key, ScanMode, tr};

use crate::app::{AppState, can_delete, fill};
use crate::icons;
use crate::theme::{Theme, with_alpha};
use crate::widgets::{self, mono};

/// Trennstrich zwischen Fußzeilen-Feldern.
fn sep(ui: &mut egui::Ui, t: &Theme) {
    let (r, _) = ui.allocate_exact_size(vec2(9.0, 14.0), Sense::hover());
    ui.painter().vline(
        r.center().x.round() + 0.5,
        r.y_range(),
        Stroke::new(1.0, t.line),
    );
}

fn tag(ui: &mut egui::Ui, text: &str, color: egui::Color32) -> egui::Response {
    ui.add(
        egui::Label::new(egui::RichText::new(text).font(mono(12.0)).color(color))
            .selectable(false)
            .truncate(),
    )
}

/// Fußzeile: Status als Klammer-Tag, Scan-Modus, Admin/Nur-Lesen, rechts "@ Joschy".
pub fn show(ui: &mut egui::Ui, state: &AppState) {
    let lang = state.lang();
    let t = Theme::of(ui);
    let full = ui.max_rect();
    // Laufender Scan: dünner wandernder Akzent-Verlauf am oberen Rand.
    if state.scan.is_some() || state.io_busy.is_some() {
        let top = Rect::from_min_size(pos2(full.left(), full.top() - 4.0), vec2(full.width(), 3.0));
        let time = ui.input(|i| i.time) as f32;
        let seg = (full.width() * 0.25).max(40.0);
        let x = (time * 260.0) % (full.width() + seg) - seg;
        let r =
            Rect::from_min_size(pos2(full.left() + x, top.top()), vec2(seg, 3.0)).intersect(top);
        let p = ui.painter().with_clip_rect(top);
        p.rect_filled(top, 0.0, with_alpha(t.accent, 30));
        widgets::paint_gradient(&p, r, with_alpha(t.accent, 0), t.accent_hi);
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(33));
    }
    let (rights, rights_tip, admin) = if can_delete() {
        (tr(lang, Key::FooterAdmin), None, true)
    } else {
        (
            tr(lang, Key::FooterReadOnly),
            Some(tr(lang, Key::ReadOnlyHint)),
            false,
        )
    };
    let (mode, mode_tip) = match state.tree.as_ref().map(|t| t.meta.mode) {
        Some(ScanMode::Mft) => (tr(lang, Key::ModeMft), tr(lang, Key::ModeMftLong)),
        Some(ScanMode::Walk) => (tr(lang, Key::ModeWalk), tr(lang, Key::ModeWalkLong)),
        None => ("–", tr(lang, Key::FooterMode)),
    };
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            tag(ui, tr(lang, Key::FooterBrand), t.dim);
            let (r, _) = ui.allocate_exact_size(vec2(18.0, 3.0), Sense::hover());
            ui.painter().rect_filled(r, 0.0, t.red);
            sep(ui, t);
            let rr = tag(ui, rights, if admin { t.ok } else { t.warn });
            if let Some(tip) = rights_tip {
                rr.on_hover_text(tip);
            }
            let (r, _) = ui.allocate_exact_size(vec2(12.0, 14.0), Sense::hover());
            icons::paint_shield(
                ui.painter(),
                Rect::from_center_size(r.center(), vec2(11.0, 12.0)),
                if admin { t.ok } else { t.warn },
                admin,
            );
            sep(ui, t);
            tag(
                ui,
                &format!("{}: {mode}", tr(lang, Key::FooterMode)),
                t.text2,
            )
            .on_hover_text(mode_tip);
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                // Status links als Klammer-Tag.
                if let Some(job) = &state.scan {
                    widgets::busy(ui);
                    let p = &job.progress;
                    tag(
                        ui,
                        &format!(
                            "[ {} ]",
                            fill(
                                tr(lang, Key::StatusScanProgress),
                                &[
                                    ("{entries}", &number(p.entries, lang)),
                                    ("{size}", &bytes(p.bytes, lang)),
                                    ("{current}", &p.current),
                                ],
                            )
                        ),
                        t.warn,
                    );
                } else if let Some(k) = state.io_busy {
                    widgets::busy(ui);
                    tag(ui, &format!("[ {} ]", tr(lang, k)), t.warn);
                } else if !state.status.is_empty() {
                    tag(ui, &format!("[ {} ]", state.status), t.ok);
                }
                if let Some(tr_) = &state.tree {
                    if tr_.meta.errors > 0 {
                        sep(ui, t);
                        tag(
                            ui,
                            &fill(
                                tr(lang, Key::StatusErrors),
                                &[("{n}", &number(u64::from(tr_.meta.errors), lang))],
                            ),
                            t.err,
                        );
                    }
                    if tr_.meta.volume_total > 0 {
                        sep(ui, t);
                        tag(
                            ui,
                            &fill(
                                tr(lang, Key::StatusVolume),
                                &[
                                    ("{free}", &bytes(tr_.meta.volume_free, lang)),
                                    ("{total}", &bytes(tr_.meta.volume_total, lang)),
                                ],
                            ),
                            t.text2,
                        );
                    }
                }
            });
        });
    });
}
