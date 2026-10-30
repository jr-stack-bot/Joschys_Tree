// Joschys_Tree – Scanner: ScanRequest, ScanProgress, Auswahl MFT/Walk mit Fallback – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fmt;
use std::sync::atomic::AtomicBool;

use crate::model::Tree;

pub mod mft_parse;
#[cfg(windows)]
pub mod mft_win;
pub mod walk;

#[cfg(windows)]
pub(crate) use mft_win::scan_mft;
pub(crate) use walk::scan_walk;

#[derive(Clone, Debug)]
pub struct ScanRequest {
    pub root: String,
    pub prefer_mft: bool,
    /// immer false in UI
    pub follow_reparse: bool,
}

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub enum Phase {
    #[default]
    Starting,
    ReadingMft,
    Walking,
    Building,
    Aggregating,
    Done,
}

#[derive(Clone, Default, Debug)]
pub struct ScanProgress {
    pub entries: u64,
    pub bytes: u64,
    pub current: String,
    pub phase: Phase,
}

#[derive(Debug)]
pub enum ScanError {
    Io(std::io::Error),
    Cancelled,
    NotFound(String),
    Mft(mft_parse::MftError),
    Unsupported(String),
}

impl fmt::Display for ScanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScanError::Io(e) => write!(f, "E/A-Fehler: {e}"),
            ScanError::Cancelled => write!(f, "Scan abgebrochen"),
            ScanError::NotFound(p) => write!(f, "Ziel nicht gefunden: {p}"),
            ScanError::Mft(e) => write!(f, "MFT-Fehler: {e}"),
            ScanError::Unsupported(s) => write!(f, "nicht unterstützt: {s}"),
        }
    }
}

impl std::error::Error for ScanError {}

impl From<std::io::Error> for ScanError {
    fn from(e: std::io::Error) -> Self {
        ScanError::Io(e)
    }
}

impl From<mft_parse::MftError> for ScanError {
    fn from(e: mft_parse::MftError) -> Self {
        ScanError::Mft(e)
    }
}

/// Logik: prefer_mft && platform::is_admin() && Laufwerk lokal && NTFS -> mft; bei Err -> walk (meta.mode = Walk).
pub fn scan(
    req: &ScanRequest,
    progress: &dyn Fn(&ScanProgress),
    cancel: &AtomicBool,
) -> Result<Tree, ScanError> {
    #[cfg(windows)]
    if req.prefer_mft && crate::platform::is_admin() {
        if let Ok(t) = scan_mft(req, progress, cancel) {
            return Ok(t);
        }
    }
    scan_walk(req, progress, cancel)
}
