// Joschys_Tree – Excel-Export via rust_xlsxwriter – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

//! XLSX mit Spaltenformaten (Bytes als Zahl, Datum als Datum, Prozent als Prozent),
//! AutoFilter über die Tabelle und fixierter Kopfzeile. Kopf-/Fußzeile beim Druck "@ Joschy".

use std::io::Write;

use chrono::{Datelike, Timelike};
use rust_xlsxwriter::{DocProperties, ExcelDateTime, Format, FormatAlign, Workbook, XlsxError};

use super::{Cell, DocInfo, ExportError, Table, local_time};
use crate::i18n::Lang;

impl From<XlsxError> for ExportError {
    fn from(e: XlsxError) -> Self {
        ExportError::Format(e.to_string())
    }
}

/// Maximale Zeilen pro Blatt in Excel (inkl. Kopf).
const MAX_ROWS: usize = 1_048_576;

pub(crate) fn write(table: &Table, info: &DocInfo, out: &mut dyn Write) -> Result<(), ExportError> {
    let mut wb = Workbook::new();
    wb.set_properties(
        &DocProperties::new()
            .set_title(&info.title)
            .set_author("Joschys_Tree @ Joschy")
            .set_comment(&info.footer),
    );

    let head = Format::new().set_bold().set_background_color(0xDDE3EE);
    let bytes_fmt = Format::new().set_num_format("#,##0");
    let int_fmt = Format::new().set_num_format("#,##0");
    let pct_fmt = Format::new().set_num_format("0.0%");
    let date_fmt = Format::new()
        .set_num_format(match info.lang {
            Lang::De => "dd.mm.yyyy hh:mm",
            Lang::En => "yyyy-mm-dd hh:mm",
        })
        .set_align(FormatAlign::Left);

    let ws = wb.add_worksheet();
    // Blattnamen dürfen bestimmte Zeichen nicht enthalten und max. 31 Zeichen lang sein.
    let sheet_name: String = info
        .title
        .chars()
        .filter(|c| !matches!(c, '[' | ']' | ':' | '*' | '?' | '/' | '\\'))
        .take(31)
        .collect();
    if !sheet_name.trim().is_empty() && !sheet_name.starts_with('\'') {
        let _ = ws.set_name(sheet_name.trim());
    }
    ws.set_landscape();
    ws.set_header(format!("&L&\"-,Bold\"{}&R@ Joschy", xl_hf(&info.title)));
    ws.set_footer(format!("&L{}&R@ Joschy · &P/&N", xl_hf(&info.footer)));

    let cols = table.headers.len();
    let mut widths: Vec<usize> = table.headers.iter().map(|h| h.chars().count()).collect();

    for (c, h) in table.headers.iter().enumerate() {
        ws.write_string_with_format(0, c as u16, h, &head)?;
    }
    let rows = table.rows.len().min(MAX_ROWS - 1);
    for (r, row) in table.rows.iter().take(rows).enumerate() {
        let r = (r + 1) as u32;
        for (c, cell) in row.iter().enumerate() {
            let col = c as u16;
            let w = match cell {
                Cell::Text(s) => {
                    ws.write_string(r, col, s)?;
                    s.chars().count()
                }
                Cell::Bytes(b) => {
                    ws.write_number_with_format(r, col, *b as f64, &bytes_fmt)?;
                    14
                }
                Cell::Int(i) => {
                    ws.write_number_with_format(r, col, *i as f64, &int_fmt)?;
                    8
                }
                Cell::Percent(p) => {
                    ws.write_number_with_format(r, col, *p as f64 / 100.0, &pct_fmt)?;
                    7
                }
                Cell::Time(t) => {
                    if let Some(d) = local_time(*t).and_then(|d| {
                        ExcelDateTime::from_ymd(d.year() as u16, d.month() as u8, d.day() as u8)
                            .and_then(|x| {
                                x.and_hms(d.hour() as u16, d.minute() as u8, d.second() as f64)
                            })
                            .ok()
                    }) {
                        ws.write_datetime_with_format(r, col, &d, &date_fmt)?;
                    }
                    16
                }
            };
            if c < widths.len() {
                widths[c] = widths[c].max(w);
            }
        }
    }
    for (c, w) in widths.iter().enumerate() {
        ws.set_column_width(c as u16, (*w as f64 + 2.0).min(80.0))?;
    }
    if cols > 0 {
        ws.autofilter(0, 0, rows as u32, (cols - 1) as u16)?;
    }
    ws.set_freeze_panes(1, 0)?;

    let buf = wb.save_to_buffer()?;
    out.write_all(&buf)?;
    out.flush()?;
    Ok(())
}

/// '&' ist in Excel-Kopf-/Fußzeilen ein Steuerzeichen.
fn xl_hf(s: &str) -> String {
    s.replace('&', "&&")
}
