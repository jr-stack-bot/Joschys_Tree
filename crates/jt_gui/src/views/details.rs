// Joschys_Tree – Detailpanel: Owner, ACL, ADS, Zeiten – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

use eframe::egui::{self, Align2, Rect, Sense, Stroke, Ui, pos2, vec2};
use jt_core::format::{bytes, number};
use jt_core::{Key, Lang, NodeFlags, NodeId, tr};

use crate::app::{Action, AppState, DetailInfo};
use crate::icons::{self, NodeIcon};
use crate::theme::{self, FileCat, Theme, with_alpha};
use crate::views::{cell, fmt_percent, fmt_time, frac_of_parent};
use crate::widgets;

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
    let t = Theme::of(ui);
    let fx = theme::effects(ui);
    // Kopf: dezenter Verlauf, Scanlines, Eck-Klammern.
    let (head, _) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::hover());
    {
        let p = ui.painter();
        let mut m = egui::Mesh::default();
        let i = 0;
        m.colored_vertex(head.left_top(), t.core_hi);
        m.colored_vertex(head.right_top(), t.core_hi);
        m.colored_vertex(head.right_bottom(), t.core_lo);
        m.colored_vertex(head.left_bottom(), t.core_lo);
        m.add_triangle(i, i + 1, i + 2);
        m.add_triangle(i, i + 2, i + 3);
        p.add(egui::Shape::mesh(m));
        if fx {
            widgets::scanlines(p, head, t);
        }
        icons::paint_corner_brackets(p, head, 10.0, Stroke::new(2.0, t.accent));
        let wedge = Rect::from_min_size(
            pos2(head.left() + 8.0, head.center().y - 5.0),
            vec2(10.0, 10.0),
        );
        p.add(egui::Shape::convex_polygon(
            icons::corner_wedge(wedge).to_vec(),
            t.accent,
            Stroke::NONE,
        ));
        p.text(
            pos2(wedge.right() + 8.0, head.center().y),
            Align2::LEFT_CENTER,
            widgets::upper(tr(lang, Key::DetTitle)),
            widgets::mono(12.0),
            t.accent_text,
        );
        p.hline(
            (head.right() - 26.0)..=(head.right() - 8.0),
            head.center().y,
            Stroke::new(3.0, t.red),
        );
    }
    ui.add_space(6.0);
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
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
                let r = Rect::from_center_size(r.center(), vec2(16.0, 15.0));
                if n.is_dir() {
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
                    let c = FileCat::from_ext(tree.ext_of(id)).color();
                    icons::paint_node_icon(ui.painter(), r, NodeIcon::File, c, c, None, None);
                }
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(&*n.name)
                            .font(widgets::title_font(16.0))
                            .color(t.text_hi),
                    )
                    .wrap(),
                );
            });
            cell(ui, tree.full_path(id), t.text2);
            // Große Zahl mit Farbsaum.
            let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 34.0), Sense::hover());
            widgets::fringe_text(
                ui.painter(),
                pos2(r.left(), r.center().y),
                Align2::LEFT_CENTER,
                &bytes(n.size, lang),
                widgets::title_font(26.0),
                t,
            );
            widgets::section(ui, tr(lang, Key::DetTitle), t.accent);
            egui::Grid::new("jt_details_grid")
                .num_columns(2)
                .striped(true)
                .show(ui, |ui| {
                    let mut row = |k: Key, v: String| {
                        cell(ui, tr(lang, k), t.text2);
                        cell(ui, v, t.text);
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
            ui.add_space(2.0);
            ui.add(
                egui::Label::new(
                    egui::RichText::new(tr(lang, Key::DetAccessNote))
                        .small()
                        .color(t.dim),
                )
                .wrap(),
            );
            ui.add_space(6.0);
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
                widgets::busy(ui);
            } else if widgets::tool_button(
                ui,
                &format!(
                    "{}: {} / {}",
                    tr(lang, Key::DetLoad),
                    tr(lang, Key::DetPermissions),
                    tr(lang, Key::DetStreams)
                ),
                state.details.loading.is_none(),
            )
            .clicked()
            {
                state.actions.push(Action::LoadDetails(id));
            }
        }
        Some(info) => {
            let t = Theme::of(ui);
            widgets::section(ui, tr(lang, Key::DetOwner), t.accent);
            match &info.security {
                Ok(sec) => {
                    ui.label(&sec.owner);
                    ui.add_space(4.0);
                    widgets::section(ui, tr(lang, Key::DetPermissions), t.warn);
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
            widgets::section(ui, tr(lang, Key::DetStreams), t.ok);
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
