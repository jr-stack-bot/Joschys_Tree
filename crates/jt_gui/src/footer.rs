// Joschys_Tree – Statuszeile und Fußzeile mit Branding – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

use eframe::egui;
use jt_core::format::{bytes, number};
use jt_core::{Key, ScanMode, tr};

use crate::app::{AppState, can_delete, fill};

/// Fußzeile: "Joschys_Tree @ Joschy · Scan-Modus · Admin/Nur-Lesen" plus Status.
pub fn show(ui: &mut egui::Ui, state: &AppState) {
    let lang = state.lang();
    let (rights, rights_tip) = if can_delete() {
        (tr(lang, Key::FooterAdmin), None)
    } else {
        (
            tr(lang, Key::FooterReadOnly),
            Some(tr(lang, Key::ReadOnlyHint)),
        )
    };
    let (mode, mode_tip) = match state.tree.as_ref().map(|t| t.meta.mode) {
        Some(ScanMode::Mft) => (tr(lang, Key::ModeMft), tr(lang, Key::ModeMftLong)),
        Some(ScanMode::Walk) => (tr(lang, Key::ModeWalk), tr(lang, Key::ModeWalkLong)),
        None => ("–", tr(lang, Key::FooterMode)),
    };
    ui.horizontal(|ui| {
        ui.strong(tr(lang, Key::FooterBrand));
        ui.separator();
        ui.label(format!("{}: {mode}", tr(lang, Key::FooterMode)))
            .on_hover_text(mode_tip);
        ui.separator();
        let r = ui.label(rights);
        if let Some(tip) = rights_tip {
            r.on_hover_text(tip);
        }
        if let Some(t) = &state.tree {
            if t.meta.errors > 0 {
                ui.separator();
                ui.label(fill(
                    tr(lang, Key::StatusErrors),
                    &[("{n}", &number(u64::from(t.meta.errors), lang))],
                ));
            }
            if t.meta.volume_total > 0 {
                ui.separator();
                ui.label(fill(
                    tr(lang, Key::StatusVolume),
                    &[
                        ("{free}", &bytes(t.meta.volume_free, lang)),
                        ("{total}", &bytes(t.meta.volume_total, lang)),
                    ],
                ));
            }
        }
        if let Some(job) = &state.scan {
            ui.separator();
            ui.spinner();
            let p = &job.progress;
            ui.label(fill(
                tr(lang, Key::StatusScanProgress),
                &[
                    ("{entries}", &number(p.entries, lang)),
                    ("{size}", &bytes(p.bytes, lang)),
                    ("{current}", &p.current),
                ],
            ));
        } else if let Some(k) = state.io_busy {
            ui.separator();
            ui.spinner();
            ui.label(tr(lang, k));
        } else if !state.status.is_empty() {
            ui.separator();
            ui.label(&state.status);
        }
    });
}
