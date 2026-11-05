// Joschys_Tree – Tab "Aufräumen": Kandidaten-Karten, Ersparnis, Übergabe an den Korb – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later
//
// Die Suche (jt_core::cleanup::find_cleanup) läuft in einem Hintergrund-Thread
// (app.rs, Action::StartCleanup). Hier wird nur angezeigt; gelöscht wird nie direkt,
// sondern immer über den Auswahlkorb mit Bestätigungsdialog.

use std::collections::HashSet;

use eframe::egui::{self, Sense, Ui, vec2};
use jt_core::cleanup::{CleanupGroup, CleanupKind};
use jt_core::format::{bytes, number};
use jt_core::protect::{ProtectEnv, classify};
use jt_core::{FileTime, Key, NodeId, Tree, tr};

use crate::app::{Action, AppState, fill};
use crate::icons;
use crate::theme::Theme;
use crate::views::{age_days, cell, fill_n, no_tree};
use crate::widgets;

/// Anzahl Elemente in den Details einer Karte.
pub const DETAIL_ROWS: usize = 20;

/// Eine Karte des Aufräum-Tabs (im Hintergrund-Thread vorbereitet).
#[derive(Clone, Debug)]
pub struct CleanCard {
    pub kind: CleanupKind,
    pub nodes: Vec<NodeId>,
    pub allocated: u64,
    pub count: u64,
    pub info_only: bool,
    /// Mindestens ein Element hat die Schutzstufe Vorsicht.
    pub caution: bool,
    /// Die größten Elemente: (Knoten, belegter Speicher, Alter in Tagen).
    pub top: Vec<(NodeId, u64, Option<i64>)>,
}

impl CleanCard {
    /// Soll "Alle empfohlenen" diese Gruppe mitnehmen? Alte Downloads und große
    /// alte Dateien wählt der Nutzer bewusst selbst.
    pub fn recommended(&self) -> bool {
        !self.info_only
            && !matches!(
                self.kind,
                CleanupKind::LargeOldFiles | CleanupKind::OldDownloads
            )
    }
}

/// Gruppen in Karten umwandeln (reine Funktion, läuft im Hintergrund).
pub fn build_cards(
    tree: &Tree,
    groups: &[CleanupGroup],
    now: FileTime,
    env: &ProtectEnv,
) -> Vec<CleanCard> {
    let mut cards: Vec<CleanCard> = groups
        .iter()
        .map(|g| {
            let mut top: Vec<(NodeId, u64, Option<i64>)> = g
                .nodes
                .iter()
                .map(|&id| (id, tree.node(id).allocated, age_days(tree.node(id), now)))
                .collect();
            top.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
            top.truncate(DETAIL_ROWS);
            let caution = !g.info_only
                && g.nodes
                    .iter()
                    .any(|&id| classify(tree, id, env).is_caution());
            CleanCard {
                kind: g.kind,
                nodes: g.nodes.clone(),
                allocated: g.allocated,
                count: g.count,
                info_only: g.info_only,
                caution,
                top,
            }
        })
        .collect();
    // Löschbare Gruppen zuerst (nach Größe), Hinweise ans Ende.
    cards.sort_by(|a, b| {
        a.info_only
            .cmp(&b.info_only)
            .then(b.allocated.cmp(&a.allocated))
    });
    cards
}

/// Mögliche Ersparnis aller löschbaren Gruppen.
pub fn savings(cards: &[CleanCard]) -> u64 {
    cards
        .iter()
        .filter(|c| !c.info_only)
        .map(|c| c.allocated)
        .sum()
}

/// Knoten einer Karte ohne abgewählte.
pub fn card_selection(card: &CleanCard, deselected: &HashSet<NodeId>) -> Vec<NodeId> {
    card.nodes
        .iter()
        .copied()
        .filter(|id| !deselected.contains(id))
        .collect()
}

/// Alle empfohlenen Knoten (ohne Info-Gruppen, alte Downloads, große alte Dateien).
pub fn recommended_nodes(cards: &[CleanCard], deselected: &HashSet<NodeId>) -> Vec<NodeId> {
    cards
        .iter()
        .filter(|c| c.recommended())
        .flat_map(|c| card_selection(c, deselected))
        .collect()
}

#[derive(Default)]
pub struct CleanupState {
    /// Laufende Suche für diese Baum-Generation.
    pub running: Option<u64>,
    /// Generation, für die `cards` gelten.
    pub done_gen: Option<u64>,
    pub cards: Vec<CleanCard>,
    pub open: HashSet<CleanupKind>,
    deselected: HashSet<NodeId>,
}

impl CleanupState {
    pub fn set_cards(&mut self, gen_: u64, cards: Vec<CleanCard>) {
        self.cards = cards;
        self.done_gen = Some(gen_);
        self.deselected.clear();
    }
}

pub fn show(ui: &mut Ui, state: &mut AppState) {
    let lang = state.lang();
    let Some(tree) = state.tree.clone() else {
        no_tree(ui, lang);
        return;
    };
    let gen_ = state.tree_gen;
    if state.cleanup.running.is_none() && state.cleanup.done_gen != Some(gen_) {
        state.actions.push(Action::StartCleanup);
    }
    let t = Theme::of(ui);
    let cs = &mut state.cleanup;
    let actions = &mut state.actions;
    let ready = cs.done_gen == Some(gen_) && cs.running.is_none();

    // Kopf: Besen, Ersparnis, Sammelknopf.
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::hover());
        icons::paint_broom(ui.painter(), r.shrink(2.0), t.accent);
        let total = if ready { savings(&cs.cards) } else { 0 };
        let (r, _) = ui.allocate_exact_size(vec2(360.0, 30.0), Sense::hover());
        widgets::fringe_text(
            ui.painter(),
            r.left_center(),
            egui::Align2::LEFT_CENTER,
            &fill(
                tr(lang, Key::CleanSavingsTotal),
                &[("{size}", &bytes(total, lang))],
            ),
            widgets::title_font(22.0),
            t,
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if widgets::tool_button(ui, tr(lang, Key::CleanRecalc), ready).clicked() {
                actions.push(Action::StartCleanup);
            }
            let rec = if ready {
                recommended_nodes(&cs.cards, &cs.deselected)
            } else {
                Vec::new()
            };
            if widgets::primary_button(ui, tr(lang, Key::CleanAddRecommended), !rec.is_empty())
                .on_hover_text(tr(lang, Key::CleanRecommendedNote))
                .clicked()
            {
                actions.push(Action::AddBasket(rec));
            }
        });
    });
    cell(ui, tr(lang, Key::CleanRecommendedNote), t.text2);
    ui.add_space(4.0);
    if !ready {
        ui.add_space(20.0);
        ui.horizontal(|ui| {
            widgets::busy(ui);
            cell(ui, tr(lang, Key::CleanRunning), t.warn);
        });
        return;
    }
    if cs.cards.is_empty() {
        ui.add_space(20.0);
        cell(ui, tr(lang, Key::CleanNone), t.text2);
        return;
    }
    egui::ScrollArea::vertical()
        .id_salt("jt_cleanup_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            // Breite Fenster: zwei Spalten, sonst eine.
            let cols = if ui.available_width() >= 1000.0 { 2 } else { 1 };
            let open = &mut cs.open;
            let deselected = &mut cs.deselected;
            ui.columns(cols, |columns| {
                for (i, card) in cs.cards.iter().enumerate() {
                    let ui = &mut columns[i % cols];
                    card_ui(ui, &tree, card, open, deselected, actions, lang);
                    ui.add_space(6.0);
                }
            });
        });
}

fn card_ui(
    ui: &mut Ui,
    tree: &Tree,
    card: &CleanCard,
    open: &mut HashSet<CleanupKind>,
    deselected: &mut HashSet<NodeId>,
    actions: &mut Vec<Action>,
    lang: jt_core::Lang,
) {
    let t = Theme::of(ui);
    let edge = if card.info_only {
        t.line
    } else {
        crate::theme::with_alpha(t.accent, 110)
    };
    egui::Frame::new()
        .fill(t.panel)
        .stroke(egui::Stroke::new(1.0, edge))
        .inner_margin(egui::Margin::symmetric(12, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            // Linke Akzentleiste
            let r = ui.max_rect();
            let bar =
                egui::Rect::from_min_size(r.min - vec2(12.0, 8.0), vec2(3.0, r.height() + 16.0));
            ui.painter().rect_filled(
                bar,
                0.0,
                if card.info_only {
                    t.dim
                } else if card.caution {
                    t.warn
                } else {
                    t.accent
                },
            );
            ui.horizontal(|ui| {
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(tr(lang, card.kind.title_key()))
                            .font(widgets::title_font(16.0))
                            .color(t.text_hi),
                    )
                    .selectable(false),
                );
                if card.info_only {
                    cell(ui, format!("[ {} ]", tr(lang, Key::CleanInfoBadge)), t.dim);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(bytes(card.allocated, lang))
                                .font(widgets::title_font(18.0))
                                .color(t.accent_text),
                        )
                        .selectable(false),
                    );
                    cell(
                        ui,
                        fill_n(tr(lang, Key::CleanCount), card.count as i64),
                        t.text2,
                    );
                });
            });
            ui.add(
                egui::Label::new(
                    egui::RichText::new(tr(lang, card.kind.hint_key())).color(if card.info_only {
                        t.text
                    } else {
                        t.text2
                    }),
                )
                .wrap(),
            );
            if card.caution {
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
                    icons::paint_warning(ui.painter(), r.shrink(1.0), t.warn);
                    cell(ui, tr(lang, Key::CleanCautionNote), t.warn);
                });
            }
            if card.info_only {
                return;
            }
            let is_open = open.contains(&card.kind);
            ui.horizontal(|ui| {
                let sel = card_selection(card, deselected);
                if widgets::tool_button(ui, tr(lang, Key::CtxAddBasket), !sel.is_empty()).clicked()
                {
                    actions.push(Action::AddBasket(sel));
                }
                let key = if is_open {
                    Key::CleanDetailsHide
                } else {
                    Key::CleanDetailsShow
                };
                if widgets::tool_button(ui, tr(lang, key), true).clicked()
                    && !open.remove(&card.kind)
                {
                    open.insert(card.kind);
                }
                if !deselected.is_empty() {
                    let size: u64 = card
                        .nodes
                        .iter()
                        .filter(|id| !deselected.contains(id))
                        .map(|&id| tree.node(id).allocated)
                        .sum();
                    cell(
                        ui,
                        fill(
                            tr(lang, Key::CleanSelectedSize),
                            &[("{size}", &bytes(size, lang))],
                        ),
                        t.dim,
                    );
                }
            });
            if is_open {
                ui.add_space(2.0);
                cell(
                    ui,
                    fill(
                        tr(lang, Key::CleanLargest),
                        &[
                            ("{n}", &card.top.len().to_string()),
                            ("{total}", &number(card.nodes.len() as u64, lang)),
                        ],
                    ),
                    t.dim,
                );
                for (row, &(id, alloc, age)) in card.top.iter().enumerate() {
                    let mut on = !deselected.contains(&id);
                    let bg = ui.painter().add(egui::Shape::Noop);
                    let r = ui.horizontal(|ui| {
                        if ui.checkbox(&mut on, "").changed() {
                            if on {
                                deselected.remove(&id);
                            } else {
                                deselected.insert(id);
                            }
                        }
                        let fg = if on { t.text } else { t.dim };
                        let w = 78.0;
                        ui.add_sized(
                            [w, 18.0],
                            egui::Label::new(egui::RichText::new(bytes(alloc, lang)).color(fg)),
                        );
                        let age_s = match age {
                            Some(d) => fill_n(tr(lang, Key::AgeDays), d),
                            None => tr(lang, Key::Unknown).to_string(),
                        };
                        ui.add_sized(
                            [w, 18.0],
                            egui::Label::new(egui::RichText::new(age_s).color(t.text2)),
                        );
                        ui.add(
                            egui::Label::new(egui::RichText::new(tree.full_path(id)).color(fg))
                                .truncate(),
                        );
                    });
                    if row % 2 == 1 {
                        ui.painter().set(
                            bg,
                            egui::Shape::rect_filled(
                                r.response.rect,
                                0.0,
                                crate::theme::with_alpha(t.row, 160),
                            ),
                        );
                    }
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use jt_core::cleanup::{CleanupOptions, find_cleanup};
    use jt_core::model::{EntryInfo, ScanMeta, ScanMode, TreeBuilder};

    fn tree(now: FileTime) -> Tree {
        let old = FileTime(now.0 - 400 * 86_400 * 10_000_000);
        let f = |size: u64, m: FileTime| EntryInfo {
            size,
            allocated: size,
            modified: m,
            created: m,
            accessed: m,
            ..Default::default()
        };
        let d = |m: FileTime| EntryInfo {
            modified: m,
            ..Default::default()
        };
        let mut b = TreeBuilder::new(ScanMeta::new("C:\\", ScanMode::Walk), "C:\\");
        let users = b.add_dir(0, "Users", d(old));
        let demo = b.add_dir(users, "demo", d(old));
        let dl = b.add_dir(demo, "Downloads", d(old));
        b.add_file(dl, "win.iso", f(5 << 30, old));
        b.add_file(dl, "brief.pdf", f(1 << 20, old));
        let app = b.add_dir(demo, "AppData", d(old));
        let local = b.add_dir(app, "Local", d(old));
        let temp = b.add_dir(local, "Temp", d(old));
        b.add_file(temp, "a.tmp", f(300 << 20, old));
        b.add_dir(0, "Leer", d(old));
        b.add_file(0, "hiberfil.sys", f(8 << 30, old));
        b.finish()
    }

    #[test]
    fn cards_savings_and_recommended() {
        let now = FileTime::from_unix_secs(1_793_318_400);
        let t = tree(now);
        let env = ProtectEnv::windows_default();
        let groups = find_cleanup(&t, &env, now, &CleanupOptions::default());
        let cards = build_cards(&t, &groups, now, &env);
        assert_eq!(cards.len(), groups.len());
        let kinds: Vec<CleanupKind> = cards.iter().map(|c| c.kind).collect();
        assert!(kinds.contains(&CleanupKind::InfoHibernation), "{kinds:?}");
        assert!(kinds.contains(&CleanupKind::UserTemp), "{kinds:?}");
        // Info-Gruppen zählen nicht zur Ersparnis.
        let s = savings(&cards);
        let info: u64 = cards
            .iter()
            .filter(|c| c.info_only)
            .map(|c| c.allocated)
            .sum();
        let all: u64 = cards.iter().map(|c| c.allocated).sum();
        assert_eq!(s, all - info);
        assert!(s > 0);
        // Empfohlen: ohne Info, alte Downloads, große alte Dateien.
        let none = HashSet::new();
        let rec = recommended_nodes(&cards, &none);
        let hib = t.find_path("C:\\hiberfil.sys").unwrap();
        assert!(!rec.contains(&hib));
        for c in cards.iter().filter(|c| !c.recommended()) {
            for n in &c.nodes {
                assert!(!rec.contains(n));
            }
        }
        let tmp = cards
            .iter()
            .find(|c| c.kind == CleanupKind::UserTemp)
            .unwrap();
        assert!(tmp.nodes.iter().all(|n| rec.contains(n)));
        // Abwählen wirkt.
        let mut des = HashSet::new();
        des.insert(tmp.nodes[0]);
        assert!(!recommended_nodes(&cards, &des).contains(&tmp.nodes[0]));
        // Details: höchstens 20, absteigend.
        for c in &cards {
            assert!(c.top.len() <= DETAIL_ROWS);
            assert!(c.top.windows(2).all(|w| w[0].1 >= w[1].1));
            // Kein Kandidat ist gesperrt.
            if !c.info_only {
                assert!(c.nodes.iter().all(|&n| !classify(&t, n, &env).is_blocked()));
            }
        }
    }
}
