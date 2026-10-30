// Joschys_Tree – PDF-Export via pdf-writer mit Fuß – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

//! PDF-Tabelle in A4 quer mit den Standard-Fonts Helvetica/Helvetica-Bold
//! (WinAnsiEncoding, kein Font-Embedding). Auf jeder Seite: Kopfzeile mit Titel
//! und Tabellenkopf, Fuß "Erstellt mit … · <Datum>" links und "@ Joschy · Seite n/m" rechts.
//! Zeichen außerhalb von WinAnsi werden durch `?` ersetzt. Content-Streams sind
//! unkomprimiert (keine zusätzliche Abhängigkeit, im Test direkt durchsuchbar).

use std::io::Write;

use pdf_writer::{Content, Finish, Name, Pdf, Rect, Ref, Str, TextStr};

use super::{Cell, DocInfo, ExportError, Table, display_cell};
use crate::i18n::{Key, tr};

// A4 quer in Punkt.
const PAGE_W: f32 = 842.0;
const PAGE_H: f32 = 595.0;
const MARGIN: f32 = 36.0;
const FONT_SIZE: f32 = 8.0;
const ROW_H: f32 = 12.0;
const PAD: f32 = 3.0;
const TITLE_SIZE: f32 = 14.0;
const SUB_SIZE: f32 = 9.0;
const FOOT_SIZE: f32 = 7.5;
/// Oberkante des Tabellenkopfs (unter Titel und Untertitel).
const TABLE_TOP: f32 = PAGE_H - MARGIN - 34.0;
/// Unterkante des Tabellenbereichs (über dem Fuß).
const TABLE_BOTTOM: f32 = MARGIN + 14.0;

const F_REG: Name<'static> = Name(b"F1");
const F_BOLD: Name<'static> = Name(b"F2");

/// Wandelt Unicode in WinAnsi (CP-1252); nicht darstellbare Zeichen werden `?`.
pub(crate) fn to_winansi(s: &str) -> Vec<u8> {
    s.chars()
        .map(|c| {
            let u = c as u32;
            match u {
                0x20..=0x7E | 0xA0..=0xFF => u as u8,
                0x09 | 0x0A | 0x0D => b' ',
                _ => match c {
                    '€' => 0x80,
                    '‚' => 0x82,
                    'ƒ' => 0x83,
                    '„' => 0x84,
                    '…' => 0x85,
                    '†' => 0x86,
                    '‡' => 0x87,
                    'ˆ' => 0x88,
                    '‰' => 0x89,
                    'Š' => 0x8A,
                    '‹' => 0x8B,
                    'Œ' => 0x8C,
                    'Ž' => 0x8E,
                    '‘' => 0x91,
                    '’' => 0x92,
                    '“' => 0x93,
                    '”' => 0x94,
                    '•' => 0x95,
                    '–' => 0x96,
                    '—' => 0x97,
                    '˜' => 0x98,
                    '™' => 0x99,
                    'š' => 0x9A,
                    '›' => 0x9B,
                    'œ' => 0x9C,
                    'ž' => 0x9E,
                    'Ÿ' => 0x9F,
                    '\u{202F}' | '\u{2009}' => b' ',
                    _ => b'?',
                },
            }
        })
        .collect()
}

/// Helvetica-Breiten (AFM, 1/1000 em) für 0x20..=0x7E.
const HELV_ASCII: [u16; 95] = [
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278, 278, // 0x20
    556, 556, 556, 556, 556, 556, 556, 556, 556, 556, 278, 278, 584, 584, 584, 556, // 0x30
    1015, 667, 667, 722, 722, 667, 611, 778, 722, 278, 500, 667, 556, 833, 722, 778, // 0x40
    667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611, 278, 278, 278, 469, 556, // 0x50
    333, 556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500, 222, 833, 556, 556, // 0x60
    556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500, 334, 260, 334, 584, // 0x70
];

fn glyph_width(b: u8) -> u16 {
    match b {
        0x20..=0x7E => HELV_ASCII[(b - 0x20) as usize],
        0x85 | 0x89 | 0x97 => 1000,
        0xC0..=0xC5 => 667, // À..Å
        0xC6 => 1000,
        0xC7 => 722,
        0xC8..=0xCB => 667,
        0xCC..=0xCF => 278,
        0xD1..=0xD6 | 0xD8 => 778,
        0xD9..=0xDC => 722,
        0xDF => 611, // ß
        0xE0..=0xE5 | 0xE8..=0xEB | 0xF1..=0xF6 | 0xF8..=0xFC => 556,
        0xE6 => 889,
        0xE7 => 500,
        0xEC..=0xEF => 278,
        0xB7 | 0x95 => 278, // · •
        _ => 556,
    }
}

fn text_width(bytes: &[u8], size: f32, bold: bool) -> f32 {
    let w: u32 = bytes.iter().map(|&b| glyph_width(b) as u32).sum();
    let f = if bold { 1.06 } else { 1.0 };
    w as f32 * size / 1000.0 * f
}

/// Kürzt auf `max` Punkt Breite und hängt "…" an.
fn fit(bytes: Vec<u8>, max: f32, size: f32, bold: bool) -> Vec<u8> {
    if text_width(&bytes, size, bold) <= max {
        return bytes;
    }
    let ell = text_width(&[0x85], size, bold);
    let mut out = Vec::with_capacity(bytes.len());
    let mut w = 0.0;
    for b in bytes {
        let gw = text_width(&[b], size, bold);
        if w + gw + ell > max {
            break;
        }
        w += gw;
        out.push(b);
    }
    out.push(0x85);
    out
}

fn show_at(c: &mut Content, font: Name, size: f32, x: f32, y: f32, text: &[u8]) {
    c.begin_text();
    c.set_font(font, size);
    c.next_line(x, y);
    c.show(Str(text));
    c.end_text();
}

/// Spaltenbreiten: natürliche Breite, bei Platzmangel werden breite Spalten gestaucht.
fn column_widths(headers: &[Vec<u8>], cells: &[Vec<Vec<u8>>], avail: f32) -> Vec<f32> {
    let n = headers.len();
    let mut w: Vec<f32> = headers
        .iter()
        .map(|h| text_width(h, FONT_SIZE, true) + 2.0 * PAD)
        .collect();
    for row in cells {
        for (i, c) in row.iter().enumerate().take(n) {
            w[i] = w[i].max(text_width(c, FONT_SIZE, false) + 2.0 * PAD);
        }
    }
    for x in &mut w {
        *x = x.clamp(24.0, 420.0);
    }
    let total: f32 = w.iter().sum();
    if total > avail {
        // Schmale Spalten (unter dem fairen Anteil) behalten, Rest proportional verteilen.
        let fair = avail / n as f32;
        let small: f32 = w.iter().filter(|&&x| x <= fair).sum();
        let big: f32 = w.iter().filter(|&&x| x > fair).sum();
        let room = (avail - small).max(0.0);
        for x in &mut w {
            if *x > fair && big > 0.0 {
                *x = (*x * room / big).max(24.0);
            }
        }
        // Falls die Mindestbreiten immer noch nicht passen: alles gleichmäßig skalieren.
        let total: f32 = w.iter().sum();
        if total > avail {
            for x in &mut w {
                *x *= avail / total;
            }
        }
    }
    w
}

pub(crate) fn write(table: &Table, info: &DocInfo, out: &mut dyn Write) -> Result<(), ExportError> {
    let lang = info.lang;
    let avail_w = PAGE_W - 2.0 * MARGIN;
    let ncols = table.headers.len();

    let headers: Vec<Vec<u8>> = table.headers.iter().map(|h| to_winansi(h)).collect();
    let cells: Vec<Vec<Vec<u8>>> = table
        .rows
        .iter()
        .map(|r| {
            r.iter()
                .map(|c| to_winansi(&display_cell(c, lang)))
                .collect()
        })
        .collect();
    let numeric: Vec<bool> = (0..ncols)
        .map(|i| {
            table
                .rows
                .first()
                .and_then(|r| r.get(i))
                .is_some_and(Cell::is_numeric)
        })
        .collect();
    let widths = column_widths(&headers, &cells, avail_w);

    let rows_per_page = (((TABLE_TOP - ROW_H) - TABLE_BOTTOM) / ROW_H)
        .floor()
        .max(1.0) as usize;
    let page_count = table.rows.len().div_ceil(rows_per_page).max(1);

    let mut pdf = Pdf::new();
    let catalog_id = Ref::new(1);
    let pages_id = Ref::new(2);
    let font_reg = Ref::new(3);
    let font_bold = Ref::new(4);
    let info_id = Ref::new(5);
    let page_ids: Vec<Ref> = (0..page_count)
        .map(|i| Ref::new(6 + 2 * i as i32))
        .collect();
    let content_ids: Vec<Ref> = (0..page_count)
        .map(|i| Ref::new(7 + 2 * i as i32))
        .collect();

    pdf.catalog(catalog_id).pages(pages_id);
    pdf.pages(pages_id)
        .kids(page_ids.iter().copied())
        .count(page_count as i32);
    pdf.type1_font(font_reg)
        .base_font(Name(b"Helvetica"))
        .encoding_predefined(Name(b"WinAnsiEncoding"));
    pdf.type1_font(font_bold)
        .base_font(Name(b"Helvetica-Bold"))
        .encoding_predefined(Name(b"WinAnsiEncoding"));
    pdf.document_info(info_id)
        .title(TextStr(&info.title))
        .creator(TextStr("Joschys_Tree @ Joschy"))
        .producer(TextStr("Joschys_Tree @ Joschy (pdf-writer)"));

    let title = fit(to_winansi(&info.title), avail_w, TITLE_SIZE, true);
    let subtitle = fit(to_winansi(&info.subtitle), avail_w, SUB_SIZE, false);
    let footer_left = to_winansi(&info.footer);
    let page_word = tr(lang, Key::ExpPage);

    for p in 0..page_count {
        let mut c = Content::new();

        // Kopfzeile: Titel, Untertitel, Branding rechts.
        c.set_fill_rgb(0.1, 0.12, 0.16);
        show_at(
            &mut c,
            F_BOLD,
            TITLE_SIZE,
            MARGIN,
            PAGE_H - MARGIN - TITLE_SIZE,
            &title,
        );
        let brand = to_winansi("Joschys_Tree @ Joschy");
        let bw = text_width(&brand, SUB_SIZE, false);
        c.set_fill_rgb(0.4, 0.43, 0.5);
        show_at(
            &mut c,
            F_REG,
            SUB_SIZE,
            PAGE_W - MARGIN - bw,
            PAGE_H - MARGIN - TITLE_SIZE,
            &brand,
        );
        if !subtitle.is_empty() {
            show_at(
                &mut c,
                F_REG,
                SUB_SIZE,
                MARGIN,
                PAGE_H - MARGIN - TITLE_SIZE - 13.0,
                &subtitle,
            );
        }

        // Tabellenkopf mit Hintergrund.
        let mut y = TABLE_TOP;
        c.set_fill_rgb(0.87, 0.89, 0.93);
        c.rect(MARGIN, y - ROW_H, avail_w, ROW_H);
        c.fill_nonzero();
        c.set_fill_rgb(0.0, 0.0, 0.0);
        draw_row(&mut c, F_BOLD, true, &headers, &widths, &numeric, y);
        y -= ROW_H;

        // Datenzeilen dieser Seite.
        let start = p * rows_per_page;
        let end = (start + rows_per_page).min(cells.len());
        for (i, row) in cells[start..end].iter().enumerate() {
            if i % 2 == 1 {
                c.set_fill_rgb(0.96, 0.97, 0.98);
                c.rect(MARGIN, y - ROW_H, avail_w, ROW_H);
                c.fill_nonzero();
                c.set_fill_rgb(0.0, 0.0, 0.0);
            }
            draw_row(&mut c, F_REG, false, row, &widths, &numeric, y);
            y -= ROW_H;
        }

        // Linie unter der Tabelle und über dem Fuß.
        c.set_stroke_rgb(0.75, 0.78, 0.83);
        c.set_line_width(0.5);
        c.move_to(MARGIN, MARGIN + 10.0);
        c.line_to(PAGE_W - MARGIN, MARGIN + 10.0);
        c.stroke();

        // Fuß: links Erstellt-Vermerk, rechts "@ Joschy · Seite n/m".
        c.set_fill_rgb(0.4, 0.43, 0.5);
        // ASCII-Stücke getrennt vom "·" (das als Hex-String landet), damit
        // "@ Joschy" und "Seite n/m" im Stream lesbar bleiben.
        let parts: [Vec<u8>; 3] = [
            b"@ Joschy".to_vec(),
            to_winansi(" · "),
            to_winansi(&format!("{page_word} {}/{}", p + 1, page_count)),
        ];
        let rw: f32 = parts.iter().map(|t| text_width(t, FOOT_SIZE, false)).sum();
        let left = fit(footer_left.clone(), avail_w - rw - 12.0, FOOT_SIZE, false);
        show_at(&mut c, F_REG, FOOT_SIZE, MARGIN, MARGIN, &left);
        let mut rx = PAGE_W - MARGIN - rw;
        for t in &parts {
            show_at(&mut c, F_REG, FOOT_SIZE, rx, MARGIN, t);
            rx += text_width(t, FOOT_SIZE, false);
        }

        pdf.stream(content_ids[p], &c.finish());

        let mut page = pdf.page(page_ids[p]);
        page.media_box(Rect::new(0.0, 0.0, PAGE_W, PAGE_H));
        page.parent(pages_id);
        page.contents(content_ids[p]);
        let mut res = page.resources();
        let mut fonts = res.fonts();
        fonts.pair(F_REG, font_reg);
        fonts.pair(F_BOLD, font_bold);
        fonts.finish();
        res.finish();
        page.finish();
    }

    out.write_all(&pdf.finish())?;
    out.flush()?;
    Ok(())
}

fn draw_row(
    c: &mut Content,
    font: Name,
    bold: bool,
    row: &[Vec<u8>],
    widths: &[f32],
    numeric: &[bool],
    y_top: f32,
) {
    let baseline = y_top - ROW_H + 3.5;
    let mut x = MARGIN;
    for (i, w) in widths.iter().enumerate() {
        if let Some(text) = row.get(i) {
            let t = fit(text.clone(), w - 2.0 * PAD, FONT_SIZE, bold);
            let tx = if numeric.get(i).copied().unwrap_or(false) {
                x + w - PAD - text_width(&t, FONT_SIZE, bold)
            } else {
                x + PAD
            };
            show_at(c, font, FONT_SIZE, tx, baseline, &t);
        }
        x += w;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn winansi_umlauts() {
        assert_eq!(to_winansi("Größe"), vec![b'G', b'r', 0xF6, 0xDF, b'e']);
        assert_eq!(to_winansi("€ …"), vec![0x80, b' ', 0x85]);
        assert_eq!(to_winansi("≥"), b"?".to_vec());
    }

    #[test]
    fn fit_truncates() {
        let long = to_winansi(&"x".repeat(200));
        let t = fit(long, 50.0, FONT_SIZE, false);
        assert!(text_width(&t, FONT_SIZE, false) <= 50.0);
        assert_eq!(*t.last().unwrap(), 0x85);
    }
}
