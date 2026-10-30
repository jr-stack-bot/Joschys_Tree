// Joschys_Tree – Plattform-Schicht: Admin, Löschen, Explorer, ACL, ADS, Volume – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Öffentliche Plattform-Funktionen. Unter Windows echte Implementierung (`win.rs`),
//! sonst Stubs. Reine Hilfslogik (Pfad-Umwandlung, Rechte-Text, Stream-Namen,
//! Laufwerksliste) steht hier plattformneutral und ist auf Linux getestet.

#[cfg(windows)]
mod win;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SecurityInfo {
    pub owner: String,
    pub aces: Vec<Ace>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ace {
    pub principal: String,
    pub allow: bool,
    pub rights: String,
    pub inherited: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StreamInfo {
    pub name: String,
    pub size: u64,
}

#[cfg(windows)]
pub use win::{
    alternate_streams, delete_permanently, is_admin, list_drives, long_path, move_to_recycle_bin,
    open_in_explorer, security_info, volume_info,
};

#[cfg(windows)]
pub(crate) use win::compressed_size;

#[cfg(not(windows))]
pub use stub::{
    alternate_streams, delete_permanently, is_admin, list_drives, long_path, move_to_recycle_bin,
    open_in_explorer, security_info, volume_info,
};

// ---------------------------------------------------------------------------
// Plattformneutrale Hilfslogik (auf Linux getestet)
// ---------------------------------------------------------------------------

/// Wandelt einen absoluten Windows-Pfad in die `\\?\`-Form für lange Pfade um.
///
/// * `C:\a\b` / `C:/a/b` → `\\?\C:\a\b`
/// * `C:` → `\\?\C:\`
/// * `\\server\share\x` → `\\?\UNC\server\share\x`
/// * bereits `\\?\…` oder `\\.\…` → unverändert
/// * relative Pfade → unverändert (können nicht präfixiert werden)
///
/// `.`-Komponenten werden entfernt und `..` aufgelöst, weil `\\?\` das nicht mehr tut.
pub fn to_long_path(path: &str) -> String {
    if path.starts_with(r"\\?\") || path.starts_with(r"\\.\") {
        return path.to_string();
    }
    let p = path.replace('/', "\\");
    if let Some(rest) = p.strip_prefix(r"\\") {
        // UNC: \\server\share\rest
        let mut parts = rest.split('\\').filter(|s| !s.is_empty());
        let (Some(server), Some(share)) = (parts.next(), parts.next()) else {
            return path.to_string();
        };
        let tail = normalize_components(parts);
        let mut out = format!(r"\\?\UNC\{server}\{share}");
        if !tail.is_empty() {
            out.push('\\');
            out.push_str(&tail);
        }
        return out;
    }
    let b = p.as_bytes();
    if b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':' {
        if b.len() > 2 && b[2] != b'\\' {
            return path.to_string(); // "C:relativ" – laufwerksrelativ, nicht präfixierbar
        }
        let drive = (b[0] as char).to_ascii_uppercase();
        let tail = normalize_components(p[2..].split('\\').filter(|s| !s.is_empty()));
        return format!(r"\\?\{drive}:\{tail}");
    }
    path.to_string()
}

fn normalize_components<'a>(parts: impl Iterator<Item = &'a str>) -> String {
    let mut out: Vec<&str> = Vec::new();
    for c in parts {
        match c {
            "." => {}
            ".." => {
                out.pop();
            }
            _ => out.push(c),
        }
    }
    out.join("\\")
}

/// Entfernt das `\\?\`-Präfix wieder (für Explorer und Shell-APIs, die es nicht kennen).
pub fn strip_long_prefix(path: &str) -> String {
    if let Some(rest) = path.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = path.strip_prefix(r"\\?\") {
        rest.to_string()
    } else {
        path.to_string()
    }
}

/// Bitmaske von `GetLogicalDrives` → `["C:\\", "D:\\", …]`.
pub fn drives_from_mask(mask: u32) -> Vec<String> {
    (0..26u8)
        .filter(|i| mask & (1 << i) != 0)
        .map(|i| format!("{}:\\", (b'A' + i) as char))
        .collect()
}

/// Stream-Name aus `FindFirstStreamW` (`:name:$DATA`) → `name`.
/// Der Hauptstrom (`::$DATA`) liefert `None`.
pub fn stream_display_name(raw: &str) -> Option<String> {
    let s = raw.strip_prefix(':').unwrap_or(raw);
    let name = s.strip_suffix(":$DATA").unwrap_or(s);
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

/// Zugriffsmaske einer Datei-ACE in lesbaren Text (Kurzformen wie im Explorer).
pub fn format_access_mask(mask: u32) -> String {
    const FULL: u32 = 0x001F_01FF;
    const MODIFY: u32 = 0x0013_01BF;
    const READ_EXEC: u32 = 0x0012_00A9;
    const READ: u32 = 0x0012_0089;
    const WRITE: u32 = 0x0010_0116;
    const GENERIC_ALL: u32 = 0x1000_0000;
    const GENERIC_EXECUTE: u32 = 0x2000_0000;
    const GENERIC_WRITE: u32 = 0x4000_0000;
    const GENERIC_READ: u32 = 0x8000_0000;

    if mask & GENERIC_ALL != 0 || mask & FULL == FULL {
        return String::from("FullControl");
    }
    if mask & MODIFY == MODIFY {
        return String::from("Modify");
    }
    let mut parts: Vec<&str> = Vec::new();
    if mask & READ_EXEC == READ_EXEC
        || mask & (GENERIC_READ | GENERIC_EXECUTE) == (GENERIC_READ | GENERIC_EXECUTE)
    {
        parts.push("ReadAndExecute");
    } else if mask & READ == READ || mask & GENERIC_READ != 0 {
        parts.push("Read");
    }
    if mask & WRITE == WRITE || mask & GENERIC_WRITE != 0 {
        parts.push("Write");
    }
    if parts.is_empty() {
        return format!("0x{mask:08X}");
    }
    parts.join(", ")
}

// ---------------------------------------------------------------------------
// Stubs für Nicht-Windows
// ---------------------------------------------------------------------------

#[cfg(not(windows))]
mod stub {
    use super::{SecurityInfo, StreamInfo};
    use std::io;

    fn unsupported<T>() -> io::Result<T> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "nur unter Windows verfügbar",
        ))
    }

    pub fn is_admin() -> bool {
        false
    }

    pub fn move_to_recycle_bin(_paths: &[String]) -> io::Result<()> {
        unsupported()
    }

    pub fn delete_permanently(_paths: &[String]) -> io::Result<()> {
        unsupported()
    }

    /// explorer /select,"<pfad>"
    pub fn open_in_explorer(_path: &str) -> io::Result<()> {
        unsupported()
    }

    pub fn security_info(_path: &str) -> io::Result<SecurityInfo> {
        unsupported()
    }

    pub fn alternate_streams(_path: &str) -> io::Result<Vec<StreamInfo>> {
        unsupported()
    }

    /// (Dateisystem, Clustergröße, Gesamt, Frei)
    pub fn volume_info(_root: &str) -> io::Result<(String, u32, u64, u64)> {
        unsupported()
    }

    /// Auf Nicht-Windows unverändert.
    pub fn long_path(path: &str) -> String {
        path.to_string()
    }

    pub fn list_drives() -> Vec<String> {
        Vec::new()
    }
}
