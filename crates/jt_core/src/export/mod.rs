// Joschys_Tree – Export: Formate, Anfrage, gemeinsame Tabellenerzeugung – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Alle vier Formate (CSV, XLSX, HTML, PDF) arbeiten auf derselben [`Table`].
//! Diese wird hier aus dem [`ExportContent`] erzeugt.

use std::fmt;
use std::io::Write;

use chrono::{DateTime, Local, NaiveDateTime};

use crate::delta::{ChangeKind, DeltaEntry};
use crate::duplicates::DupGroup;
use crate::i18n::{Key, Lang, tr};
use crate::model::{FileTime, NO_NODE, NodeId, Tree};
use crate::stats::TypeStat;

pub mod csv;
pub mod html;
pub mod pdf;
pub mod xlsx;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ExportFormat {
    Csv,
    Xlsx,
    Html,
    Pdf,
}

impl ExportFormat {
    /// Dateiendung ohne Punkt.
    pub fn extension(self) -> &'static str {
        match self {
            ExportFormat::Csv => "csv",
            ExportFormat::Xlsx => "xlsx",
            ExportFormat::Html => "html",
            ExportFormat::Pdf => "pdf",
        }
    }
}

#[derive(Clone, Debug)]
pub enum ExportContent {
    Tree { root: NodeId, max_depth: u32 },
    TopFiles(Vec<NodeId>),
    FileTypes(Vec<TypeStat>),
    Duplicates(Vec<DupGroup>),
    Delta(Vec<DeltaEntry>),
}

pub struct ExportRequest<'a> {
    pub tree: Option<&'a Tree>,
    pub content: ExportContent,
    pub lang: Lang,
    pub title: String,
}

#[derive(Debug)]
pub enum ExportError {
    Io(std::io::Error),
    MissingTree,
    Format(String),
}

impl fmt::Display for ExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExportError::Io(e) => write!(f, "E/A-Fehler: {e}"),
            ExportError::MissingTree => write!(f, "kein Scan vorhanden"),
            ExportError::Format(e) => write!(f, "Exportfehler: {e}"),
        }
    }
}

impl std::error::Error for ExportError {}

impl From<std::io::Error> for ExportError {
    fn from(e: std::io::Error) -> Self {
        ExportError::Io(e)
    }
}

/// Gemeinsam für alle Formate.
pub(crate) struct Table {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<Cell>>,
}

pub(crate) enum Cell {
    Text(String),
    Bytes(u64),
    Int(i64),
    Percent(f32),
    Time(FileTime),
}

impl Cell {
    /// Zahlen-Spalten werden rechtsbündig gesetzt.
    pub(crate) fn is_numeric(&self) -> bool {
        !matches!(self, Cell::Text(_))
    }
}

/// Kopfdaten für Titelzeilen und Fuß (HTML/PDF/XLSX).
pub(crate) struct DocInfo {
    pub title: String,
    pub lang: Lang,
    /// z. B. "Ziel: C:\ · Scan vom 30.10.2026 12:00"
    pub subtitle: String,
    /// "Erstellt mit Joschys_Tree @ Joschy · <Datum>"
    pub footer: String,
}

pub fn export(
    req: &ExportRequest,
    fmt: ExportFormat,
    out: &mut dyn Write,
) -> Result<(), ExportError> {
    let table = build_table(req)?;
    let info = doc_info(req);
    match fmt {
        ExportFormat::Csv => csv::write(&table, req.lang, out),
        ExportFormat::Xlsx => xlsx::write(&table, &info, out),
        ExportFormat::Html => html::write(&table, &info, out),
        ExportFormat::Pdf => pdf::write(&table, &info, out),
    }
}

fn doc_info(req: &ExportRequest) -> DocInfo {
    let lang = req.lang;
    let title = if req.title.trim().is_empty() {
        tr(lang, default_title_key(&req.content)).to_string()
    } else {
        req.title.clone()
    };
    let mut parts = Vec::new();
    if let Some(t) = req.tree {
        parts.push(format!(
            "{}: {}",
            tr(lang, Key::ExpScanRoot),
            t.meta.root_path
        ));
        if t.meta.started.0 != 0 {
            parts.push(format!(
                "{} {}",
                tr(lang, Key::ExpScanDate),
                fmt_time(t.meta.started, lang)
            ));
        }
    }
    DocInfo {
        title,
        lang,
        subtitle: parts.join(" · "),
        footer: format!(
            "{} · {}",
            tr(lang, Key::ExpCreatedWith),
            fmt_datetime(&Local::now().naive_local(), lang)
        ),
    }
}

fn default_title_key(c: &ExportContent) -> Key {
    match c {
        ExportContent::Tree { .. } => Key::ExpTitleTree,
        ExportContent::TopFiles(_) => Key::ExpTitleTopFiles,
        ExportContent::FileTypes(_) => Key::ExpTitleFileTypes,
        ExportContent::Duplicates(_) => Key::ExpTitleDuplicates,
        ExportContent::Delta(_) => Key::ExpTitleDelta,
    }
}

fn headers(lang: Lang, keys: &[Key]) -> Vec<String> {
    keys.iter().map(|&k| tr(lang, k).to_string()).collect()
}

fn type_text(lang: Lang, is_dir: bool) -> String {
    tr(
        lang,
        if is_dir {
            Key::TypeFolder
        } else {
            Key::TypeFile
        },
    )
    .to_string()
}

fn percent(part: u64, whole: u64) -> f32 {
    if whole == 0 {
        0.0
    } else {
        (part as f64 * 100.0 / whole as f64) as f32
    }
}

pub(crate) fn build_table(req: &ExportRequest) -> Result<Table, ExportError> {
    let lang = req.lang;
    let need_tree = || req.tree.ok_or(ExportError::MissingTree);
    let table = match &req.content {
        ExportContent::Tree { root, max_depth } => {
            let tree = need_tree()?;
            check_id(tree, *root)?;
            let mut rows = Vec::new();
            // Tiefensuche, Kinder nach Größe absteigend.
            let mut stack: Vec<(NodeId, u32)> = vec![(*root, 0)];
            while let Some((id, depth)) = stack.pop() {
                let n = tree.node(id);
                let parent_size = if n.parent == NO_NODE || id == *root {
                    n.size
                } else {
                    tree.node(n.parent).size
                };
                rows.push(vec![
                    Cell::Text(tree.full_path(id)),
                    Cell::Text(n.name.to_string()),
                    Cell::Text(type_text(lang, n.is_dir())),
                    Cell::Bytes(n.size),
                    Cell::Bytes(n.allocated),
                    Cell::Percent(percent(n.size, parent_size)),
                    Cell::Int(n.file_count as i64),
                    Cell::Int(n.dir_count as i64),
                    Cell::Time(n.modified),
                ]);
                if depth < *max_depth {
                    let mut kids: Vec<NodeId> = tree.children(id).collect();
                    kids.sort_by(|&a, &b| {
                        tree.node(b)
                            .size
                            .cmp(&tree.node(a).size)
                            .then_with(|| tree.node(a).name.cmp(&tree.node(b).name))
                    });
                    for k in kids.into_iter().rev() {
                        stack.push((k, depth + 1));
                    }
                }
            }
            Table {
                headers: headers(
                    lang,
                    &[
                        Key::ColPath,
                        Key::ColName,
                        Key::ColType,
                        Key::ColSize,
                        Key::ColAllocated,
                        Key::ColPercentOfParent,
                        Key::ColFiles,
                        Key::ColFolders,
                        Key::ColModified,
                    ],
                ),
                rows,
            }
        }
        ExportContent::TopFiles(ids) => {
            let tree = need_tree()?;
            let mut rows = Vec::with_capacity(ids.len());
            for (i, &id) in ids.iter().enumerate() {
                check_id(tree, id)?;
                let n = tree.node(id);
                rows.push(vec![
                    Cell::Int(i as i64 + 1),
                    Cell::Text(n.name.to_string()),
                    Cell::Text(tree.full_path(id)),
                    Cell::Text(tree.ext_of(id).to_string()),
                    Cell::Bytes(n.size),
                    Cell::Bytes(n.allocated),
                    Cell::Time(n.modified),
                ]);
            }
            Table {
                headers: headers(
                    lang,
                    &[
                        Key::ColRank,
                        Key::ColName,
                        Key::ColPath,
                        Key::ColExtension,
                        Key::ColSize,
                        Key::ColAllocated,
                        Key::ColModified,
                    ],
                ),
                rows,
            }
        }
        ExportContent::FileTypes(stats) => {
            let total: u64 = stats.iter().map(|s| s.size).sum();
            let rows = stats
                .iter()
                .map(|s| {
                    let ext = if s.ext.is_empty() {
                        tr(lang, Key::NoExtension).to_string()
                    } else {
                        s.ext.clone()
                    };
                    vec![
                        Cell::Text(ext),
                        Cell::Int(s.files as i64),
                        Cell::Bytes(s.size),
                        Cell::Bytes(s.allocated),
                        Cell::Percent(percent(s.size, total)),
                    ]
                })
                .collect();
            Table {
                headers: headers(
                    lang,
                    &[
                        Key::ColExtension,
                        Key::ColFiles,
                        Key::ColSize,
                        Key::ColAllocated,
                        Key::ColPercent,
                    ],
                ),
                rows,
            }
        }
        ExportContent::Duplicates(groups) => {
            let tree = need_tree()?;
            let mut rows = Vec::new();
            for (gi, g) in groups.iter().enumerate() {
                let savings = g
                    .size
                    .saturating_mul(g.files.len().saturating_sub(1) as u64);
                for &id in &g.files {
                    check_id(tree, id)?;
                    rows.push(vec![
                        Cell::Int(gi as i64 + 1),
                        Cell::Int(g.files.len() as i64),
                        Cell::Bytes(g.size),
                        Cell::Bytes(savings),
                        Cell::Text(format!("{:032x}", g.hash)),
                        Cell::Text(tree.full_path(id)),
                        Cell::Time(tree.node(id).modified),
                    ]);
                }
            }
            Table {
                headers: headers(
                    lang,
                    &[
                        Key::ColGroup,
                        Key::ColCount,
                        Key::ColSize,
                        Key::ColSavings,
                        Key::ColHash,
                        Key::ColPath,
                        Key::ColModified,
                    ],
                ),
                rows,
            }
        }
        ExportContent::Delta(entries) => {
            let rows = entries
                .iter()
                .map(|e| {
                    let kind = match e.kind {
                        ChangeKind::Added => Key::DeltaAdded,
                        ChangeKind::Removed => Key::DeltaRemoved,
                        ChangeKind::Grown => Key::DeltaGrown,
                        ChangeKind::Shrunk => Key::DeltaShrunk,
                    };
                    let diff = e.new_size as i128 - e.old_size as i128;
                    vec![
                        Cell::Text(tr(lang, kind).to_string()),
                        Cell::Text(e.path.clone()),
                        Cell::Text(type_text(lang, e.is_dir)),
                        Cell::Bytes(e.old_size),
                        Cell::Bytes(e.new_size),
                        Cell::Int(diff.clamp(i64::MIN as i128, i64::MAX as i128) as i64),
                    ]
                })
                .collect();
            Table {
                headers: headers(
                    lang,
                    &[
                        Key::ColChange,
                        Key::ColPath,
                        Key::ColType,
                        Key::ColOldSize,
                        Key::ColNewSize,
                        Key::ColDifference,
                    ],
                ),
                rows,
            }
        }
    };
    Ok(table)
}

fn check_id(tree: &Tree, id: NodeId) -> Result<(), ExportError> {
    if (id as usize) < tree.nodes.len() {
        Ok(())
    } else {
        Err(ExportError::Format(format!(
            "NodeId {id} außerhalb des Baums"
        )))
    }
}

// ---------------------------------------------------------------------------
// Eigene kleine Formatierhilfen (unabhängig von format.rs, damit der Export
// für sich testbar bleibt).
// ---------------------------------------------------------------------------

fn seps(lang: Lang) -> (char, char) {
    // (Tausender, Dezimal)
    match lang {
        Lang::De => ('.', ','),
        Lang::En => (',', '.'),
    }
}

/// Ganze Zahl mit Tausendertrennern: 1234567 -> "1.234.567" (de).
pub(crate) fn fmt_int(n: i64, lang: Lang) -> String {
    let (th, _) = seps(lang);
    let digits = n.unsigned_abs().to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3 + 1);
    if n < 0 {
        out.push('-');
    }
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(th);
        }
        out.push(c);
    }
    out
}

/// Dezimalzahl mit fester Nachkommastellenzahl und Sprach-Dezimalzeichen.
pub(crate) fn fmt_decimal(v: f64, decimals: usize, lang: Lang) -> String {
    let (_, dec) = seps(lang);
    let s = format!("{v:.decimals$}");
    if dec == '.' { s } else { s.replace('.', ",") }
}

/// Bytes -> "1,5 KiB" (de) / "1.5 KiB" (en). Unter 1 KiB: "123 B".
pub(crate) fn fmt_bytes(n: u64, lang: Lang) -> String {
    const UNITS: [&str; 7] = ["B", "KiB", "MiB", "GiB", "TiB", "PiB", "EiB"];
    if n < 1024 {
        return format!("{n} B");
    }
    let mut v = n as f64;
    let mut u = 0;
    while v >= 1024.0 && u < UNITS.len() - 1 {
        v /= 1024.0;
        u += 1;
    }
    let mut s = format!("{v:.2}");
    while s.ends_with('0') {
        s.pop();
    }
    if s.ends_with('.') {
        s.pop();
    }
    if lang == Lang::De {
        s = s.replace('.', ",");
    }
    format!("{s} {}", UNITS[u])
}

/// Prozent mit einer Nachkommastelle: "12,5 %" (de) / "12.5%" (en).
pub(crate) fn fmt_percent(p: f32, lang: Lang) -> String {
    let v = fmt_decimal(p as f64, 1, lang);
    match lang {
        Lang::De => format!("{v} %"),
        Lang::En => format!("{v}%"),
    }
}

/// FileTime -> lokale Zeit; None bei 0 (unbekannt) oder ungültig.
pub(crate) fn local_time(t: FileTime) -> Option<NaiveDateTime> {
    if t.0 == 0 {
        return None;
    }
    DateTime::from_timestamp(t.to_unix_secs(), 0).map(|d| d.with_timezone(&Local).naive_local())
}

pub(crate) fn fmt_datetime(d: &NaiveDateTime, lang: Lang) -> String {
    match lang {
        Lang::De => d.format("%d.%m.%Y %H:%M").to_string(),
        Lang::En => d.format("%Y-%m-%d %H:%M").to_string(),
    }
}

/// Datum für HTML/PDF; leer bei unbekannt.
pub(crate) fn fmt_time(t: FileTime, lang: Lang) -> String {
    local_time(t)
        .map(|d| fmt_datetime(&d, lang))
        .unwrap_or_default()
}

/// Lesbarer Zelltext für HTML und PDF.
pub(crate) fn display_cell(c: &Cell, lang: Lang) -> String {
    match c {
        Cell::Text(s) => s.clone(),
        Cell::Bytes(b) => fmt_bytes(*b, lang),
        Cell::Int(i) => fmt_int(*i, lang),
        Cell::Percent(p) => fmt_percent(*p, lang),
        Cell::Time(t) => fmt_time(*t, lang),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helpers() {
        assert_eq!(fmt_bytes(1536, Lang::De), "1,5 KiB");
        assert_eq!(fmt_bytes(1536, Lang::En), "1.5 KiB");
        assert_eq!(fmt_bytes(100, Lang::De), "100 B");
        assert_eq!(fmt_int(1_234_567, Lang::De), "1.234.567");
        assert_eq!(fmt_int(-1234, Lang::En), "-1,234");
        assert_eq!(fmt_int(999, Lang::En), "999");
        assert_eq!(fmt_percent(12.5, Lang::En), "12.5%");
        assert_eq!(fmt_time(FileTime(0), Lang::De), "");
    }
}
