// Joschys_Tree – Treemap (Squarified-Layout und Zeichnen) – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

use eframe::egui::{self, Align2, Color32, FontId, Rect, Sense, Stroke, StrokeKind, Ui, pos2};
use jt_core::format::bytes;
use jt_core::{Key, NO_NODE, NodeId, Tree, tr};

use crate::app::{Action, AppState, fill};
use crate::theme;
use crate::views::no_tree;

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
}

#[derive(Default)]
pub struct TreemapState {
    /// Aktueller Zoom-Knoten (0 = Wurzel).
    pub root: NodeId,
    tiles: Vec<Tile>,
    built_for: Option<(u64, NodeId, Rect)>,
}

impl TreemapState {
    pub fn invalidate(&mut self) {
        self.built_for = None;
    }

    fn rebuild(&mut self, tree: &Tree, rect: Rect, gen_: u64) {
        self.tiles.clear();
        layout_rec(tree, self.root, rect, 0, &mut self.tiles);
        self.built_for = Some((gen_, self.root, rect));
    }
}

fn layout_rec(tree: &Tree, dir: NodeId, rect: Rect, depth: u32, out: &mut Vec<Tile>) {
    let items: Vec<(NodeId, u64)> = tree
        .children(dir)
        .map(|c| (c, tree.node(c).size))
        .filter(|(_, s)| *s > 0)
        .collect();
    for (id, r) in squarify(&items, rect) {
        if out.len() >= MAX_TILES {
            return;
        }
        out.push(Tile { id, rect: r, depth });
        let n = tree.node(id);
        if n.is_dir()
            && depth < MAX_DEPTH
            && r.width() > MIN_SPLIT
            && r.height() > MIN_SPLIT + HEADER
        {
            let inner = Rect::from_min_max(
                pos2(r.min.x + 1.0, r.min.y + HEADER),
                pos2(r.max.x - 1.0, r.max.y - 1.0),
            );
            layout_rec(tree, id, inner, depth + 1, out);
        }
    }
}

pub fn show(ui: &mut Ui, state: &mut AppState) {
    let lang = state.lang();
    let Some(tree) = state.tree.clone() else {
        no_tree(ui, lang);
        return;
    };
    let tm = &mut state.treemap;
    if tm.root as usize >= tree.nodes.len() {
        tm.root = 0;
    }
    ui.horizontal(|ui| {
        let parent = tree.node(tm.root).parent;
        if ui
            .add_enabled(
                parent != NO_NODE,
                egui::Button::new(tr(lang, Key::TreemapZoomOut)),
            )
            .clicked()
        {
            tm.root = parent;
        }
        ui.label(tree.full_path(tm.root));
        ui.label(bytes(tree.node(tm.root).size, lang));
    });
    let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::click());
    let rect = resp.rect;
    if tm.built_for != Some((state.tree_gen, tm.root, rect)) {
        tm.rebuild(&tree, rect, state.tree_gen);
    }
    let dark = ui.visuals().dark_mode;
    if tm.tiles.is_empty() {
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            tr(lang, Key::TreemapEmpty),
            FontId::proportional(14.0),
            ui.visuals().weak_text_color(),
        );
        return;
    }
    let border = if dark {
        Color32::from_black_alpha(160)
    } else {
        Color32::from_white_alpha(200)
    };
    let text_col = if dark { Color32::WHITE } else { Color32::BLACK };
    for t in &tm.tiles {
        let n = tree.node(t.id);
        let fill = if n.is_dir() {
            theme::dir_color(dark)
        } else {
            theme::color_for_ext(tree.ext_of(t.id))
        };
        painter.rect_filled(t.rect, 0.0, fill);
        painter.rect_stroke(t.rect, 0.0, Stroke::new(0.5, border), StrokeKind::Inside);
        if n.is_dir() && t.rect.width() > 40.0 && t.rect.height() > HEADER {
            painter.with_clip_rect(t.rect).text(
                t.rect.min + egui::vec2(3.0, 1.0),
                Align2::LEFT_TOP,
                &n.name,
                FontId::proportional(10.0),
                text_col,
            );
        }
    }
    // Hervorhebung der Auswahl
    if let Some(sel) = state.selected
        && let Some(t) = tm.tiles.iter().find(|t| t.id == sel)
    {
        painter.rect_stroke(
            t.rect,
            0.0,
            Stroke::new(2.0, Color32::YELLOW),
            StrokeKind::Inside,
        );
    }
    let hovered = resp
        .hover_pos()
        .and_then(|p| deepest_at(&tm.tiles, p).map(|t| t.id));
    if let Some(id) = hovered {
        let n = tree.node(id);
        let tip = fill(
            tr(lang, Key::TreemapTooltip),
            &[
                ("{name}", &tree.full_path(id)),
                ("{size}", &bytes(n.size, lang)),
            ],
        );
        resp.clone().on_hover_text_at_pointer(tip);
    }
    if resp.clicked()
        && let Some(id) = hovered
    {
        state.actions.push(Action::Select(id));
        // Hineinzoomen: in den obersten Ordner unter dem Zoom-Knoten, der den Treffer enthält.
        if let Some(z) = zoom_target(&tree, tm.root, id) {
            tm.root = z;
        }
    }
    if resp.secondary_clicked() {
        let p = tree.node(tm.root).parent;
        if p != NO_NODE {
            tm.root = p;
        }
    }
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
        // Flächen proportional zur Größe (±1 %)
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
