// Joschys_Tree – Scanner: ScanRequest, ScanProgress, Auswahl MFT/Walk mit Fallback – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fmt;
use std::sync::atomic::AtomicBool;

use crate::model::{FileTime, ScanMode, Tree};

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

/// Laufwerksbuchstabe, wenn `root` die Wurzel eines lokalen Laufwerks ist
/// (`C:`, `C:\`, `c:/`, `\\?\C:\`). Unterordner, UNC und relative Pfade → `None`.
/// Nur Laufwerkswurzeln werden per MFT gescannt.
pub fn drive_root_letter(root: &str) -> Option<char> {
    let r = root.strip_prefix(r"\\?\").unwrap_or(root);
    let b = r.as_bytes();
    let ok_tail = match b.len() {
        2 => true,
        3 => b[2] == b'\\' || b[2] == b'/',
        _ => false,
    };
    if ok_tail && b[0].is_ascii_alphabetic() && b[1] == b':' {
        Some((b[0] as char).to_ascii_uppercase())
    } else {
        None
    }
}

/// Fallback-Entscheidung (rein, testbar):
/// MFT nur wenn gewünscht, Admin, lokale Laufwerkswurzel und NTFS. Sonst Walk.
pub fn choose_mode(
    prefer_mft: bool,
    is_admin: bool,
    root: &str,
    fs_name: Option<&str>,
) -> ScanMode {
    let ntfs = fs_name.is_some_and(|f| f.eq_ignore_ascii_case("NTFS"));
    if prefer_mft && is_admin && ntfs && drive_root_letter(root).is_some() {
        ScanMode::Mft
    } else {
        ScanMode::Walk
    }
}

/// Aktuelle Zeit als FILETIME.
pub(crate) fn now_filetime() -> FileTime {
    let d = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    FileTime(FileTime::from_unix_secs(d.as_secs() as i64).0 + (d.subsec_nanos() / 100) as i64)
}

/// Logik: prefer_mft && platform::is_admin() && Laufwerk lokal && NTFS -> mft;
/// bei Fehler -> walk (meta.mode = Walk). Abbruch wird nicht in einen Walk umgeleitet.
pub fn scan(
    req: &ScanRequest,
    progress: &dyn Fn(&ScanProgress),
    cancel: &AtomicBool,
) -> Result<Tree, ScanError> {
    let admin = crate::platform::is_admin();
    // Volume-Abfrage nur, wenn MFT überhaupt in Frage kommt (UNC kann langsam sein).
    let fs = if req.prefer_mft && admin && drive_root_letter(&req.root).is_some() {
        crate::platform::volume_info(&req.root).ok().map(|v| v.0)
    } else {
        None
    };
    let mode = choose_mode(req.prefer_mft, admin, &req.root, fs.as_deref());
    #[cfg(windows)]
    if mode == ScanMode::Mft {
        match scan_mft(req, progress, cancel) {
            Ok(t) => return Ok(t),
            Err(ScanError::Cancelled) => return Err(ScanError::Cancelled),
            Err(_) => {} // Fallback auf Walk
        }
    }
    #[cfg(not(windows))]
    let _ = mode;
    scan_walk(req, progress, cancel)
}
