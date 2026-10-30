// Joschys_Tree – Scan speichern/laden (.jtscan = Header + lz4(postcard)) – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fmt;
use std::path::Path;

use crate::model::Tree;

pub const MAGIC: &[u8; 8] = b"JTSCAN01";
pub const VERSION: u16 = 1;

#[derive(Debug)]
pub enum SnapError {
    Io(std::io::Error),
    BadMagic,
    UnsupportedVersion(u16),
    Decode(String),
    Encode(String),
}

impl fmt::Display for SnapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SnapError::Io(e) => write!(f, "E/A-Fehler: {e}"),
            SnapError::BadMagic => write!(f, "keine Joschys_Tree-Snapshot-Datei"),
            SnapError::UnsupportedVersion(v) => write!(f, "Snapshot-Version {v} nicht unterstützt"),
            SnapError::Decode(e) => write!(f, "Snapshot beschädigt: {e}"),
            SnapError::Encode(e) => write!(f, "Snapshot konnte nicht erzeugt werden: {e}"),
        }
    }
}

impl std::error::Error for SnapError {}

impl From<std::io::Error> for SnapError {
    fn from(e: std::io::Error) -> Self {
        SnapError::Io(e)
    }
}

/// Magic "JTSCAN01", Version-u16, lz4(postcard(Tree)).
pub fn save(tree: &Tree, path: &Path) -> Result<(), SnapError> {
    let _ = (tree, path);
    todo!("Auftrag A: snapshot::save")
}

pub fn load(path: &Path) -> Result<Tree, SnapError> {
    let _ = path;
    todo!("Auftrag A: snapshot::load")
}
