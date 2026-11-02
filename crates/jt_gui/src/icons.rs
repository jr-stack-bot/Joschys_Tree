// Joschys_Tree – Selbst gezeichnete Symbole (keine Schrift-Glyphen): Geometrie und Zeichnen – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later
//
// Symbole werden nie als Unicode-Zeichen ausgegeben, weil Windows- und egui-Schriften
// viele davon nicht enthalten (leere Kästchen). Die Geometrie steckt in reinen
// Funktionen mit festen Arrays; die Zeichenfunktionen kommen ohne Heap-Speicher aus
// (Ausnahme: gefüllte Vielecke, die nur einmal pro Bild gezeichnet werden).

use eframe::egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, pos2, vec2};

/// Richtung eines Dreiecks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir {
    Up,
    Down,
    #[allow(dead_code)]
    Left,
    #[allow(dead_code)]
    Right,
}

/// Gleichschenkliges Dreieck mit Spitze in Richtung `dir`, zentriert um `c`.
pub fn triangle(c: Pos2, size: f32, dir: Dir) -> [Pos2; 3] {
    let h = size * 0.5;
    let w = size * 0.45;
    match dir {
        Dir::Up => [
            pos2(c.x, c.y - w),
            pos2(c.x + h, c.y + w),
            pos2(c.x - h, c.y + w),
        ],
        Dir::Down => [
            pos2(c.x - h, c.y - w),
            pos2(c.x + h, c.y - w),
            pos2(c.x, c.y + w),
        ],
        Dir::Left => [
            pos2(c.x - w, c.y),
            pos2(c.x + w, c.y - h),
            pos2(c.x + w, c.y + h),
        ],
        Dir::Right => [
            pos2(c.x - w, c.y - h),
            pos2(c.x + w, c.y),
            pos2(c.x - w, c.y + h),
        ],
    }
}

/// Eck-Dreieck unten rechts (Zierelement im Fensterkopf).
pub fn corner_wedge(r: Rect) -> [Pos2; 3] {
    [r.right_top(), r.right_bottom(), r.left_bottom()]
}

/// Plus/Minus-Kästchen zum Auf-/Zuklappen: Rahmen und Linien.
/// Zugeklappt: Plus (zwei Linien), aufgeklappt: Minus (eine Linie, zweite leer).
pub fn expander(c: Pos2, size: f32, open: bool) -> (Rect, [[Pos2; 2]; 2], usize) {
    let half = (size * 0.5).round();
    let c = pos2(c.x.round() + 0.5, c.y.round() + 0.5);
    let frame = Rect::from_center_size(c, vec2(half * 2.0, half * 2.0));
    let a = half - 2.0;
    let h = [pos2(c.x - a, c.y), pos2(c.x + a, c.y)];
    let v = [pos2(c.x, c.y - a), pos2(c.x, c.y + a)];
    (frame, [h, v], if open { 1 } else { 2 })
}

/// Ordner: Reiter oben links und Körper.
pub fn folder(r: Rect) -> (Rect, Rect) {
    let tab_h = (r.height() * 0.18).max(2.0);
    let top = r.top() + r.height() * 0.12;
    let tab = Rect::from_min_max(
        pos2(r.left(), top),
        pos2(r.left() + r.width() * 0.45, top + tab_h),
    );
    let body = Rect::from_min_max(
        pos2(r.left(), tab.bottom()),
        pos2(r.right(), r.bottom() - r.height() * 0.08),
    );
    (tab, body)
}

/// Datei mit Eselsohr: Umriss (5 Punkte, geschlossen) und Falz (3 Punkte, offen).
pub fn file(r: Rect) -> ([Pos2; 5], [Pos2; 3]) {
    let w = r.height() * 0.72;
    let l = r.center().x - w * 0.5;
    let rt = l + w;
    let (t, b) = (r.top() + 0.5, r.bottom() - 0.5);
    let fold = w * 0.38;
    (
        [
            pos2(l, t),
            pos2(rt - fold, t),
            pos2(rt, t + fold),
            pos2(rt, b),
            pos2(l, b),
        ],
        [
            pos2(rt - fold, t),
            pos2(rt - fold, t + fold),
            pos2(rt, t + fold),
        ],
    )
}

/// Verknüpfung/Reparse-Punkt: kleiner Pfeil nach rechts oben, unten links im Symbol.
pub fn link_arrow(r: Rect) -> [[Pos2; 2]; 3] {
    let s = r.height() * 0.5;
    let o = pos2(r.left() + 1.0, r.bottom() - 1.0);
    let tip = pos2(o.x + s, o.y - s);
    let a = s * 0.55;
    [
        [o, tip],
        [tip, pos2(tip.x - a, tip.y)],
        [tip, pos2(tip.x, tip.y + a)],
    ]
}

/// Schloss (eckig): Körper und Bügel als offener Linienzug.
pub fn lock(r: Rect) -> (Rect, [Pos2; 4]) {
    let s = r.height() * 0.55;
    let body = Rect::from_min_max(
        pos2(r.right() - s, r.bottom() - s * 0.6),
        pos2(r.right(), r.bottom()),
    );
    let i = s * 0.22;
    let top = body.top() - s * 0.45;
    (
        body,
        [
            pos2(body.left() + i, body.top()),
            pos2(body.left() + i, top),
            pos2(body.right() - i, top),
            pos2(body.right() - i, body.top()),
        ],
    )
}

/// Schild (Admin-Rechte): Fünfeck mit Spitze unten.
pub fn shield(r: Rect) -> [Pos2; 5] {
    let w = r.height() * 0.8;
    let l = r.center().x - w * 0.5;
    let rt = l + w;
    let t = r.top();
    let b = r.bottom();
    let mid = t + (b - t) * 0.55;
    [
        pos2(l, t),
        pos2(rt, t),
        pos2(rt, mid),
        pos2(r.center().x, b),
        pos2(l, mid),
    ]
}

/// Schließen-Kreuz.
pub fn cross(r: Rect) -> [[Pos2; 2]; 2] {
    [
        [r.left_top(), r.right_bottom()],
        [r.right_top(), r.left_bottom()],
    ]
}

/// Rechteck mit abgeschrägten Ecken (Fase `c`), im Uhrzeigersinn.
pub fn chamfer(r: Rect, c: f32) -> [Pos2; 8] {
    let c = c.min(r.width() * 0.5).min(r.height() * 0.5).max(0.0);
    [
        pos2(r.left() + c, r.top()),
        pos2(r.right() - c, r.top()),
        pos2(r.right(), r.top() + c),
        pos2(r.right(), r.bottom() - c),
        pos2(r.right() - c, r.bottom()),
        pos2(r.left() + c, r.bottom()),
        pos2(r.left(), r.bottom() - c),
        pos2(r.left(), r.top() + c),
    ]
}

/// Eck-Klammern an allen vier Ecken: je zwei Linien (waagrecht, senkrecht).
pub fn corner_brackets(r: Rect, len: f32) -> [[Pos2; 2]; 8] {
    let l = len.min(r.width() * 0.5).min(r.height() * 0.5);
    let (a, b, c, d) = (
        r.left_top(),
        r.right_top(),
        r.right_bottom(),
        r.left_bottom(),
    );
    [
        [a, pos2(a.x + l, a.y)],
        [a, pos2(a.x, a.y + l)],
        [b, pos2(b.x - l, b.y)],
        [b, pos2(b.x, b.y + l)],
        [c, pos2(c.x - l, c.y)],
        [c, pos2(c.x, c.y - l)],
        [d, pos2(d.x + l, d.y)],
        [d, pos2(d.x, d.y - l)],
    ]
}

// ---------------------------------------------------------------------------
// Zeichnen

/// Geschlossener Linienzug ohne Heap-Speicher.
fn closed(p: &Painter, pts: &[Pos2], stroke: Stroke) {
    for (i, a) in pts.iter().enumerate() {
        let b = pts[(i + 1) % pts.len()];
        p.line_segment([*a, b], stroke);
    }
}

fn open(p: &Painter, pts: &[Pos2], stroke: Stroke) {
    for w in pts.windows(2) {
        p.line_segment([w[0], w[1]], stroke);
    }
}

/// Gefülltes Dreieck (eine kleine Zuweisung; nur für Einzelfälle wie Sortierpfeil).
pub fn paint_triangle(p: &Painter, c: Pos2, size: f32, dir: Dir, color: Color32) {
    p.add(Shape::convex_polygon(
        triangle(c, size, dir).to_vec(),
        color,
        Stroke::NONE,
    ));
}

/// Plus/Minus-Kästchen.
pub fn paint_expander(
    p: &Painter,
    c: Pos2,
    size: f32,
    is_open: bool,
    frame: Color32,
    sign: Color32,
) {
    let (r, lines, n) = expander(c, size, is_open);
    p.rect_stroke(r, 0.0, Stroke::new(1.0, frame), StrokeKind::Inside);
    for l in &lines[..n] {
        p.line_segment(*l, Stroke::new(1.0, sign));
    }
}

/// Art des Knoten-Symbols.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeIcon {
    Folder,
    File,
}

/// Ordner- oder Datei-Symbol mit optionalen Zusätzen (Verknüpfung, gesperrt).
#[allow(clippy::too_many_arguments)]
pub fn paint_node_icon(
    p: &Painter,
    r: Rect,
    kind: NodeIcon,
    color: Color32,
    fill: Color32,
    reparse: Option<Color32>,
    locked: Option<Color32>,
) {
    match kind {
        NodeIcon::Folder => {
            let (tab, body) = folder(r);
            p.rect_filled(tab, 0.0, color);
            p.rect_filled(body, 0.0, fill);
            p.rect_stroke(body, 0.0, Stroke::new(1.0, color), StrokeKind::Inside);
        }
        NodeIcon::File => {
            let (outline, fold) = file(r);
            let s = Stroke::new(1.0, color);
            closed(p, &outline, s);
            open(p, &fold, s);
        }
    }
    if let Some(c) = reparse {
        for l in link_arrow(r) {
            p.line_segment(l, Stroke::new(1.5, c));
        }
    }
    if let Some(c) = locked {
        let (body, shackle) = lock(r);
        p.rect_filled(body, 0.0, c);
        open(p, &shackle, Stroke::new(1.2, c));
    }
}

/// Schild (gefüllt oder als Umriss).
pub fn paint_shield(p: &Painter, r: Rect, color: Color32, filled: bool) {
    let pts = shield(r);
    if filled {
        p.add(Shape::convex_polygon(pts.to_vec(), color, Stroke::NONE));
    } else {
        closed(p, &pts, Stroke::new(1.2, color));
    }
}

pub fn paint_cross(p: &Painter, r: Rect, stroke: Stroke) {
    for l in cross(r) {
        p.line_segment(l, stroke);
    }
}

pub fn paint_corner_brackets(p: &Painter, r: Rect, len: f32, stroke: Stroke) {
    for l in corner_brackets(r, len) {
        p.line_segment(l, stroke);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Erlaubte Nicht-ASCII-Zeichen (als Escape, damit diese Datei selbst sauber bleibt):
    /// deutsche Umlaute und Eszett, Halbgeviertstrich, Mittelpunkt, Auslassung, Copyright.
    const ALLOWED: [char; 11] = [
        '\u{e4}', '\u{f6}', '\u{fc}', '\u{c4}', '\u{d6}', '\u{dc}', '\u{df}', '\u{2013}', '\u{b7}',
        '\u{2026}', '\u{a9}',
    ];

    fn rs_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                rs_files(&p, out);
            } else if p.extension().is_some_and(|x| x == "rs") {
                out.push(p);
            }
        }
    }

    /// Regressionstest gegen leere Kästchen: Symbole (Pfeile, Dreiecke, Emoji, ...)
    /// dürfen im GUI-Quelltext nicht als Zeichen vorkommen, sondern werden gezeichnet.
    #[test]
    fn no_glyph_symbols_in_gui_sources() {
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = Vec::new();
        rs_files(&src, &mut files);
        assert!(files.len() >= 15);
        let mut bad = Vec::new();
        for f in files {
            let text = std::fs::read_to_string(&f).unwrap();
            for (n, line) in text.lines().enumerate() {
                for c in line.chars() {
                    if !c.is_ascii() && !ALLOWED.contains(&c) {
                        bad.push(format!("{}:{}: U+{:04X}", f.display(), n + 1, u32::from(c)));
                    }
                }
            }
        }
        assert!(bad.is_empty(), "Verbotene Symbol-Zeichen: {bad:#?}");
    }

    fn sq() -> Rect {
        Rect::from_min_size(pos2(10.0, 20.0), vec2(16.0, 16.0))
    }

    fn inside(r: Rect, p: Pos2) -> bool {
        r.expand(0.51).contains(p)
    }

    #[test]
    fn triangle_points_in_direction() {
        let c = pos2(50.0, 50.0);
        let up = triangle(c, 10.0, Dir::Up);
        assert!(up[0].y < c.y && up[1].y > c.y && up[2].y > c.y);
        let down = triangle(c, 10.0, Dir::Down);
        assert!(down[2].y > down[0].y && down[0].y == down[1].y);
        let right = triangle(c, 10.0, Dir::Right);
        assert!(right[1].x > right[0].x && right[0].x == right[2].x);
        let left = triangle(c, 10.0, Dir::Left);
        assert!(left[0].x < left[1].x);
        // Schwerpunkt nahe der Mitte, Fläche > 0
        for t in [up, down, left, right] {
            let area = ((t[1].x - t[0].x) * (t[2].y - t[0].y)
                - (t[2].x - t[0].x) * (t[1].y - t[0].y))
                .abs();
            assert!(area > 10.0);
            assert!(
                t.iter()
                    .all(|p| (p.x - c.x).abs() <= 5.0 && (p.y - c.y).abs() <= 5.0)
            );
        }
    }

    #[test]
    fn expander_plus_and_minus() {
        let (r, lines, n) = expander(pos2(20.0, 20.0), 9.0, false);
        assert_eq!(n, 2);
        assert!(r.width() >= 8.0);
        assert!(lines.iter().flatten().all(|p| r.contains(*p)));
        // waagrecht und senkrecht
        assert_eq!(lines[0][0].y, lines[0][1].y);
        assert_eq!(lines[1][0].x, lines[1][1].x);
        let (_, _, n) = expander(pos2(20.0, 20.0), 9.0, true);
        assert_eq!(n, 1);
    }

    #[test]
    fn folder_tab_sits_on_body() {
        let (tab, body) = folder(sq());
        assert_eq!(tab.bottom(), body.top());
        assert!(tab.width() < body.width());
        assert!(inside(sq(), tab.min) && inside(sq(), body.max));
    }

    #[test]
    fn file_has_dog_ear() {
        let (o, fold) = file(sq());
        assert!(o.iter().all(|p| inside(sq(), *p)));
        // Eselsohr: Falzpunkt liegt innerhalb des Umrisses, schräge Kante oben rechts
        assert!(fold[1].x < o[2].x && fold[1].y > o[1].y);
        assert_eq!(o[1], fold[0]);
        assert_eq!(o[2], fold[2]);
        // Hochformat
        assert!(o[3].y - o[1].y > o[3].x - o[0].x);
    }

    #[test]
    fn small_symbols_stay_in_rect() {
        let r = sq();
        assert!(link_arrow(r).iter().flatten().all(|p| inside(r, *p)));
        let (body, shackle) = lock(r);
        assert!(inside(r, body.min) && inside(r, body.max));
        assert!(shackle.iter().all(|p| inside(r, *p)));
        assert!(shield(r).iter().all(|p| inside(r, *p)));
        assert!(cross(r).iter().flatten().all(|p| inside(r, *p)));
        assert!(corner_wedge(r).iter().all(|p| inside(r, *p)));
        let br = corner_brackets(r, 4.0);
        assert!(br.iter().flatten().all(|p| inside(r, *p)));
    }

    #[test]
    fn chamfer_is_octagon_inside_rect() {
        let r = Rect::from_min_size(pos2(0.0, 0.0), vec2(100.0, 30.0));
        let pts = chamfer(r, 6.0);
        assert!(pts.iter().all(|p| inside(r, *p)));
        assert_eq!(pts[0], pos2(6.0, 0.0));
        assert_eq!(pts[3], pos2(100.0, 24.0));
        // Fase wird auf halbe Höhe begrenzt
        let pts = chamfer(r, 100.0);
        assert_eq!(pts[2], pos2(100.0, 15.0));
    }
}
