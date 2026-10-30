// Joschys_Tree – Plattformneutraler Parser für NTFS-MFT-Records – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fmt;

use crate::model::{FileTime, NodeFlags, ScanMeta, Tree};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MftError {
    TooShort,
    BadSignature,
    FixupMismatch,
    BadAttribute(String),
    BadRunlist,
}

impl fmt::Display for MftError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MftError::TooShort => write!(f, "Record zu kurz"),
            MftError::BadSignature => write!(f, "ungültige Record-Signatur"),
            MftError::FixupMismatch => write!(f, "Fixup-Prüfung fehlgeschlagen"),
            MftError::BadAttribute(s) => write!(f, "ungültiges Attribut: {s}"),
            MftError::BadRunlist => write!(f, "ungültige Runlist"),
        }
    }
}

impl std::error::Error for MftError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MftRecord {
    pub record_no: u64,
    pub parent_ref: u64,
    pub name: String,
    pub is_dir: bool,
    pub in_use: bool,
    pub size: u64,
    pub allocated: u64,
    /// created, modified, accessed
    pub times: [FileTime; 3],
    pub flags: NodeFlags,
    pub ads_size: u64,
    pub base_record: u64,
}

pub fn apply_fixup(buf: &mut [u8], bytes_per_sector: usize) -> Result<(), MftError> {
    let _ = (buf, bytes_per_sector);
    todo!("Auftrag B: apply_fixup")
}

pub fn parse_record(buf: &[u8], record_no: u64) -> Result<Option<MftRecord>, MftError> {
    let _ = (buf, record_no);
    todo!("Auftrag B: parse_record")
}

/// Liefert (lcn, len)-Paare.
pub fn parse_runlist(data: &[u8]) -> Result<Vec<(u64, u64)>, MftError> {
    let _ = data;
    todo!("Auftrag B: parse_runlist")
}

pub fn records_to_tree(
    records: impl Iterator<Item = MftRecord>,
    meta: ScanMeta,
    root_name: &str,
) -> Tree {
    let _ = (records.count(), meta, root_name);
    todo!("Auftrag B: records_to_tree")
}
