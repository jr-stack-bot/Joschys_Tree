// Joschys_Tree – Detailpanel: Owner, ACL, ADS, Zeiten – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

use eframe::egui::{self, Ui};
use jt_core::format::{bytes, number};
use jt_core::{Key, Lang, NodeFlags, NodeId, tr};

use crate::app::{Action, AppState, DetailInfo};
use crate::views::{fmt_percent, fmt_time, frac_of_parent};

#[derive(Default)]
pub struct DetailsState {
    /// Geladene Infos für genau einen Knoten.
    pub info: Option<(NodeId, DetailInfo)>,
    pub loading: Option<NodeId>,
}

const ATTRS: [(NodeFlags, Key); 9] = [
    (NodeFlags::HIDDEN, Key::AttrHidden),
    (NodeFlags::SYSTEM, Key::AttrSystem),
    (NodeFlags::READONLY, Key::AttrReadonly),
    (NodeFlags::REPARSE, Key::AttrReparse),
    (NodeFlags::COMPRESSED, Key::AttrCompressed),
    (NodeFlags::SPARSE, Key::AttrSparse),
    (NodeFlags::HAS_ADS, Key::AttrHasAds),
    (NodeFlags::ACCESS_DENIED, Key::AttrAccessDenied),
    (NodeFlags::OFFLINE, Key::AttrOffline),
];

pub fn show(ui: &mut Ui, state: &mut AppState) {
    let lang = state.lang();
    ui.heading(tr(lang, Key::DetTitle));
    ui.separator();
    let (Some(tree), Some(id)) = (state.tree.clone(), state.selected) else {
        return;
    };
    if id as usize >= tree.nodes.len() {
        return;
    }
    let n = tree.node(id);
    egui::ScrollArea::vertical()
        .id_salt("jt_details_scroll")
        .show(ui, |ui| {
            ui.strong(&*n.name);
            ui.small(tree.full_path(id));
            ui.add_space(4.0);
            egui::Grid::new("jt_details_grid")
                .num_columns(2)
                .striped(true)
                .show(ui, |ui| {
                    let mut row = |k: Key, v: String| {
                        ui.label(tr(lang, k));
                        ui.label(v);
                        ui.end_row();
                    };
                    row(
                        Key::ColType,
                        tr(
                            lang,
                            if n.is_dir() {
                                Key::TypeFolder
                            } else {
                                Key::TypeFile
                            },
                        )
                        .into(),
                    );
                    row(Key::ColSize, bytes(n.size, lang));
                    row(Key::ColAllocated, bytes(n.allocated, lang));
                    row(
                        Key::ColPercentOfParent,
                        fmt_percent(frac_of_parent(&tree, id), lang),
                    );
                    if n.is_dir() {
                        row(Key::ColFiles, number(u64::from(n.file_count), lang));
                        row(Key::ColFolders, number(u64::from(n.dir_count), lang));
                    }
                    row(Key::ColCreated, fmt_time(n.created, lang));
                    row(Key::ColModified, fmt_time(n.modified, lang));
                    row(Key::ColAccessed, fmt_time(n.accessed, lang));
                    if n.ads_size > 0 {
                        row(Key::ColAds, bytes(n.ads_size, lang));
                    }
                    let attrs: Vec<&str> = ATTRS
                        .iter()
                        .filter(|(f, _)| n.flags.contains(*f))
                        .map(|(_, k)| tr(lang, *k))
                        .collect();
                    if !attrs.is_empty() {
                        row(Key::ColAttributes, attrs.join(", "));
                    }
                });
            ui.small(tr(lang, Key::DetAccessNote));
            ui.separator();
            lazy_part(ui, lang, state, id);
        });
}

/// Owner/ACL/ADS werden erst auf Knopfdruck im Hintergrund geladen.
fn lazy_part(ui: &mut Ui, lang: Lang, state: &mut AppState, id: NodeId) {
    let loaded = state
        .details
        .info
        .as_ref()
        .filter(|(i, _)| *i == id)
        .map(|(_, info)| info.clone());
    match loaded {
        None => {
            if state.details.loading == Some(id) {
                ui.spinner();
            } else if ui
                .add_enabled(
                    state.details.loading.is_none(),
                    egui::Button::new(format!(
                        "{}: {} / {}",
                        tr(lang, Key::DetLoad),
                        tr(lang, Key::DetPermissions),
                        tr(lang, Key::DetStreams)
                    )),
                )
                .clicked()
            {
                state.actions.push(Action::LoadDetails(id));
            }
        }
        Some(info) => {
            ui.strong(tr(lang, Key::DetOwner));
            match &info.security {
                Ok(sec) => {
                    ui.label(&sec.owner);
                    ui.add_space(4.0);
                    ui.strong(tr(lang, Key::DetPermissions));
                    egui::Grid::new("jt_acl_grid").striped(true).show(ui, |ui| {
                        ui.small(tr(lang, Key::DetPrincipal));
                        ui.small(tr(lang, Key::ColType));
                        ui.small(tr(lang, Key::DetRights));
                        ui.small(tr(lang, Key::DetInherited));
                        ui.end_row();
                        for a in &sec.aces {
                            ui.label(&a.principal);
                            ui.label(tr(lang, if a.allow { Key::DetAllow } else { Key::DetDeny }));
                            ui.label(&a.rights);
                            ui.label(tr(lang, if a.inherited { Key::Yes } else { Key::No }));
                            ui.end_row();
                        }
                    });
                }
                Err(e) => {
                    ui.label(format!("{}: {e}", tr(lang, Key::DetNotAvailable)));
                }
            }
            ui.add_space(6.0);
            ui.strong(tr(lang, Key::DetStreams));
            match &info.streams {
                Ok(s) if s.is_empty() => {
                    ui.label(tr(lang, Key::DetNoStreams));
                }
                Ok(s) => {
                    egui::Grid::new("jt_ads_grid").striped(true).show(ui, |ui| {
                        for st in s {
                            ui.label(&st.name);
                            ui.label(bytes(st.size, lang));
                            ui.end_row();
                        }
                    });
                }
                Err(e) => {
                    ui.label(format!("{}: {e}", tr(lang, Key::DetNotAvailable)));
                }
            }
        }
    }
}
