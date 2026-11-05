// Joschys_Tree – Dialoge: Lösch-Bestätigung (2-stufig), Export, Über – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

use eframe::egui::{self, Context, Id, Modal};
use jt_core::export::ExportFormat;
use jt_core::format::bytes;
use jt_core::protect::Reason;
use jt_core::{Key, Lang, NodeId, Tree, tr};

use crate::app::{Action, AppState, fill};
use crate::theme::Theme;
use crate::widgets;

#[derive(Default)]
pub struct Dialogs {
    pub delete: Option<DeleteDialog>,
    pub export: Option<ExportDialog>,
    pub about: bool,
}

/// Zweistufige Lösch-Bestätigung.
///
/// Merkt sich beim Öffnen die vollen Pfade und die Baum-Generation: Knoten-Nummern
/// gelten nur für genau einen Baum und könnten nach einem neuen Scan auf einen
/// anderen Pfad zeigen.
#[derive(Clone, Debug)]
pub struct DeleteDialog {
    pub ids: Vec<NodeId>,
    pub paths: Vec<String>,
    /// `AppState::tree_gen` beim Öffnen.
    pub tree_gen: u64,
    /// Anzahl Einträge (inkl. Unterordner/Dateien) und Größe beim Öffnen.
    pub count: u64,
    pub size: u64,
    pub permanent: bool,
    /// 1 = erste Rückfrage, 2 = Tippwort (nur endgültig).
    pub step: u8,
    pub typed: String,
    /// Vorsicht-Elemente (Pfad, Grund): rote Warnung, Häkchen nötig.
    pub caution: Vec<(String, Reason)>,
    /// Häkchen "Ich weiß, was ich tue".
    pub ack: bool,
    /// Stapel aus dem Auswahlkorb (nur Papierkorb).
    pub batch: bool,
}

impl DeleteDialog {
    pub fn new(
        tree: &Tree,
        id: NodeId,
        permanent: bool,
        tree_gen: u64,
        caution: Option<Reason>,
    ) -> Self {
        let n = tree.node(id);
        let path = tree.full_path(id);
        DeleteDialog {
            ids: vec![id],
            caution: caution.map(|r| (path.clone(), r)).into_iter().collect(),
            paths: vec![path],
            tree_gen,
            count: entry_count(tree, id),
            size: n.size,
            permanent,
            step: 1,
            typed: String::new(),
            ack: false,
            batch: false,
        }
    }

    /// Stapel aus dem Auswahlkorb: immer nur Papierkorb. Größe = belegter Speicher.
    pub fn batch(
        tree: &Tree,
        ids: Vec<NodeId>,
        tree_gen: u64,
        caution: Vec<(String, Reason)>,
    ) -> Self {
        DeleteDialog {
            paths: ids.iter().map(|&id| tree.full_path(id)).collect(),
            count: ids.iter().map(|&id| entry_count(tree, id)).sum(),
            size: ids.iter().map(|&id| tree.node(id).allocated).sum(),
            ids,
            tree_gen,
            permanent: false,
            step: 1,
            typed: String::new(),
            caution,
            ack: false,
            batch: true,
        }
    }

    /// Darf der Bestätigen-/Weiter-Knopf aktiv sein? Bei Vorsicht-Elementen nur mit Häkchen.
    pub fn ack_ok(&self) -> bool {
        self.caution.is_empty() || self.ack
    }
}

/// Anzahl Einträge inklusive aller Unterordner und Dateien.
fn entry_count(tree: &Tree, id: NodeId) -> u64 {
    let n = tree.node(id);
    if n.is_dir() {
        1 + u64::from(n.file_count) + u64::from(n.dir_count)
    } else {
        1
    }
}

/// Höchstzahl angezeigter Pfade im Stapel-Dialog.
pub const MAX_LISTED_PATHS: usize = 10;

/// Die ersten Pfade und die Zahl der übrigen.
pub fn listed_paths(paths: &[String]) -> (&[String], usize) {
    let n = paths.len().min(MAX_LISTED_PATHS);
    (&paths[..n], paths.len() - n)
}

/// Darf eine Lösch-Bestätigung ausgeführt werden? Nur wenn der Baum seit dem Öffnen
/// des Dialogs nicht ersetzt oder verändert wurde.
pub fn delete_gen_ok(dialog_gen: u64, current_gen: u64) -> bool {
    dialog_gen == current_gen
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
    let allowed = state.deletion_allowed();
    let current_gen = state.tree_gen;
    let Some(dlg) = state.dialogs.delete.as_mut() else {
        return;
    };
    // Doppelte Absicherung: ohne Admin-Rechte oder bei geladenem Snapshot nie anzeigen.
    if !allowed || state.tree.is_none() {
        state.dialogs.delete = None;
        return;
    }
    // Baum hat sich seit dem Öffnen geändert: schließen, nichts löschen.
    if !delete_gen_ok(dlg.tree_gen, current_gen) {
        state.dialogs.delete = None;
        state.status = tr(lang, Key::DelTreeChanged).to_string();
        return;
    }
    let (shown, more) = listed_paths(&dlg.paths);
    let mut path = shown.join("\n");
    if more > 0 {
        path.push('\n');
        path.push_str(&fill(
            tr(lang, Key::DelMorePaths),
            &[("{n}", &more.to_string())],
        ));
    }
    let count = dlg.count;
    let size = dlg.size;
    let mut close = false;
    let mut confirm = false;
    let resp = Modal::new(Id::unique("jt_delete_modal")).show(ctx, |ui| {
        ui.set_max_width(560.0);
        let count_s = jt_core::format::number(count, lang);
        if dlg.batch {
            ui.heading(tr(lang, Key::DelBatchTitle));
            ui.label(fill(
                tr(lang, Key::DelBatchText),
                &[
                    ("{n}", &jt_core::format::number(dlg.ids.len() as u64, lang)),
                    ("{size}", &bytes(size, lang)),
                ],
            ));
            ui.label(fill(
                tr(lang, Key::DelStep1Count),
                &[("{n}", &count_s), ("{size}", &bytes(size, lang))],
            ));
            path_box(ui, &path);
            caution_block(ui, lang, dlg);
            ui.horizontal(|ui| {
                if widgets::danger_button(ui, tr(lang, Key::BasketRecycleAll), dlg.ack_ok())
                    .clicked()
                {
                    confirm = true;
                }
                if widgets::tool_button(ui, tr(lang, Key::Cancel), true).clicked() {
                    close = true;
                }
            });
        } else if !dlg.permanent {
            ui.heading(tr(lang, Key::DelRecycleTitle));
            ui.label(fill(tr(lang, Key::DelRecycleText), &[("{n}", &count_s)]));
            path_box(ui, &path);
            caution_block(ui, lang, dlg);
            ui.horizontal(|ui| {
                if widgets::danger_button(ui, tr(lang, Key::CtxRecycle), dlg.ack_ok()).clicked() {
                    confirm = true;
                }
                if widgets::tool_button(ui, tr(lang, Key::Cancel), true).clicked() {
                    close = true;
                }
            });
        } else if dlg.step == 1 {
            ui.heading(tr(lang, Key::DelTitle));
            ui.label(tr(lang, Key::DelStep1Text));
            path_box(ui, &path);
            ui.label(fill(
                tr(lang, Key::DelStep1Count),
                &[("{n}", &count_s), ("{size}", &bytes(size, lang))],
            ));
            caution_block(ui, lang, dlg);
            ui.horizontal(|ui| {
                if widgets::danger_button(ui, tr(lang, Key::DelContinue), dlg.ack_ok()).clicked() {
                    dlg.step = 2;
                }
                if widgets::tool_button(ui, tr(lang, Key::Cancel), true).clicked() {
                    close = true;
                }
            });
        } else {
            ui.heading(tr(lang, Key::DelStep2Title));
            path_box(ui, &path);
            ui.label(tr(lang, Key::DelStep2Text));
            let edit = ui.text_edit_singleline(&mut dlg.typed);
            edit.request_focus();
            let ok = confirm_word_ok(lang, &dlg.typed) && dlg.ack_ok();
            if !dlg.typed.is_empty() && !confirm_word_ok(lang, &dlg.typed) {
                ui.colored_label(ui.visuals().warn_fg_color, tr(lang, Key::DelWrongWord));
            }
            ui.horizontal(|ui| {
                let t = Theme::of(ui);
                let btn = egui::Button::new(
                    egui::RichText::new(tr(lang, Key::DelConfirmButton))
                        .color(egui::Color32::WHITE),
                )
                .fill(t.red);
                if ui.add_enabled(ok, btn).clicked() {
                    confirm = true;
                }
                if widgets::tool_button(ui, tr(lang, Key::Cancel), true).clicked() {
                    close = true;
                }
            });
        }
    });
    if resp.should_close() {
        close = true;
    }
    if confirm && dlg.ack_ok() {
        state.actions.push(Action::ConfirmDelete {
            ids: dlg.ids.clone(),
            paths: dlg.paths.clone(),
            tree_gen: dlg.tree_gen,
            permanent: dlg.permanent,
        });
        close = true;
    }
    if close {
        state.dialogs.delete = None;
    }
}

/// Pfadliste in fester Schrift, bei vielen Pfaden scrollbar.
fn path_box(ui: &mut egui::Ui, path: &str) {
    egui::ScrollArea::vertical()
        .id_salt("jt_delete_paths")
        .max_height(180.0)
        .show(ui, |ui| {
            ui.monospace(path);
        });
}

/// Rote Warnung mit Begründungen und Häkchen "Ich weiß, was ich tue".
fn caution_block(ui: &mut egui::Ui, lang: Lang, dlg: &mut DeleteDialog) {
    if dlg.caution.is_empty() {
        return;
    }
    let t = Theme::of(ui);
    egui::Frame::new()
        .stroke(egui::Stroke::new(1.5, t.red))
        .fill(crate::theme::with_alpha(
            t.red,
            if t.dark { 28 } else { 18 },
        ))
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
                crate::icons::paint_warning(ui.painter(), r.shrink(1.0), t.red);
                ui.colored_label(
                    t.red,
                    egui::RichText::new(tr(lang, Key::DelCautionTitle)).strong(),
                );
            });
            const SHOWN: usize = 5;
            for (p, r) in dlg.caution.iter().take(SHOWN) {
                ui.colored_label(t.red, format!("{p} – {}", r.text(lang)));
            }
            if dlg.caution.len() > SHOWN {
                ui.colored_label(
                    t.red,
                    fill(
                        tr(lang, Key::DelCautionMore),
                        &[("{n}", &(dlg.caution.len() - SHOWN).to_string())],
                    ),
                );
            }
            ui.checkbox(&mut dlg.ack, tr(lang, Key::DelCautionAck));
        });
    ui.add_space(4.0);
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
                        widgets::choice(ui, &mut dlg.format, f, format_label(lang, f));
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
                            widgets::choice(ui, &mut dlg.content, c, tr(lang, content_key(c)));
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
            if widgets::primary_button(ui, tr(lang, Key::ExpButton), true).clicked() {
                run = true;
            }
            if widgets::tool_button(ui, tr(lang, Key::Cancel), true).clicked() {
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
        let t = Theme::of(ui);
        let (r, _) = ui.allocate_exact_size(egui::vec2(360.0, 30.0), egui::Sense::hover());
        // Schließen-Kreuz oben rechts (gezeichnet, keine Glyphe).
        let xr = egui::Rect::from_center_size(
            egui::pos2(r.right() - 8.0, r.center().y),
            egui::vec2(16.0, 16.0),
        );
        let xresp = ui.interact(xr, ui.scope_id().with("jt_about_x"), egui::Sense::click());
        let xc = if xresp.hovered() { t.red } else { t.dim };
        crate::icons::paint_cross(ui.painter(), xr.shrink(4.0), egui::Stroke::new(1.5, xc));
        if xresp.clicked() {
            close = true;
        }
        widgets::fringe_text(
            ui.painter(),
            r.left_center(),
            egui::Align2::LEFT_CENTER,
            tr(lang, Key::AboutTitle),
            widgets::title_font(22.0),
            t,
        );
        ui.label(tr(lang, Key::AboutText));
        ui.label(format!(
            "{} {}",
            tr(lang, Key::AboutVersion),
            env!("CARGO_PKG_VERSION")
        ));
        ui.label(tr(lang, Key::AboutLicense));
        ui.colored_label(t.accent_text, jt_core::BRANDING);
        if widgets::tool_button(ui, tr(lang, Key::Close), true).clicked() {
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
    fn caution_needs_ack_and_batch_lists_paths() {
        let t = crate::safety::tests::win_tree();
        let cfg = t
            .find_path("C:\\Users\\demo\\AppData\\Roaming\\cfg.json")
            .unwrap();
        let iso = t.find_path("C:\\Users\\demo\\Downloads\\old.iso").unwrap();
        let mut d = DeleteDialog::new(&t, cfg, false, 3, Some(Reason::AppData));
        assert!(!d.ack_ok());
        d.ack = true;
        assert!(d.ack_ok());
        let d = DeleteDialog::new(&t, iso, true, 3, None);
        assert!(d.ack_ok());
        assert_eq!(d.step, 1);
        let b = DeleteDialog::batch(
            &t,
            vec![iso, cfg],
            3,
            vec![(t.full_path(cfg), Reason::AppData)],
        );
        assert!(b.batch && !b.permanent && !b.ack_ok());
        assert_eq!(b.paths.len(), 2);
        assert_eq!(b.size, t.node(iso).allocated + t.node(cfg).allocated);
        let many: Vec<String> = (0..25).map(|i| i.to_string()).collect();
        let (shown, more) = listed_paths(&many);
        assert_eq!((shown.len(), more), (10, 15));
        let (shown, more) = listed_paths(&many[..3]);
        assert_eq!((shown.len(), more), (3, 0));
    }

    #[test]
    fn delete_only_with_unchanged_tree_gen() {
        assert!(delete_gen_ok(7, 7));
        assert!(!delete_gen_ok(7, 8));
        assert!(!delete_gen_ok(8, 7));
    }
}
