// Joschys_Tree – Treemap (Squarified-Layout und Zeichnen) – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

use eframe::egui::{self, Align2, Rect, Sense, Stroke, StrokeKind, Ui, pos2};
use jt_core::format::bytes;
use jt_core::protect::Protection;
use jt_core::{Key, NO_NODE, NodeId, Tree, tr};

use crate::app::{Action, AppState, fill};
use crate::icons;
use crate::safety::ProtCache;
use crate::settings::TreemapColor;
use crate::theme::{self, FileCat, Theme, readable_on, with_alpha};
use crate::views::{
    MenuFrom, age_days, cell, fill_n, fmt_percent, lock_color, no_tree, node_menu, prot_text,
};
use crate::widgets;

/// Kacheln unter dieser Kantenlänge werden nicht weiter unterteilt.
const MIN_SPLIT: f32 = 12.0;
/// Kopfzeile eines Ordners in Pixeln.
const HEADER: f32 = 14.0;
/// Maximale Zahl gezeichneter Kacheln (Schutz bei Millionen Dateien).
const MAX_TILES: usize = 20_000;
/// Maximale Verschachtelungstiefe.
const MAX_DEPTH: u32 = 12;

/// Squarified-Treemap (Bruls, Huizing, van Wijk). Reine Funktion.
///
/// `items` sollten nach Größe absteigend sortiert sein (sonst wird intern sortiert).
/// Einträge mit Größe 0 erhalten keine Fläche. Die Summe der Flächen entspricht der Fläche von `rect`.
pub fn squarify(items: &[(NodeId, u64)], rect: Rect) -> Vec<(NodeId, Rect)> {
    let mut items: Vec<(NodeId, u64)> = items.iter().copied().filter(|(_, s)| *s > 0).collect();
    items.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let total: f64 = items.iter().map(|(_, s)| *s as f64).sum();
    let mut out = Vec::with_capacity(items.len());
    if items.is_empty() || rect.width() <= 0.0 || rect.height() <= 0.0 || total <= 0.0 {
        return out;
    }
    let scale = (rect.width() as f64 * rect.height() as f64) / total;
    let areas: Vec<f64> = items.iter().map(|(_, s)| *s as f64 * scale).collect();

    // Arbeitsrechteck in f64 für Genauigkeit.
    let (mut x, mut y) = (rect.min.x as f64, rect.min.y as f64);
    let (mut w, mut h) = (rect.width() as f64, rect.height() as f64);
    let mut start = 0;
    while start < items.len() {
        let side = w.min(h);
        // Zeile greedy erweitern, solange das schlechteste Seitenverhältnis besser wird.
        let mut end = start + 1;
        let mut sum = areas[start];
        let mut best = worst(&areas[start..end], sum, side);
        while end < items.len() {
            let s2 = sum + areas[end];
            let wv = worst(&areas[start..=end], s2, side);
            if wv > best {
                break;
            }
            best = wv;
            sum = s2;
            end += 1;
        }
        let last_row = end == items.len();
        if w >= h {
            // Spalte links
            let col_w = if last_row { w } else { sum / h };
            let mut cy = y;
            for (i, a) in areas[start..end].iter().enumerate() {
                let is_last = i == end - start - 1;
                let ih = if is_last { y + h - cy } else { a / col_w };
                out.push((items[start + i].0, mk_rect(x, cy, col_w, ih)));
                cy += ih;
            }
            x += col_w;
            w -= col_w;
        } else {
            // Zeile oben
            let row_h = if last_row { h } else { sum / w };
            let mut cx = x;
            for (i, a) in areas[start..end].iter().enumerate() {
                let is_last = i == end - start - 1;
                let iw = if is_last { x + w - cx } else { a / row_h };
                out.push((items[start + i].0, mk_rect(cx, y, iw, row_h)));
                cx += iw;
            }
            y += row_h;
            h -= row_h;
        }
        start = end;
    }
    out
}

fn mk_rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
    Rect::from_min_max(
        pos2(x as f32, y as f32),
        pos2((x + w.max(0.0)) as f32, (y + h.max(0.0)) as f32),
    )
}

/// Schlechtestes Seitenverhältnis einer Zeile (je kleiner, desto quadratischer).
fn worst(row: &[f64], sum: f64, side: f64) -> f64 {
    if sum <= 0.0 || side <= 0.0 {
        return f64::INFINITY;
    }
    let (mut mx, mut mn) = (f64::MIN, f64::MAX);
    for &a in row {
        mx = mx.max(a);
        mn = mn.min(a);
    }
    let s2 = sum * sum;
    let w2 = side * side;
    (w2 * mx / s2).max(s2 / (w2 * mn))
}

/// Eine gezeichnete Kachel.
#[derive(Clone, Copy, Debug)]
struct Tile {
    id: NodeId,
    rect: Rect,
    depth: u32,
    prot: Protection,
    /// Oberste gesperrte Kachel eines vererbend gesperrten Bereichs (oder gesperrte
    /// Datei): bekommt Abdunklung und Schraffur.
    hatch: bool,
}

#[derive(Default)]
pub struct TreemapState {
    /// Aktueller Zoom-Knoten (0 = Wurzel).
    pub root: NodeId,
    tiles: Vec<Tile>,
    built_for: Option<(u64, NodeId, Rect)>,
    /// Kachel, für die das Kontextmenü offen ist.
    menu_target: Option<NodeId>,
}

impl TreemapState {
    pub fn invalidate(&mut self) {
        self.built_for = None;
    }

    /// Layout neu berechnen; Schutzstufen nur für die gezeichneten Kacheln (Cache).
    fn rebuild(&mut self, tree: &Tree, rect: Rect, gen_: u64, prot: &mut ProtCache) {
        self.tiles.clear();
        let base = prot.protection(tree, self.root);
        layout_rec(tree, self.root, rect, 0, base, &mut self.tiles, prot);
        self.built_for = Some((gen_, self.root, rect));
    }
}

fn layout_rec(
    tree: &Tree,
    dir: NodeId,
    rect: Rect,
    depth: u32,
    parent_prot: Protection,
    out: &mut Vec<Tile>,
    prot: &mut ProtCache,
) {
    let items: Vec<(NodeId, u64)> = tree
        .children(dir)
        .map(|c| (c, tree.node(c).size))
        .filter(|(_, s)| *s > 0)
        .collect();
    let parent_inherits = matches!(parent_prot, Protection::Blocked(r) if r.is_inherited());
    for (id, r) in squarify(&items, rect) {
        if out.len() >= MAX_TILES {
            return;
        }
        let p = if parent_inherits {
            parent_prot
        } else {
            prot.protection(tree, id)
        };
        let n = tree.node(id);
        let hatch = !parent_inherits
            && matches!(p, Protection::Blocked(reason) if reason.is_inherited() || !n.is_dir());
        out.push(Tile {
            id,
            rect: r,
            depth,
            prot: p,
            hatch,
        });
        if n.is_dir()
            && depth < MAX_DEPTH
            && r.width() > MIN_SPLIT
            && r.height() > MIN_SPLIT + HEADER
        {
            let inner = Rect::from_min_max(
                pos2(r.min.x + 1.0, r.min.y + HEADER),
                pos2(r.max.x - 1.0, r.max.y - 1.0),
            );
            layout_rec(tree, id, inner, depth + 1, p, out, prot);
        }
    }
}

/// Höhe der Legende unter der Treemap.
const LEGEND_H: f32 = 20.0;

/// Kachelfarbe je Farbmodus.
fn tile_fill(
    th: &Theme,
    mode: TreemapColor,
    tree: &Tree,
    t: &Tile,
    now: jt_core::FileTime,
) -> (egui::Color32, egui::Color32) {
    match mode {
        TreemapColor::FileType => {
            let cat = FileCat::from_ext(tree.ext_of(t.id));
            (cat.tile(th), cat.color())
        }
        TreemapColor::Age => {
            let f = theme::age_frac(age_days(tree.node(t.id), now));
            let c = f.map_or(FileCat::Other.color(), |f| theme::age_color(th, f));
            (theme::age_tile(th, f), c)
        }
        TreemapColor::Protection => (theme::prot_tile(th, t.prot), theme::prot_color(th, t.prot)),
    }
}

pub fn show(ui: &mut Ui, state: &mut AppState) {
    let lang = state.lang();
    let Some(tree) = state.tree.clone() else {
        no_tree(ui, lang);
        return;
    };
    if state.treemap.root as usize >= tree.nodes.len() {
        state.treemap.root = 0;
    }
    let th = Theme::of(ui);
    let mut mode = state.settings.treemap_color;
    // Kopfzeile: Brotkrumen (anklickbar) links, Farbmodus rechts.
    let mut jump: Option<NodeId> = None;
    ui.horizontal(|ui| {
        let tm = &state.treemap;
        let mut chain = vec![tm.root];
        let mut p = tree.node(tm.root).parent;
        while p != NO_NODE {
            chain.push(p);
            p = tree.node(p).parent;
        }
        chain.reverse();
        let parent = tree.node(tm.root).parent;
        if widgets::tool_button(ui, tr(lang, Key::TreemapZoomOut), parent != NO_NODE).clicked() {
            jump = Some(parent);
        }
        ui.add_space(4.0);
        ui.spacing_mut().item_spacing.x = 2.0;
        // Sehr tiefe Pfade: Anfang, Auslassung, die letzten Ebenen.
        const KEEP: usize = 6;
        let skip = chain.len().saturating_sub(KEEP + 1);
        for (i, &id) in chain.iter().enumerate() {
            if i > 0 && i <= skip {
                if i == 1 {
                    cell(ui, '\u{2026}'.to_string(), th.dim);
                }
                continue;
            }
            if i > 0 {
                let (r, _) = ui.allocate_exact_size(egui::vec2(10.0, 14.0), Sense::hover());
                icons::paint_chevron(ui.painter(), r.center(), 8.0, Stroke::new(1.2, th.dim));
            }
            let last = i + 1 == chain.len();
            let name = tree.node(id).name.to_string();
            let color = if last { th.accent_text } else { th.text };
            let r = ui.add(
                egui::Label::new(
                    egui::RichText::new(name)
                        .color(color)
                        .font(widgets::mono(12.0)),
                )
                .sense(if last { Sense::hover() } else { Sense::click() })
                .selectable(false),
            );
            if !last && r.hovered() {
                ui.painter().hline(
                    r.rect.x_range(),
                    r.rect.bottom(),
                    Stroke::new(1.0, th.accent),
                );
            }
            if r.clicked() {
                jump = Some(id);
            }
        }
        ui.add_space(8.0);
        cell(ui, bytes(tree.node(tm.root).size, lang), th.accent_text);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            for (m, k) in [
                (TreemapColor::Protection, Key::TmColorProt),
                (TreemapColor::Age, Key::TmColorAge),
                (TreemapColor::FileType, Key::TmColorType),
            ] {
                widgets::choice(ui, &mut mode, m, tr(lang, k));
            }
            cell(ui, tr(lang, Key::TmColorBy), th.dim);
        });
    });
    if let Some(j) = jump {
        state.treemap.root = j;
    }
    if mode != state.settings.treemap_color {
        state.settings.treemap_color = mode;
        state.save_settings();
    }
    ui.add_space(2.0);
    let avail = ui.available_size() - egui::vec2(0.0, LEGEND_H + 4.0);
    let (resp, painter) = ui.allocate_painter(avail.max(egui::vec2(10.0, 10.0)), Sense::click());
    let outer = resp.rect;
    painter.rect_filled(outer, 0.0, th.panel);
    let rect = outer.shrink(3.0);
    let gen_ = state.tree_gen;
    let now = state.now;
    let selected = state.selected;
    let tm = &mut state.treemap;
    if tm.built_for != Some((gen_, tm.root, rect)) {
        tm.rebuild(&tree, rect, gen_, &mut state.prot);
    }
    if tm.tiles.is_empty() {
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            tr(lang, Key::TreemapEmpty),
            widgets::title_font(15.0),
            th.text2,
        );
        legend(ui, mode, lang);
        return;
    }
    let font = widgets::mono(11.0);
    for t in &tm.tiles {
        let n = tree.node(t.id);
        if n.is_dir() {
            // Ordner: nur Kontur und Kopfzeile, damit die Dateien dominieren.
            painter.rect_stroke(t.rect, 0.0, Stroke::new(1.0, th.line), StrokeKind::Inside);
            if t.rect.width() > 40.0 && t.rect.height() > HEADER {
                let mut x = 4.0;
                let icon =
                    Rect::from_min_size(t.rect.min + egui::vec2(3.0, 2.0), egui::vec2(10.0, 10.0));
                match t.prot {
                    Protection::Blocked(_) => {
                        icons::paint_padlock(&painter, icon, lock_color(th));
                        x = 15.0;
                    }
                    Protection::Caution(_) => {
                        icons::paint_warning(&painter, icon, th.warn);
                        x = 15.0;
                    }
                    Protection::Free => {}
                }
                painter.with_clip_rect(t.rect.shrink(1.0)).text(
                    t.rect.min + egui::vec2(x, 1.0),
                    Align2::LEFT_TOP,
                    &n.name,
                    font.clone(),
                    th.text2,
                );
            }
        } else {
            let (fill, edge_c) = tile_fill(th, mode, &tree, t, now);
            painter.rect_filled(t.rect, 0.0, fill);
            if t.rect.width() >= 3.0 && t.rect.height() >= 3.0 {
                let edge = if th.dark {
                    with_alpha(edge_c, 170)
                } else {
                    with_alpha(th.panel, 200)
                };
                painter.rect_stroke(t.rect, 0.0, Stroke::new(1.0, edge), StrokeKind::Inside);
            }
            if t.rect.width() > 60.0 && t.rect.height() > 18.0 {
                painter.with_clip_rect(t.rect.shrink(2.0)).text(
                    t.rect.min + egui::vec2(4.0, 3.0),
                    Align2::LEFT_TOP,
                    &n.name,
                    font.clone(),
                    readable_on(fill),
                );
            }
            if t.prot.is_caution() && t.rect.width() > 26.0 && t.rect.height() > 26.0 {
                let ir = Rect::from_min_size(
                    pos2(t.rect.right() - 15.0, t.rect.bottom() - 14.0),
                    egui::vec2(12.0, 11.0),
                );
                icons::paint_warning(&painter, ir, th.warn);
            }
        }
    }
    // Gesperrte Bereiche: abdunkeln, schraffieren, Schloss.
    let (shade, hatch) = theme::hatch_colors(th);
    for t in tm.tiles.iter().filter(|t| t.hatch) {
        let r = if tree.node(t.id).is_dir() {
            Rect::from_min_max(pos2(t.rect.min.x, t.rect.min.y + HEADER), t.rect.max)
        } else {
            t.rect
        };
        painter.rect_filled(r, 0.0, shade);
        icons::paint_hatch(&painter.with_clip_rect(r), r, 7.0, Stroke::new(1.0, hatch));
        if r.width() > 30.0 && r.height() > 30.0 {
            let s = (r.width().min(r.height()) * 0.3).clamp(12.0, 28.0);
            let lr = Rect::from_center_size(r.center(), egui::vec2(s, s));
            painter.rect_filled(lr.expand(4.0), 0.0, with_alpha(th.panel, 210));
            icons::paint_padlock(&painter, lr, lock_color(th));
        }
    }
    // Korb-Elemente: gestrichelter Akzentrahmen (vier Ecken) und Korb-Symbol.
    for t in tm.tiles.iter().filter(|t| state.basket.contains(t.id)) {
        painter.rect_stroke(
            t.rect.shrink(1.0),
            0.0,
            Stroke::new(1.5, th.accent),
            StrokeKind::Inside,
        );
        if t.rect.width() > 20.0 && t.rect.height() > 20.0 {
            let br = Rect::from_min_size(
                pos2(t.rect.right() - 16.0, t.rect.top() + 2.0),
                egui::vec2(13.0, 13.0),
            );
            painter.rect_filled(br.expand(1.0), 0.0, with_alpha(th.panel, 220));
            icons::paint_basket(&painter, br, th.accent);
        }
    }
    let hovered_tile = resp
        .hover_pos()
        .and_then(|p| deepest_at(&tm.tiles, p))
        .copied();
    if let Some(t) = hovered_tile {
        if th.glow && theme::effects(ui) {
            widgets::glow(&painter, t.rect, th.accent, 1.0);
        }
        painter.rect_stroke(t.rect, 0.0, Stroke::new(2.0, th.accent), StrokeKind::Inside);
    }
    // Hervorhebung der Auswahl
    if let Some(sel) = selected
        && let Some(t) = tm.tiles.iter().find(|t| t.id == sel)
    {
        painter.rect_stroke(
            t.rect,
            0.0,
            Stroke::new(2.0, th.text_hi),
            StrokeKind::Inside,
        );
        icons::paint_corner_brackets(
            &painter,
            t.rect.expand(1.0),
            8.0,
            Stroke::new(2.0, th.accent),
        );
    }
    icons::paint_corner_brackets(&painter, outer, 14.0, Stroke::new(2.0, th.accent));
    if let Some(t) = hovered_tile
        && !egui::Popup::is_id_open(ui.ctx(), egui::Popup::default_response_id(&resp))
    {
        let tip = tooltip(&tree, tm.root, t.id, t.prot, now, lang);
        resp.clone().on_hover_text_at_pointer(tip);
    }
    let hovered = hovered_tile.map(|t| t.id);
    if resp.double_clicked() {
        // Hineinzoomen: in den obersten Ordner unter dem Zoom-Knoten, der den Treffer enthält.
        if let Some(id) = hovered
            && let Some(z) = zoom_target(&tree, tm.root, id)
        {
            tm.root = z;
        }
    } else if resp.clicked()
        && let Some(id) = hovered
    {
        if ui.input(|i| i.modifiers.command) {
            state.actions.push(Action::ToggleBasket(id));
        } else {
            state.actions.push(Action::Select(id));
        }
    }
    if resp.secondary_clicked() {
        tm.menu_target = hovered;
        if let Some(id) = hovered {
            state.actions.push(Action::Select(id));
        }
    }
    let target = tm.menu_target;
    let mut cx = crate::menu_cx!(state, &tree);
    resp.context_menu(|ui| {
        if let Some(id) = target {
            node_menu(ui, &mut cx, id, MenuFrom::Treemap);
        }
    });
    legend(ui, mode, lang);
}

/// Mehrzeiliger Tooltip: Pfad, Größe mit Anteil an der Ansicht, Alter, Schutz.
fn tooltip(
    tree: &Tree,
    view: NodeId,
    id: NodeId,
    prot: Protection,
    now: jt_core::FileTime,
    lang: jt_core::Lang,
) -> String {
    let n = tree.node(id);
    let total = tree.node(view).size.max(1);
    let share = fmt_percent((n.size as f64 / total as f64) as f32, lang);
    let age = match age_days(n, now) {
        Some(d) => fill_n(tr(lang, Key::AgeDays), d),
        None => tr(lang, Key::Unknown).to_string(),
    };
    let level = match prot.reason() {
        Some(r) => prot_text(prot, r, lang),
        None => tr(lang, prot.level_key()).to_string(),
    };
    let lines = [
        tree.full_path(id),
        fill(
            tr(lang, Key::TmTipSize),
            &[("{size}", &bytes(n.size, lang)), ("{share}", &share)],
        ),
        fill(tr(lang, Key::TmTipAge), &[("{age}", &age)]),
        fill(tr(lang, Key::TmTipProt), &[("{level}", &level)]),
    ];
    let mut out = String::new();
    for (i, l) in lines.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(l);
    }
    out
}

/// Legende passend zum Farbmodus, plus Bedienhinweis rechts.
fn legend(ui: &mut Ui, mode: TreemapColor, lang: jt_core::Lang) {
    let th = Theme::of(ui);
    ui.add_space(2.0);
    let small = |ui: &mut Ui, text: &str, c: egui::Color32| {
        ui.add(
            egui::Label::new(egui::RichText::new(text).color(c).font(widgets::mono(11.0)))
                .selectable(false),
        );
    };
    let row = ui.horizontal(|ui| {
        ui.set_height(LEGEND_H);
        ui.spacing_mut().item_spacing.x = 3.0;
        // Schutz-Markierungen gelten in allen Modi.
        let (r, _) = ui.allocate_exact_size(egui::vec2(16.0, 11.0), Sense::hover());
        let (shade, hatch) = theme::hatch_colors(th);
        ui.painter().rect_filled(r, 0.0, th.row);
        ui.painter().rect_filled(r, 0.0, shade);
        icons::paint_hatch(
            &ui.painter().with_clip_rect(r),
            r,
            4.0,
            Stroke::new(1.0, hatch),
        );
        let (ir, _) = ui.allocate_exact_size(egui::vec2(10.0, 11.0), Sense::hover());
        icons::paint_padlock(ui.painter(), ir, lock_color(th));
        small(ui, tr(lang, Key::ProtLevelBlocked), th.text2);
        ui.add_space(4.0);
        let (ir, _) = ui.allocate_exact_size(egui::vec2(11.0, 11.0), Sense::hover());
        icons::paint_warning(ui.painter(), ir, th.warn);
        small(ui, tr(lang, Key::ProtLevelCaution), th.text2);
        let (r, _) = ui.allocate_exact_size(egui::vec2(13.0, 12.0), Sense::hover());
        ui.painter().vline(
            r.center().x.round() + 0.5,
            r.y_range(),
            Stroke::new(1.0, th.line),
        );
        let swatch = |ui: &mut Ui, c: egui::Color32, text: &str| {
            let (r, _) = ui.allocate_exact_size(egui::vec2(9.0, 9.0), Sense::hover());
            ui.painter().rect_filled(r, 0.0, c);
            small(ui, text, th.text2);
            ui.add_space(3.0);
        };
        match mode {
            TreemapColor::FileType => {
                for c in FileCat::ALL {
                    swatch(ui, c.tile(th), tr(lang, c.label_key()));
                }
            }
            TreemapColor::Age => {
                small(ui, tr(lang, Key::TmLegendNew), th.text2);
                let (r, _) = ui.allocate_exact_size(egui::vec2(140.0, 9.0), Sense::hover());
                let half = Rect::from_min_size(r.min, egui::vec2(r.width() * 0.5, r.height()));
                widgets::paint_gradient(
                    ui.painter(),
                    half,
                    theme::age_tile(th, Some(0.0)),
                    theme::age_tile(th, Some(0.5)),
                );
                widgets::paint_gradient(
                    ui.painter(),
                    half.translate(egui::vec2(r.width() * 0.5, 0.0)),
                    theme::age_tile(th, Some(0.5)),
                    theme::age_tile(th, Some(1.0)),
                );
                small(ui, tr(lang, Key::TmLegendOld), th.text2);
                ui.add_space(6.0);
                swatch(ui, theme::age_tile(th, None), tr(lang, Key::Unknown));
            }
            TreemapColor::Protection => {
                use jt_core::protect::Reason;
                swatch(
                    ui,
                    theme::prot_tile(th, Protection::Free),
                    tr(lang, Key::ProtLevelFree),
                );
                swatch(
                    ui,
                    theme::prot_tile(th, Protection::Caution(Reason::AppData)),
                    tr(lang, Key::ProtLevelCaution),
                );
                swatch(
                    ui,
                    theme::prot_tile(th, Protection::Blocked(Reason::WindowsDir)),
                    tr(lang, Key::ProtLevelBlocked),
                );
            }
        }
        // Bedienhinweis rechts, gekürzt, wenn der Platz nicht reicht.
        if ui.available_width() > 60.0 {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(tr(lang, Key::TmHint))
                            .color(th.dim)
                            .font(widgets::mono(11.0)),
                    )
                    .truncate()
                    .selectable(false),
                );
            });
        }
    });
    row.response.on_hover_text(tr(lang, Key::TmHint));
}

/// Tiefste Kachel unter dem Punkt (Kacheln liegen in Zeichenreihenfolge, Kinder nach Eltern).
fn deepest_at(tiles: &[Tile], p: egui::Pos2) -> Option<&Tile> {
    tiles
        .iter()
        .filter(|t| t.rect.contains(p))
        .max_by_key(|t| t.depth)
}

/// Ordner direkt unter `zoom` auf dem Pfad zu `id` (oder `id` selbst, wenn Ordner).
fn zoom_target(tree: &Tree, zoom: NodeId, id: NodeId) -> Option<NodeId> {
    let mut cur = id;
    while cur != NO_NODE {
        let p = tree.node(cur).parent;
        if p == zoom {
            return tree.node(cur).is_dir().then_some(cur);
        }
        cur = p;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::vec2;

    fn area(r: &Rect) -> f64 {
        r.width() as f64 * r.height() as f64
    }

    fn overlap(a: &Rect, b: &Rect) -> f64 {
        let w = (a.max.x.min(b.max.x) - a.min.x.max(b.min.x)).max(0.0) as f64;
        let h = (a.max.y.min(b.max.y) - a.min.y.max(b.min.y)).max(0.0) as f64;
        w * h
    }

    fn check(items: &[(NodeId, u64)], rect: Rect) {
        let out = squarify(items, rect);
        let nonzero = items.iter().filter(|(_, s)| *s > 0).count();
        assert_eq!(out.len(), nonzero);
        let total: f64 = out.iter().map(|(_, r)| area(r)).sum();
        let want = area(&rect);
        assert!(
            (total - want).abs() <= want * 0.001,
            "Fläche {total} statt {want}"
        );
        for (i, (_, a)) in out.iter().enumerate() {
            assert!(rect.expand(0.01).contains_rect(*a), "{a:?} außerhalb");
            for (_, b) in &out[i + 1..] {
                assert!(overlap(a, b) < 0.01, "Überlappung {a:?} {b:?}");
            }
        }
        // Flächen proportional zur Größe (+-1 %)
        let sum: u64 = items.iter().map(|(_, s)| s).sum();
        for (id, r) in &out {
            let s = items.iter().find(|(i, _)| i == id).unwrap().1;
            let expect = want * s as f64 / sum as f64;
            assert!((area(r) - expect).abs() <= expect * 0.01 + 0.5);
        }
    }

    #[test]
    fn squarify_area_and_no_overlap() {
        let rect = Rect::from_min_size(pos2(10.0, 20.0), vec2(600.0, 400.0));
        check(
            &[(1, 6), (2, 6), (3, 4), (4, 3), (5, 2), (6, 2), (7, 1)],
            rect,
        );
        check(&[(1, 100)], rect);
        check(&[(1, 1), (2, 1), (3, 1), (4, 1)], rect);
        let many: Vec<(NodeId, u64)> = (0..500)
            .map(|i| (i, 1 + (i as u64 * 7919) % 1000))
            .collect();
        check(&many, rect);
        let tall = Rect::from_min_size(pos2(0.0, 0.0), vec2(50.0, 900.0));
        check(&many, tall);
    }

    #[test]
    fn squarify_unsorted_and_zero() {
        let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(300.0, 200.0));
        check(&[(1, 0), (2, 5), (3, 50), (4, 0), (5, 20)], rect);
        assert!(squarify(&[], rect).is_empty());
        assert!(squarify(&[(1, 0)], rect).is_empty());
        assert!(squarify(&[(1, 5)], Rect::NOTHING).is_empty());
    }

    #[test]
    fn squarify_is_reasonably_square() {
        let rect = Rect::from_min_size(pos2(0.0, 0.0), vec2(400.0, 400.0));
        let out = squarify(&[(1, 1), (2, 1), (3, 1), (4, 1)], rect);
        for (_, r) in out {
            let ratio = r.width().max(r.height()) / r.width().min(r.height());
            assert!(ratio < 1.01, "Verhältnis {ratio}");
        }
    }

    #[test]
    fn zoom_goes_one_level() {
        let t = jt_core::model::testutil::sample_tree();
        let jpg = t.find_path("C:\\Users\\joschy\\Bild.JPG").unwrap();
        let users = t.find_path("C:\\Users").unwrap();
        let joschy = t.find_path("C:\\Users\\joschy").unwrap();
        assert_eq!(zoom_target(&t, 0, jpg), Some(users));
        assert_eq!(zoom_target(&t, users, jpg), Some(joschy));
        let page = t.find_path("C:\\pagefile.sys").unwrap();
        assert_eq!(zoom_target(&t, 0, page), None);
    }
}
