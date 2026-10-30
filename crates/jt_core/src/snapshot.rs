// Joschys_Tree – Scan speichern/laden (.jtscan = Header + lz4(postcard)) – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fmt;
use std::io::Write;
use std::path::Path;

use crate::model::{NO_NODE, NodeId, Tree};

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

const HEADER_LEN: usize = 8 + 2;

/// Magic "JTSCAN01", Version-u16 (little endian), lz4(postcard(Tree)) mit vorangestellter Länge.
/// Schreibt erst in eine temporäre Datei und benennt dann um (kein halber Snapshot bei Absturz).
pub fn save(tree: &Tree, path: &Path) -> Result<(), SnapError> {
    let bytes = to_bytes(tree)?;
    let tmp = path.with_extension("jtscan.tmp");
    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(&bytes)?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })?;
    Ok(())
}

pub fn load(path: &Path) -> Result<Tree, SnapError> {
    let data = std::fs::read(path)?;
    from_bytes(&data)
}

/// Snapshot als Bytes (gleiches Format wie die Datei).
pub fn to_bytes(tree: &Tree) -> Result<Vec<u8>, SnapError> {
    let raw = postcard::to_stdvec(tree).map_err(|e| SnapError::Encode(e.to_string()))?;
    let packed = lz4_flex::compress_prepend_size(&raw);
    let mut out = Vec::with_capacity(HEADER_LEN + packed.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.extend_from_slice(&packed);
    Ok(out)
}

/// Gegenstück zu [`to_bytes`]. Prüft Magic, Version und die Baumstruktur.
pub fn from_bytes(data: &[u8]) -> Result<Tree, SnapError> {
    if data.len() < HEADER_LEN || &data[..8] != MAGIC {
        return Err(SnapError::BadMagic);
    }
    let ver = u16::from_le_bytes([data[8], data[9]]);
    if ver != VERSION {
        return Err(SnapError::UnsupportedVersion(ver));
    }
    let raw = lz4_flex::decompress_size_prepended(&data[HEADER_LEN..])
        .map_err(|e| SnapError::Decode(e.to_string()))?;
    let tree: Tree = postcard::from_bytes(&raw).map_err(|e| SnapError::Decode(e.to_string()))?;
    validate(&tree)?;
    Ok(tree)
}

/// Verweise müssen im Bereich liegen, sonst würde ein manipulierter Snapshot später paniken.
fn validate(tree: &Tree) -> Result<(), SnapError> {
    let n = tree.nodes.len();
    if n == 0 || n > u32::MAX as usize {
        return Err(SnapError::Decode("leerer Baum".into()));
    }
    if tree.extensions.is_empty() {
        return Err(SnapError::Decode("Endungstabelle fehlt".into()));
    }
    let ok = |id: NodeId| id == NO_NODE || (id as usize) < n;
    for node in &tree.nodes {
        if !ok(node.parent)
            || !ok(node.first_child)
            || !ok(node.next_sibling)
            || node.ext_id as usize >= tree.extensions.len()
        {
            return Err(SnapError::Decode("ungültiger Knotenverweis".into()));
        }
    }
    if tree.nodes[0].parent != NO_NODE {
        return Err(SnapError::Decode("Wurzel hat Elternteil".into()));
    }
    Ok(())
}
