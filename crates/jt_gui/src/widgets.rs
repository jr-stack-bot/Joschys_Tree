// Joschys_Tree – Eigene Bedienelemente im Reaktor-Stil (eckig, 1-px-Linien, Akzent-Leiste) – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::time::Duration;

use eframe::egui::{
    self, Align2, Color32, FontFamily, FontId, Id, Mesh, Painter, Pos2, Rangef, Rect, Response,
    Sense, Shape, Stroke, StrokeKind, Ui, Vec2, pos2, vec2,
};
use jt_core::Lang;

use crate::icons::{self, Dir};
use crate::theme::{self, Theme, bar_gradient, bar_level, lerp_color, with_alpha};

/// Breite der leuchtenden Akzent-Leiste links an Zeilen und Knöpfen.
pub const BAR_W: f32 = 3.0;
/// Dauer weicher Hover-Übergänge in Sekunden.
pub const HOVER_T: f32 = 0.15;

pub fn mono(size: f32) -> FontId {
    FontId::new(size, FontFamily::Monospace)
}

pub fn title_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}

/// Text in Großbuchstaben (Überschriften, Tabs, Spaltenköpfe).
pub fn upper(s: &str) -> String {
    s.to_uppercase()
}

/// Waagrechter Farbverlauf als Viereck in einem gemeinsamen Mesh.
pub fn gradient_quad(mesh: &mut Mesh, r: Rect, left: Color32, right: Color32) {
    let i = mesh.vertices.len() as u32;
    mesh.colored_vertex(r.left_top(), left);
    mesh.colored_vertex(r.right_top(), right);
    mesh.colored_vertex(r.right_bottom(), right);
    mesh.colored_vertex(r.left_bottom(), left);
    mesh.add_triangle(i, i + 1, i + 2);
    mesh.add_triangle(i, i + 2, i + 3);
}

/// Einzelner Verlaufsbalken (eine kleine Zuweisung; nur für Einzelfälle).
pub fn paint_gradient(p: &Painter, r: Rect, left: Color32, right: Color32) {
    if r.width() <= 0.0 {
        return;
    }
    let mut m = Mesh::default();
    gradient_quad(&mut m, r, left, right);
    p.add(Shape::mesh(m));
}

/// Dezente Scanlines (alle 3 px eine dunkle 1-px-Linie).
pub fn scanlines(p: &Painter, r: Rect, t: &Theme) {
    if !t.glow {
        return;
    }
    let c = Color32::from_black_alpha(40);
    let mut y = r.top() + 1.5;
    while y < r.bottom() {
        p.hline(r.x_range(), y, Stroke::new(1.0, c));
        y += 3.0;
    }
}

/// Weiche Aura (gestapelte Konturen mit fallender Deckkraft) statt Unschärfe.
pub fn glow(p: &Painter, r: Rect, color: Color32, strength: f32) {
    for (w, a) in [(2.0, 40.0), (4.0, 22.0), (6.0, 10.0)] {
        let alpha = (a * strength).clamp(0.0, 255.0) as u8;
        p.rect_stroke(
            r.expand(w * 0.5),
            0.0,
            Stroke::new(w, with_alpha(color, alpha)),
            StrokeKind::Outside,
        );
    }
}

/// Titel mit Farbsaum (links Akzent, rechts Rot, Text obenauf); hell ohne Saum.
pub fn fringe_text(p: &Painter, pos: Pos2, anchor: Align2, text: &str, font: FontId, t: &Theme) {
    if t.glow {
        p.text(
            pos - vec2(1.0, 0.0),
            anchor,
            text,
            font.clone(),
            with_alpha(t.accent, 150),
        );
        p.text(
            pos + vec2(1.0, 0.0),
            anchor,
            text,
            font.clone(),
            with_alpha(t.red, 150),
        );
    }
    p.text(pos, anchor, text, font, t.text_hi);
}

/// Abschnittstitel: 6x6-Kennquadrat, GROSSER gedämpfter Text, 1-px-Linie bis zum Rand.
pub fn section(ui: &mut Ui, text: &str, mark: Color32) {
    let t = Theme::of(ui);
    let font = mono(11.0);
    let galley = ui.painter().layout_no_wrap(upper(text), font, t.dim);
    let h = galley.size().y.max(12.0) + 6.0;
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::hover());
    let p = ui.painter();
    let sq = Rect::from_center_size(pos2(r.left() + 3.0, r.center().y), vec2(6.0, 6.0));
    p.rect_filled(sq, 0.0, mark);
    let tx = sq.right() + 6.0;
    let gw = galley.size().x;
    p.galley(
        pos2(tx, r.center().y - galley.size().y * 0.5),
        galley,
        t.dim,
    );
    let lx = tx + gw + 8.0;
    if lx < r.right() {
        p.hline(
            lx..=r.right(),
            r.center().y.round() + 0.5,
            Stroke::new(1.0, t.line),
        );
    }
}

fn button_frame(
    ui: &mut Ui,
    text: &str,
    font: FontId,
    pad: Vec2,
    enabled: bool,
) -> (Response, std::sync::Arc<egui::Galley>) {
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), font, Color32::PLACEHOLDER);
    let size = galley.size() + pad * 2.0;
    let size = vec2(size.x, size.y.max(ui.spacing().interact_size.y));
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (_, resp) = ui.allocate_exact_size(size, sense);
    (resp, galley)
}

/// Segment-Tab: aktiv voll in Akzent, inaktiv Zeilengrund, Hover mit Akzent-Text.
pub fn tab_button(ui: &mut Ui, selected: bool, text: &str) -> Response {
    let t = Theme::of(ui);
    let (resp, galley) = button_frame(ui, &upper(text), mono(12.0), vec2(9.0, 4.0), true);
    let r = resp.rect;
    let hov = ui
        .ctx()
        .animate_bool_with_time(resp.id, resp.hovered(), HOVER_T);
    let p = ui.painter();
    let (bg, fg) = if selected {
        (t.accent_fill, t.on_accent)
    } else {
        (
            lerp_color(t.row, t.row_hover, hov),
            lerp_color(t.text, t.accent_text, hov),
        )
    };
    if selected && t.glow && theme::effects(ui) {
        glow(p, r, t.accent, 0.8);
    }
    p.rect_filled(r, 0.0, bg);
    if !selected {
        p.hline(
            r.x_range(),
            r.bottom() - 0.5,
            Stroke::new(1.0, with_alpha(t.accent, (hov * 255.0) as u8)),
        );
    }
    p.galley(r.center() - galley.size() * 0.5, galley, fg);
    resp
}

/// Flacher Werkzeug-Knopf im Menüzeilen-Stil (links 3-px-Leiste bei Hover).
pub fn tool_button(ui: &mut Ui, text: &str, enabled: bool) -> Response {
    styled_button(ui, text, enabled, false)
}

/// Destruktiver Knopf (roter Text).
pub fn danger_button(ui: &mut Ui, text: &str, enabled: bool) -> Response {
    styled_button(ui, text, enabled, true)
}

fn styled_button(ui: &mut Ui, text: &str, enabled: bool, danger: bool) -> Response {
    let t = Theme::of(ui);
    let (resp, galley) = button_frame(ui, text, title_font(14.0), vec2(10.0, 3.0), enabled);
    let r = resp.rect;
    let hov = if enabled {
        ui.ctx()
            .animate_bool_with_time(resp.id, resp.hovered(), HOVER_T)
    } else {
        0.0
    };
    let p = ui.painter();
    let base = if danger { t.red } else { t.text };
    let fg = if !enabled {
        with_alpha(t.dim, 120)
    } else if danger {
        t.red
    } else {
        lerp_color(base, t.accent_text, hov)
    };
    p.rect_filled(r, 0.0, lerp_color(t.row, t.row_hover, hov));
    p.rect_stroke(
        r,
        0.0,
        Stroke::new(
            1.0,
            if enabled {
                t.line
            } else {
                with_alpha(t.line, 120)
            },
        ),
        StrokeKind::Inside,
    );
    if hov > 0.0 {
        let bar = Rect::from_min_size(r.min, vec2(BAR_W, r.height()));
        p.rect_filled(
            bar,
            0.0,
            with_alpha(if danger { t.red } else { t.accent }, (hov * 255.0) as u8),
        );
    }
    if resp.is_pointer_button_down_on() && enabled {
        p.rect_stroke(r, 0.0, Stroke::new(1.0, t.accent), StrokeKind::Inside);
    }
    p.galley(
        r.center() - galley.size() * 0.5 + vec2(1.0, 0.0),
        galley,
        fg,
    );
    resp
}

/// Primäraktion: invertiert (Akzentfläche, dunkler Text), abgeschrägte Ecken.
pub fn primary_button(ui: &mut Ui, text: &str, enabled: bool) -> Response {
    let t = Theme::of(ui);
    let (resp, galley) = button_frame(ui, &upper(text), mono(13.0), vec2(14.0, 4.0), enabled);
    let r = resp.rect;
    let hov = if enabled {
        ui.ctx()
            .animate_bool_with_time(resp.id, resp.hovered(), HOVER_T)
    } else {
        0.0
    };
    let p = ui.painter();
    let fill = if enabled {
        lerp_color(
            t.accent_fill,
            if t.dark { t.accent_hi } else { t.sel_text },
            hov * 0.5,
        )
    } else {
        t.row
    };
    if enabled && t.glow && theme::effects(ui) {
        glow(p, r, t.accent, 0.4 + 0.6 * hov);
    }
    let pts = icons::chamfer(r, 6.0);
    p.add(Shape::convex_polygon(
        pts.to_vec(),
        fill,
        Stroke::new(1.6, if enabled { t.accent } else { t.line }),
    ));
    let fg = if enabled { t.on_accent } else { t.dim };
    p.galley(r.center() - galley.size() * 0.5, galley, fg);
    resp
}

/// Auswahlfeld (statt runder Radio-Knöpfe): eckiges Kennquadrat plus Text.
pub fn choice<T: PartialEq + Copy>(ui: &mut Ui, current: &mut T, value: T, text: &str) -> Response {
    let t = Theme::of(ui);
    let on = *current == value;
    let galley = ui.painter().layout_no_wrap(
        text.to_owned(),
        ui.style().text_styles[&egui::TextStyle::Body].clone(),
        Color32::PLACEHOLDER,
    );
    let box_s = 10.0;
    let size = vec2(
        box_s + 6.0 + galley.size().x + 4.0,
        galley.size().y.max(18.0),
    );
    let sense = if ui.is_enabled() {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (r, mut resp) = ui.allocate_exact_size(size, sense);
    if resp.clicked() && !on {
        *current = value;
        resp.mark_changed();
    }
    let hov = ui
        .ctx()
        .animate_bool_with_time(resp.id, resp.hovered(), HOVER_T);
    let p = ui.painter();
    let b = Rect::from_center_size(
        pos2(r.left() + box_s * 0.5 + 0.5, r.center().y),
        vec2(box_s, box_s),
    );
    let edge = lerp_color(t.line, t.accent, hov.max(if on { 1.0 } else { 0.0 }));
    p.rect_stroke(b, 0.0, Stroke::new(1.0, edge), StrokeKind::Inside);
    if on {
        p.rect_filled(b.shrink(2.5), 0.0, t.accent);
    }
    let mut fg = if on {
        t.accent_text
    } else {
        lerp_color(t.text, t.accent_text, hov)
    };
    if !ui.is_enabled() {
        fg = with_alpha(t.dim, 120);
    }
    p.galley(
        pos2(b.right() + 6.0, r.center().y - galley.size().y * 0.5),
        galley,
        fg,
    );
    resp
}

/// Wartesymbol ohne Kreis: wandernder Block in einer schmalen Spur.
pub fn busy(ui: &mut Ui) {
    let t = Theme::of(ui);
    let (r, _) = ui.allocate_exact_size(vec2(36.0, 14.0), Sense::hover());
    let track = Rect::from_center_size(r.center(), vec2(r.width(), 4.0));
    let p = ui.painter();
    p.rect_filled(track, 0.0, t.row);
    let time = ui.input(|i| i.time) as f32;
    let seg = 10.0;
    let x = (time * 40.0) % (track.width() + seg) - seg;
    let s = Rect::from_min_size(
        pos2(track.left() + x, track.top()),
        vec2(seg, track.height()),
    )
    .intersect(track);
    if s.width() > 0.0 {
        p.rect_filled(s, 0.0, t.accent);
    }
    ui.ctx().request_repaint_after(Duration::from_millis(33));
}

/// Fortschrittsbalken mit Verlauf und lesbarem Text rechts daneben.
pub fn progress(ui: &mut Ui, frac: f32, text: &str, width: f32) {
    let t = Theme::of(ui);
    let frac = frac.clamp(0.0, 1.0);
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), mono(12.0), t.text);
    let (r, _) = ui.allocate_exact_size(vec2(width + 8.0 + galley.size().x, 16.0), Sense::hover());
    let bar = Rect::from_min_size(pos2(r.left(), r.center().y - 5.0), vec2(width, 10.0));
    let p = ui.painter();
    p.rect_filled(bar, 0.0, t.row);
    let mut f = bar;
    f.set_width(bar.width() * frac);
    paint_gradient(p, f, t.accent, t.accent_hi);
    p.rect_stroke(bar, 0.0, Stroke::new(1.0, t.line), StrokeKind::Inside);
    p.galley(
        pos2(bar.right() + 8.0, r.center().y - galley.size().y * 0.5),
        galley,
        t.text,
    );
}

/// Sammelt die Verlaufsflächen aller Anteilsbalken einer Tabelle in einem Mesh,
/// damit pro Zeile keine Speicherzuweisung nötig ist.
pub struct BarBatch {
    mesh: Mesh,
    clip: Rect,
}

impl Default for BarBatch {
    fn default() -> Self {
        BarBatch {
            mesh: Mesh::default(),
            clip: Rect::NOTHING,
        }
    }
}

impl BarBatch {
    /// Zeichnet alle gesammelten Füllungen (einmal pro Bild).
    pub fn flush(self, ui: &Ui) {
        if !self.mesh.is_empty() {
            ui.painter()
                .with_clip_rect(self.clip)
                .add(Shape::mesh(self.mesh));
        }
    }
}

/// Breite des Prozenttextes rechts neben dem Balken.
const PCT_W: f32 = 62.0;

/// Anteilsbalken in einer Tabellenzelle: Spur, Verlauf (Farbe warnt bei hohem Anteil),
/// Prozent rechtsbündig AUSSERHALB der Füllung, daher immer lesbar.
pub fn percent_bar(ui: &mut Ui, frac: f32, lang: Lang, warn: bool, batch: &mut BarBatch) {
    let t = Theme::of(ui);
    let frac = if frac.is_finite() {
        frac.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let w = ui.available_width().clamp(PCT_W + 20.0, 360.0);
    let h = ui.available_height().clamp(14.0, 22.0);
    let (r, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
    let bar = Rect::from_min_max(
        pos2(r.left(), r.center().y - 5.0),
        pos2(r.right() - PCT_W, r.center().y + 5.0),
    );
    let p = ui.painter();
    p.rect_filled(bar, 0.0, t.row);
    p.rect_stroke(bar, 0.0, Stroke::new(1.0, t.line), StrokeKind::Inside);
    let level = if warn {
        bar_level(frac)
    } else {
        theme::BarLevel::Normal
    };
    let (a, b) = bar_gradient(t, level);
    if frac > 0.0 {
        let fill = Rect::from_min_size(bar.min, vec2((bar.width() * frac).max(1.0), bar.height()));
        if t.glow {
            p.rect_filled(fill.expand2(vec2(0.0, 2.0)), 0.0, with_alpha(a, 40));
        }
        gradient_quad(&mut batch.mesh, fill, a, lerp_color(a, b, frac));
        batch.clip = batch.clip.union(ui.clip_rect());
    }
    let col = match level {
        theme::BarLevel::Normal => t.text2,
        _ => t.warn,
    };
    p.text(
        pos2(r.right() - 2.0, r.center().y),
        Align2::RIGHT_CENTER,
        crate::views::fmt_percent(frac, lang),
        mono(12.0),
        col,
    );
}

/// Spaltenkopf: GROSS, gedämpft; aktive Sortierspalte in Akzent mit gezeichnetem Dreieck.
/// `sort`: `None` = nicht sortierbar, `Some(None)` = sortierbar, `Some(Some(desc))` = aktiv.
pub fn header_cell(ui: &mut Ui, text: &str, sort: Option<Option<bool>>) -> Response {
    let t = Theme::of(ui);
    let sense = if sort.is_some() {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (r, resp) = ui.allocate_exact_size(ui.available_size(), sense);
    let hov = sort.is_some() && resp.hovered();
    let active = matches!(sort, Some(Some(_)));
    let col = if active || hov { t.accent_text } else { t.dim };
    let p = ui.painter();
    let galley = p.layout_no_wrap(upper(text), mono(11.0), col);
    let gw = galley.size().x;
    p.galley(
        pos2(r.left() + 2.0, r.center().y - galley.size().y * 0.5),
        galley,
        col,
    );
    if let Some(Some(desc)) = sort {
        let c = pos2(r.left() + 2.0 + gw + 9.0, r.center().y);
        icons::paint_triangle(p, c, 8.0, if desc { Dir::Down } else { Dir::Up }, t.accent);
    }
    if hov {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

/// Hover-/Auswahlmarkierung von Tabellenzeilen: 3-px-Akzentleiste links mit weichem
/// Ein- und Ausblenden. Merkt sich nur die aktuelle und die vorige Hover-Zeile.
#[derive(Clone, Copy, Default)]
struct HoverMemo {
    cur: Option<u64>,
    prev: Option<u64>,
}

pub struct TableFx {
    id: Id,
    x: Rangef,
    memo: HoverMemo,
    seen: Option<u64>,
}

impl TableFx {
    /// Vor dem `TableBuilder` aufrufen (merkt sich die Tabellenbreite).
    pub fn begin(ui: &Ui, salt: &str) -> Self {
        let id = Id::unique(("jt_rowfx", salt));
        TableFx {
            id,
            x: ui.available_rect_before_wrap().x_range(),
            memo: ui.data(|d| d.get_temp::<HoverMemo>(id)).unwrap_or_default(),
            seen: None,
        }
    }

    /// Im ersten Feld einer Zeile aufrufen.
    pub fn mark(&mut self, ui: &Ui, key: u64, selected: bool) {
        let r = ui.max_rect();
        let clip = ui.clip_rect();
        let x = self.x;
        let hovered = ui.ctx().pointer_hover_pos().is_some_and(|p| {
            p.y >= r.top() && p.y < r.bottom() && clip.y_range().contains(p.y) && x.contains(p.x)
        });
        if hovered {
            self.seen = Some(key);
        }
        let anim = if hovered || self.memo.cur == Some(key) || self.memo.prev == Some(key) {
            ui.ctx()
                .animate_bool_with_time(self.id.with(key), hovered, HOVER_T)
        } else {
            0.0
        };
        let a = if selected { 1.0 } else { anim };
        if a > 0.0 {
            let t = Theme::of(ui);
            let bar = Rect::from_min_size(r.min, vec2(BAR_W, r.height()));
            ui.painter()
                .rect_filled(bar, 0.0, with_alpha(t.accent, (a * 255.0) as u8));
        }
    }

    /// Nach der Tabelle aufrufen.
    pub fn end(self, ui: &Ui) {
        let mut m = self.memo;
        if self.seen != m.cur {
            m.prev = m.cur;
            m.cur = self.seen;
        }
        ui.data_mut(|d| d.insert_temp(self.id, m));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gradient_quad_builds_two_triangles() {
        let mut m = Mesh::default();
        let r = Rect::from_min_size(pos2(0.0, 0.0), vec2(10.0, 4.0));
        gradient_quad(&mut m, r, Color32::RED, Color32::BLUE);
        gradient_quad(&mut m, r, Color32::RED, Color32::BLUE);
        assert_eq!(m.vertices.len(), 8);
        assert_eq!(m.indices.len(), 12);
        assert_eq!(m.vertices[0].color, Color32::RED);
        assert_eq!(m.vertices[1].color, Color32::BLUE);
        assert!(m.indices.iter().all(|&i| (i as usize) < m.vertices.len()));
        assert_eq!(&m.indices[6..9], &[4, 5, 6]);
    }

    #[test]
    fn upper_case_keeps_umlauts() {
        assert_eq!(upper("Größe"), "GRÖSSE");
    }
}
