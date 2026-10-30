// Joschys_Tree – HTML-Export mit Fuß – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Eine einzelne HTML-Datei mit inline CSS, ohne externe Ressourcen.
//! Alle Texte werden HTML-escaped. Fuß: "Erstellt mit Joschys_Tree @ Joschy · <Datum>".

use std::io::Write;

use super::{DocInfo, ExportError, Table, display_cell};

/// Escaped `& < > " '` für Text und Attributwerte.
pub(crate) fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

const CSS: &str = "\
:root{color-scheme:light dark;--bg:#ffffff;--fg:#1d2433;--muted:#5d6678;--line:#d9dee7;--head:#eef1f6;--zebra:#f7f8fb;--accent:#2f6fde}\
@media (prefers-color-scheme:dark){:root{--bg:#16191f;--fg:#e4e7ee;--muted:#9aa3b2;--line:#2c323d;--head:#20252e;--zebra:#1b1f27;--accent:#6ea0ff}}\
*{box-sizing:border-box}\
body{margin:0;padding:24px;background:var(--bg);color:var(--fg);font:14px/1.45 'Segoe UI',system-ui,-apple-system,Helvetica,Arial,sans-serif}\
h1{margin:0 0 4px;font-size:22px}\
.sub{margin:0 0 16px;color:var(--muted)}\
.wrap{overflow-x:auto;border:1px solid var(--line);border-radius:6px}\
table{border-collapse:collapse;width:100%}\
th,td{padding:5px 10px;border-bottom:1px solid var(--line);text-align:left;white-space:nowrap}\
th{position:sticky;top:0;background:var(--head);font-weight:600}\
tbody tr:nth-child(even){background:var(--zebra)}\
td.num,th.num{text-align:right;font-variant-numeric:tabular-nums}\
footer{margin-top:16px;padding-top:8px;border-top:2px solid var(--accent);color:var(--muted);font-size:12px}\
@media print{body{padding:0}th{position:static}}";

pub(crate) fn write(table: &Table, info: &DocInfo, out: &mut dyn Write) -> Result<(), ExportError> {
    // Spalte gilt als numerisch, wenn die erste Zeile dort eine Zahl hat.
    let numeric: Vec<bool> = (0..table.headers.len())
        .map(|i| {
            table
                .rows
                .first()
                .and_then(|r| r.get(i))
                .is_some_and(|c| c.is_numeric())
        })
        .collect();

    let mut s = String::with_capacity(1024 + table.rows.len() * 160);
    s.push_str("<!DOCTYPE html>\n<!-- Joschys_Tree @ Joschy -->\n");
    s.push_str(&format!(
        "<html lang=\"{}\">\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <meta name=\"generator\" content=\"Joschys_Tree @ Joschy\">\n<title>{}</title>\n\
         <style>{CSS}</style>\n</head>\n<body>\n",
        info.lang.code(),
        escape(&info.title)
    ));
    s.push_str(&format!("<h1>{}</h1>\n", escape(&info.title)));
    if !info.subtitle.is_empty() {
        s.push_str(&format!(
            "<p class=\"sub\">{}</p>\n",
            escape(&info.subtitle)
        ));
    }
    s.push_str("<div class=\"wrap\"><table>\n<thead><tr>");
    for (i, h) in table.headers.iter().enumerate() {
        let cls = if numeric[i] { " class=\"num\"" } else { "" };
        s.push_str(&format!("<th{cls}>{}</th>", escape(h)));
    }
    s.push_str("</tr></thead>\n<tbody>\n");
    for row in &table.rows {
        s.push_str("<tr>");
        for c in row {
            let cls = if c.is_numeric() { " class=\"num\"" } else { "" };
            s.push_str(&format!(
                "<td{cls}>{}</td>",
                escape(&display_cell(c, info.lang))
            ));
        }
        s.push_str("</tr>\n");
    }
    s.push_str("</tbody>\n</table></div>\n");
    s.push_str(&format!(
        "<footer>{}</footer>\n</body>\n</html>\n",
        escape(&info.footer)
    ));
    out.write_all(s.as_bytes())?;
    out.flush()?;
    Ok(())
}
