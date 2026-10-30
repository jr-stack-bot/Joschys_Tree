// Joschys_Tree – Dialoge: Lösch-Bestätigung (2-stufig), Export, Über – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

use eframe::egui::{self, Context, Id, Modal};
use jt_core::export::ExportFormat;
use jt_core::format::bytes;
use jt_core::{Key, Lang, NodeId, tr};

use crate::app::{Action, AppState, can_delete, fill};

#[derive(Default)]
pub struct Dialogs {
    pub delete: Option<DeleteDialog>,
    pub export: Option<ExportDialog>,
    pub about: bool,
}

/// Zweistufige Lösch-Bestätigung.
#[derive(Clone, Debug)]
pub struct DeleteDialog {
    pub id: NodeId,
    pub permanent: bool,
    /// 1 = erste Rückfrage, 2 = Tippwort (nur endgültig).
    pub step: u8,
    pub typed: String,
}

impl DeleteDialog {
    pub fn new(id: NodeId, permanent: bool) -> Self {
        DeleteDialog {
            id,
            permanent,
            step: 1,
            typed: String::new(),
        }
    }
}

/// Prüft das Tippwort (exakt, nach Trimmen).
pub fn confirm_word_ok(lang: Lang, typed: &str) -> bool {
    typed.trim() == tr(lang, Key::DelConfirmWord)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContentKind {
    Tree,
    TopFiles,
    FileTypes,
    Duplicates,
    Delta,
}

#[derive(Clone, Debug)]
pub struct ExportDialog {
    pub format: ExportFormat,
    pub content: ContentKind,
    pub depth: u32,
    pub root: NodeId,
    pub title: String,
}

impl ExportDialog {
    pub fn new(depth: u32, selected: Option<NodeId>) -> Self {
        ExportDialog {
            format: ExportFormat::Xlsx,
            content: ContentKind::Tree,
            depth: depth.max(1),
            root: selected.unwrap_or(0),
            title: String::new(),
        }
    }
}

pub fn format_label(lang: Lang, f: ExportFormat) -> &'static str {
    tr(
        lang,
        match f {
            ExportFormat::Csv => Key::ExpFormatCsv,
            ExportFormat::Xlsx => Key::ExpFormatXlsx,
            ExportFormat::Html => Key::ExpFormatHtml,
            ExportFormat::Pdf => Key::ExpFormatPdf,
        },
    )
}

fn content_key(c: ContentKind) -> Key {
    match c {
        ContentKind::Tree => Key::ExpContentTree,
        ContentKind::TopFiles => Key::ExpContentTopFiles,
        ContentKind::FileTypes => Key::ExpContentFileTypes,
        ContentKind::Duplicates => Key::ExpContentDuplicates,
        ContentKind::Delta => Key::ExpContentDelta,
    }
}

fn content_title_key(c: ContentKind) -> Key {
    match c {
        ContentKind::Tree => Key::ExpTitleTree,
        ContentKind::TopFiles => Key::ExpTitleTopFiles,
        ContentKind::FileTypes => Key::ExpTitleFileTypes,
        ContentKind::Duplicates => Key::ExpTitleDuplicates,
        ContentKind::Delta => Key::ExpTitleDelta,
    }
}

pub fn show(ctx: &Context, state: &mut AppState) {
    delete_dialog(ctx, state);
    export_dialog(ctx, state);
    about_dialog(ctx, state);
}

fn delete_dialog(ctx: &Context, state: &mut AppState) {
    let lang = state.lang();
    let Some(dlg) = state.dialogs.delete.as_mut() else {
        return;
    };
    // Doppelte Absicherung: ohne Admin-Rechte nie anzeigen.
    let tree = match (&state.tree, can_delete()) {
        (Some(t), true) if (dlg.id as usize) < t.nodes.len() => t.clone(),
        _ => {
            state.dialogs.delete = None;
            return;
        }
    };
    let n = tree.node(dlg.id);
    let path = tree.full_path(dlg.id);
    let count = if n.is_dir() {
        1 + u64::from(n.file_count) + u64::from(n.dir_count)
    } else {
        1
    };
    let mut close = false;
    let mut confirm = false;
    let resp = Modal::new(Id::unique("jt_delete_modal")).show(ctx, |ui| {
        ui.set_max_width(520.0);
        let count_s = jt_core::format::number(count, lang);
        if !dlg.permanent {
            ui.heading(tr(lang, Key::DelRecycleTitle));
            ui.label(fill(tr(lang, Key::DelRecycleText), &[("{n}", &count_s)]));
            ui.monospace(&path);
            ui.horizontal(|ui| {
                if ui.button(tr(lang, Key::CtxRecycle)).clicked() {
                    confirm = true;
                }
                if ui.button(tr(lang, Key::Cancel)).clicked() {
                    close = true;
                }
            });
        } else if dlg.step == 1 {
            ui.heading(tr(lang, Key::DelTitle));
            ui.label(tr(lang, Key::DelStep1Text));
            ui.monospace(&path);
            ui.label(fill(
                tr(lang, Key::DelStep1Count),
                &[("{n}", &count_s), ("{size}", &bytes(n.size, lang))],
            ));
            ui.horizontal(|ui| {
                if ui.button(tr(lang, Key::DelContinue)).clicked() {
                    dlg.step = 2;
                }
                if ui.button(tr(lang, Key::Cancel)).clicked() {
                    close = true;
                }
            });
        } else {
            ui.heading(tr(lang, Key::DelStep2Title));
            ui.monospace(&path);
            ui.label(tr(lang, Key::DelStep2Text));
            let edit = ui.text_edit_singleline(&mut dlg.typed);
            edit.request_focus();
            let ok = confirm_word_ok(lang, &dlg.typed);
            if !dlg.typed.is_empty() && !ok {
                ui.colored_label(ui.visuals().warn_fg_color, tr(lang, Key::DelWrongWord));
            }
            ui.horizontal(|ui| {
                let btn = egui::Button::new(tr(lang, Key::DelConfirmButton))
                    .fill(egui::Color32::from_rgb(0xb0, 0x20, 0x20));
                if ui.add_enabled(ok, btn).clicked() {
                    confirm = true;
                }
                if ui.button(tr(lang, Key::Cancel)).clicked() {
                    close = true;
                }
            });
        }
    });
    if resp.should_close() {
        close = true;
    }
    if confirm {
        state.actions.push(Action::ConfirmDelete {
            ids: vec![dlg.id],
            permanent: dlg.permanent,
        });
        close = true;
    }
    if close {
        state.dialogs.delete = None;
    }
}

fn export_dialog(ctx: &Context, state: &mut AppState) {
    let lang = state.lang();
    let has_dups = !state.dups.groups.is_empty();
    let has_delta = !state.delta.entries.is_empty();
    let root_path = state
        .tree
        .as_ref()
        .map(|t| t.full_path(state.dialogs.export.as_ref().map_or(0, |d| d.root)))
        .unwrap_or_default();
    let Some(dlg) = state.dialogs.export.as_mut() else {
        return;
    };
    let mut close = false;
    let mut run = false;
    let resp = Modal::new(Id::unique("jt_export_modal")).show(ctx, |ui| {
        ui.set_min_width(380.0);
        ui.heading(tr(lang, Key::ExpTitle));
        ui.separator();
        egui::Grid::new("jt_export_grid")
            .num_columns(2)
            .spacing([12.0, 6.0])
            .show(ui, |ui| {
                ui.label(tr(lang, Key::ExpFormat));
                ui.vertical(|ui| {
                    for f in [
                        ExportFormat::Xlsx,
                        ExportFormat::Csv,
                        ExportFormat::Html,
                        ExportFormat::Pdf,
                    ] {
                        ui.radio_value(&mut dlg.format, f, format_label(lang, f));
                    }
                });
                ui.end_row();
                ui.label(tr(lang, Key::ExpContent));
                ui.vertical(|ui| {
                    for (c, enabled) in [
                        (ContentKind::Tree, true),
                        (ContentKind::TopFiles, true),
                        (ContentKind::FileTypes, true),
                        (ContentKind::Duplicates, has_dups),
                        (ContentKind::Delta, has_delta),
                    ] {
                        ui.add_enabled_ui(enabled, |ui| {
                            ui.radio_value(&mut dlg.content, c, tr(lang, content_key(c)));
                        });
                    }
                });
                ui.end_row();
                if dlg.content == ContentKind::Tree {
                    ui.label(tr(lang, Key::ExpScanRoot));
                    ui.label(&root_path);
                    ui.end_row();
                    ui.label(tr(lang, Key::ExpMaxDepth));
                    ui.add(egui::DragValue::new(&mut dlg.depth).range(1..=64));
                    ui.end_row();
                }
                ui.label(tr(lang, Key::ExpReportTitle));
                ui.add(
                    egui::TextEdit::singleline(&mut dlg.title)
                        .hint_text(tr(lang, content_title_key(dlg.content))),
                );
                ui.end_row();
            });
        ui.separator();
        ui.horizontal(|ui| {
            if ui.button(tr(lang, Key::ExpButton)).clicked() {
                run = true;
            }
            if ui.button(tr(lang, Key::Cancel)).clicked() {
                close = true;
            }
        });
    });
    if resp.should_close() {
        close = true;
    }
    if run {
        let mut cfg = dlg.clone();
        if cfg.title.trim().is_empty() {
            cfg.title = tr(lang, content_title_key(cfg.content)).to_string();
        }
        state.actions.push(Action::RunExport(cfg));
        close = true;
    }
    if close {
        state.dialogs.export = None;
    }
}

fn about_dialog(ctx: &Context, state: &mut AppState) {
    if !state.dialogs.about {
        return;
    }
    let lang = state.lang();
    let mut close = false;
    let resp = Modal::new(Id::unique("jt_about_modal")).show(ctx, |ui| {
        ui.heading(tr(lang, Key::AboutTitle));
        ui.label(tr(lang, Key::AboutText));
        ui.label(format!(
            "{} {}",
            tr(lang, Key::AboutVersion),
            env!("CARGO_PKG_VERSION")
        ));
        ui.label(tr(lang, Key::AboutLicense));
        ui.strong(jt_core::BRANDING);
        if ui.button(tr(lang, Key::Close)).clicked() {
            close = true;
        }
    });
    if close || resp.should_close() {
        state.dialogs.about = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confirm_word_per_language() {
        assert!(confirm_word_ok(Lang::De, "LÖSCHEN"));
        assert!(confirm_word_ok(Lang::De, " LÖSCHEN "));
        assert!(!confirm_word_ok(Lang::De, "löschen"));
        assert!(!confirm_word_ok(Lang::De, "DELETE"));
        assert!(confirm_word_ok(Lang::En, "DELETE"));
        assert!(!confirm_word_ok(Lang::En, ""));
    }

    #[test]
    fn delete_dialog_starts_at_step_one() {
        let d = DeleteDialog::new(5, true);
        assert_eq!(d.step, 1);
        assert!(d.typed.is_empty());
    }
}
