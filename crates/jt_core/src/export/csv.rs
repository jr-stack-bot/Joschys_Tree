// Joschys_Tree – CSV-Export (UTF-8 mit BOM) – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

//! CSV nach RFC 4180, UTF-8 mit BOM (damit Excel Umlaute erkennt).
//! Trennzeichen `;` bei Deutsch, `,` bei Englisch. Zeilenende CRLF.
//! Bytes und Zähler als reine Zahlen (rechenbar), Prozent mit Sprach-Dezimalzeichen.

use std::io::Write;

use super::{Cell, ExportError, Table, fmt_decimal, fmt_time};
use crate::i18n::Lang;

pub(crate) const BOM: &[u8] = b"\xEF\xBB\xBF";

pub(crate) fn delimiter(lang: Lang) -> char {
    match lang {
        Lang::De => ';',
        Lang::En => ',',
    }
}

/// Setzt ein Feld in Anführungszeichen, wenn es Trennzeichen, `"`, CR oder LF enthält.
pub(crate) fn quote(field: &str, delim: char) -> String {
    if field.contains([delim, '"', '\r', '\n']) {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

fn cell_text(c: &Cell, lang: Lang) -> String {
    match c {
        Cell::Text(s) => s.clone(),
        Cell::Bytes(b) => b.to_string(),
        Cell::Int(i) => i.to_string(),
        Cell::Percent(p) => fmt_decimal(*p as f64, 2, lang),
        Cell::Time(t) => fmt_time(*t, lang),
    }
}

fn write_line(
    out: &mut dyn Write,
    fields: impl Iterator<Item = String>,
    delim: char,
) -> std::io::Result<()> {
    let mut line = String::new();
    for (i, f) in fields.enumerate() {
        if i > 0 {
            line.push(delim);
        }
        line.push_str(&quote(&f, delim));
    }
    line.push_str("\r\n");
    out.write_all(line.as_bytes())
}

pub(crate) fn write(table: &Table, lang: Lang, out: &mut dyn Write) -> Result<(), ExportError> {
    let delim = delimiter(lang);
    out.write_all(BOM)?;
    write_line(out, table.headers.iter().cloned(), delim)?;
    for row in &table.rows {
        write_line(out, row.iter().map(|c| cell_text(c, lang)), delim)?;
    }
    out.flush()?;
    Ok(())
}
