// Joschys_Tree – Export: Formate, Anfrage, gemeinsame Tabellenerzeugung – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fmt;
use std::io::Write;

use crate::delta::DeltaEntry;
use crate::duplicates::DupGroup;
use crate::i18n::Lang;
use crate::model::{FileTime, NodeId, Tree};
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
#[allow(dead_code)]
pub(crate) struct Table {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<Cell>>,
}

#[allow(dead_code)]
pub(crate) enum Cell {
    Text(String),
    Bytes(u64),
    Int(i64),
    Percent(f32),
    Time(FileTime),
}

pub fn export(
    req: &ExportRequest,
    fmt: ExportFormat,
    out: &mut dyn Write,
) -> Result<(), ExportError> {
    let _ = (req, fmt, out);
    todo!("Auftrag D: export")
}
