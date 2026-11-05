// Joschys_Tree – Korb-Panel: gesammelte Elemente, Summe, Stapel-Papierkorb – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

use eframe::egui::{self, Rect, Sense, Ui, vec2};
use egui_extras::{Column, TableBuilder};
use jt_core::format::{bytes, number};
use jt_core::protect::Protection;
use jt_core::{Key, NodeId, tr};

use crate::app::{Action, AppState, can_delete, fill};
use crate::icons::{self, NodeIcon};
use crate::theme::{FileCat, Theme, with_alpha};
use crate::views::{cell, lock_color, prot_text, row_interaction};
use crate::widgets::{self, TableFx};

pub fn show(ui: &mut Ui, state: &mut AppState) {
    let lang = state.lang();
    let Some(tree) = state.tree.clone() else {
        return;
    };
    if state.basket.gen_() != state.tree_gen {
        state.basket.reset(state.tree_gen, state.tree_is_live);
    }
    let t = Theme::of(ui);
    let allowed = state.deletion_allowed() && state.basket.live();
    let busy = state.delete_running;
    let empty = state.basket.is_empty();
    let total = state.basket.total_allocated(&tree);
    let n_items = state.basket.len();
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::hover());
        icons::paint_basket(ui.painter(), r.shrink(1.0), t.accent);
        ui.add(
            egui::Label::new(
                egui::RichText::new(widgets::upper(tr(lang, Key::BasketTitle)))
                    .font(widgets::mono(12.0))
                    .color(t.accent_text),
            )
            .selectable(false),
        );
        cell(ui, format!("[{}]", number(n_items as u64, lang)), t.dim);
        ui.add_space(8.0);
        let freed = fill(
            tr(lang, Key::BasketFreed),
            &[("{size}", &bytes(total, lang))],
        );
        ui.add(
            egui::Label::new(
                egui::RichText::new(freed)
                    .font(widgets::title_font(17.0))
                    .color(if empty { t.text2 } else { t.text_hi }),
            )
            .selectable(false),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // Einklappen (gezeichnetes Kreuz).
            let (xr, xresp) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::click());
            let xc = if xresp.hovered() { t.red } else { t.dim };
            icons::paint_cross(ui.painter(), xr.shrink(5.0), egui::Stroke::new(1.5, xc));
            if xresp.on_hover_text(tr(lang, Key::BasketShortcut)).clicked() {
                state.actions.push(Action::ToggleBasketPanel);
            }
            if widgets::tool_button(ui, tr(lang, Key::BasketClear), !empty).clicked() {
                state.actions.push(Action::ClearBasket);
            }
            if widgets::tool_button(ui, tr(lang, Key::BasketCopyPaths), !empty).clicked() {
                state.actions.push(Action::CopyBasketPaths);
            }
            let r = widgets::danger_button(
                ui,
                tr(lang, Key::BasketRecycleAll),
                allowed && !empty && !busy,
            );
            let r = if !can_delete() {
                r.on_hover_text(tr(lang, Key::BasketNeedAdmin))
            } else if !state.tree_is_live {
                r.on_hover_text(tr(lang, Key::BasketNeedLive))
            } else {
                r
            };
            if r.clicked() {
                state.actions.push(Action::AskDeleteBasket);
            }
        });
    });
    ui.add_space(2.0);
    if empty {
        ui.add_space(6.0);
        cell(ui, tr(lang, Key::BasketEmpty), t.text2);
        return;
    }
    let items: Vec<NodeId> = state.basket.items().to_vec();
    let selected = state.selected;
    let mut remove: Option<NodeId> = None;
    let mut cx = crate::menu_cx!(state, &tree);
    let mut fx = TableFx::begin(ui, "jt_basket_table");
    let max_h = (ui.available_height() - 24.0).max(40.0);
    TableBuilder::new(ui)
        .id_salt("jt_basket_table")
        .striped(true)
        .resizable(true)
        .sense(Sense::click())
        .auto_shrink([false, false])
        .min_scrolled_height(40.0)
        .max_scroll_height(max_h)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::exact(24.0))
        .column(Column::initial(230.0).at_least(100.0).clip(true))
        .column(Column::initial(90.0))
        .column(Column::initial(120.0).clip(true))
        .column(Column::remainder().at_least(120.0).clip(true))
        .header(22.0, |mut h| {
            h.col(|_| {});
            for k in [
                Key::ColName,
                Key::ColAllocated,
                Key::ColProtection,
                Key::ColPath,
            ] {
                h.col(|ui| {
                    widgets::header_cell(ui, tr(lang, k), None);
                });
            }
        })
        .body(|body| {
            body.rows(21.0, items.len(), |mut row| {
                let id = items[row.index()];
                let n = tree.node(id);
                let is_sel = selected == Some(id);
                row.set_selected(is_sel);
                let main = if is_sel { t.sel_text } else { t.text };
                row.col(|ui| {
                    fx.mark(ui, u64::from(id), is_sel);
                    let (xr, xresp) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::click());
                    let c = if xresp.hovered() { t.red } else { t.dim };
                    icons::paint_cross(ui.painter(), xr.shrink(5.0), egui::Stroke::new(1.5, c));
                    if xresp
                        .on_hover_text(tr(lang, Key::CtxRemoveBasket))
                        .clicked()
                    {
                        remove = Some(id);
                    }
                });
                row.col(|ui| {
                    let (ir, _) = ui.allocate_exact_size(vec2(16.0, 18.0), Sense::hover());
                    let ir = Rect::from_center_size(ir.center(), vec2(14.0, 13.0));
                    if n.is_dir() {
                        icons::paint_node_icon(
                            ui.painter(),
                            ir,
                            NodeIcon::Folder,
                            t.accent,
                            with_alpha(t.accent, if t.dark { 60 } else { 90 }),
                            None,
                            None,
                        );
                    } else {
                        let c = FileCat::from_ext(tree.ext_of(id)).color();
                        icons::paint_node_icon(ui.painter(), ir, NodeIcon::File, c, c, None, None);
                    }
                    cell(ui, &*n.name, main);
                });
                row.col(|ui| {
                    cell(ui, bytes(n.allocated, lang), main);
                });
                row.col(|ui| {
                    let prot = cx.prot.protection(&tree, id);
                    let (ir, resp) = ui.allocate_exact_size(vec2(13.0, 16.0), Sense::hover());
                    let ir = Rect::from_center_size(ir.center(), vec2(11.0, 11.0));
                    let color = match prot {
                        Protection::Free => t.ok,
                        Protection::Caution(_) => {
                            icons::paint_warning(ui.painter(), ir, t.warn);
                            t.warn
                        }
                        Protection::Blocked(_) => {
                            icons::paint_padlock(ui.painter(), ir, lock_color(t));
                            lock_color(t)
                        }
                    };
                    let l = cell(ui, tr(lang, prot.level_key()), color);
                    if let Some(r) = prot.reason() {
                        let tip = prot_text(prot, r, lang);
                        resp.on_hover_text(tip.clone());
                        l.on_hover_text(tip);
                    }
                });
                row.col(|ui| {
                    ui.add(
                        egui::Label::new(egui::RichText::new(tree.full_path(id)).color(t.text2))
                            .truncate()
                            .selectable(false),
                    );
                });
                row_interaction(&row.response(), &mut cx, id);
            });
        });
    fx.end(ui);
    if let Some(id) = remove {
        state.actions.push(Action::RemoveBasket(id));
    }
}
