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
    alternate_streams, is_admin, list_drives, long_path, open_in_explorer, security_info,
    volume_info,
};

#[cfg(windows)]
use win::{
    delete_permanently as raw_delete_permanently, move_to_recycle_bin as raw_move_to_recycle_bin,
};

#[cfg(windows)]
pub(crate) use win::compressed_size;

#[cfg(not(windows))]
pub use stub::{
    alternate_streams, is_admin, list_drives, long_path, open_in_explorer, security_info,
    volume_info,
};

#[cfg(not(windows))]
use stub::{
    delete_permanently as raw_delete_permanently, move_to_recycle_bin as raw_move_to_recycle_bin,
};

// ---------------------------------------------------------------------------
// Löschschutz im Löschweg (Defense in Depth, plattformneutral)
// ---------------------------------------------------------------------------

use crate::model::NodeFlags;
use crate::protect::{NormEnv, ProtectEnv, ProtectError, Protection, classify_path_norm};
use std::io;
use std::sync::OnceLock;

/// Schutz-Umgebung des laufenden Systems, einmalig aus [`ProtectEnv::from_system`].
pub fn protect_env() -> &'static ProtectEnv {
    static ENV: OnceLock<ProtectEnv> = OnceLock::new();
    ENV.get_or_init(ProtectEnv::from_system)
}

/// Attribute und Ordner-Eigenschaft eines Pfads, soweit lesbar (sonst leer/Datei).
fn path_facts(path: &str) -> (NodeFlags, bool) {
    let Ok(md) = std::fs::symlink_metadata(long_path(path)) else {
        return (NodeFlags::empty(), false);
    };
    let mut flags = NodeFlags::empty();
    if md.file_type().is_symlink() {
        flags |= NodeFlags::REPARSE;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        let a = md.file_attributes();
        if a & 0x2 != 0 {
            flags |= NodeFlags::HIDDEN;
        }
        if a & 0x4 != 0 {
            flags |= NodeFlags::SYSTEM;
        }
        if a & 0x400 != 0 {
            flags |= NodeFlags::REPARSE;
        }
    }
    if md.is_dir() {
        flags |= NodeFlags::DIR;
    }
    (flags, md.is_dir())
}

/// Prüft alle Pfade, bevor irgendetwas gelöscht wird (alles oder nichts):
/// Ist ein Pfad `Blocked`, endet der Aufruf mit `ErrorKind::PermissionDenied` und einer
/// [`ProtectError`]-Nutzlast (Pfad, Grund, Anzahl). `Caution` ist erlaubt.
pub fn guard_paths(paths: &[String], env: &ProtectEnv) -> io::Result<()> {
    let ne = NormEnv::new(env);
    let mut first: Option<ProtectError> = None;
    let mut count = 0usize;
    for p in paths {
        let (flags, is_dir) = path_facts(p);
        if let Protection::Blocked(reason) = classify_path_norm(p, flags, is_dir, &ne) {
            count += 1;
            if first.is_none() {
                first = Some(ProtectError {
                    path: p.clone(),
                    reason,
                    blocked_count: 0,
                });
            }
        }
    }
    match first {
        None => Ok(()),
        Some(mut e) => {
            e.blocked_count = count;
            Err(io::Error::new(io::ErrorKind::PermissionDenied, e))
        }
    }
}

/// Führt `op` nur aus, wenn [`guard_paths`] alle Pfade freigibt.
pub fn guarded<F>(paths: &[String], env: &ProtectEnv, op: F) -> io::Result<()>
where
    F: FnOnce(&[String]) -> io::Result<()>,
{
    guard_paths(paths, env)?;
    op(paths)
}

/// In den Papierkorb (nie endgültig), vorher Löschschutz-Prüfung für alle Pfade.
pub fn move_to_recycle_bin(paths: &[String]) -> io::Result<()> {
    move_to_recycle_bin_with_env(paths, protect_env())
}

/// Wie [`move_to_recycle_bin`], mit eigener Schutz-Umgebung (Tests).
pub fn move_to_recycle_bin_with_env(paths: &[String], env: &ProtectEnv) -> io::Result<()> {
    guarded(paths, env, raw_move_to_recycle_bin)
}

/// Endgültig löschen, vorher Löschschutz-Prüfung für alle Pfade.
pub fn delete_permanently(paths: &[String]) -> io::Result<()> {
    delete_permanently_with_env(paths, protect_env())
}

/// Wie [`delete_permanently`], mit eigener Schutz-Umgebung (Tests).
pub fn delete_permanently_with_env(paths: &[String], env: &ProtectEnv) -> io::Result<()> {
    guarded(paths, env, raw_delete_permanently)
}

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

/// Laufwerkstyp von `GetDriveTypeW` → hat das Laufwerk einen Papierkorb?
/// Nur feste lokale Laufwerke (`DRIVE_FIXED` = 3). Netzfreigaben (4), Wechseldatenträger
/// wie USB-Sticks (2), CD (5), RAM-Disk (6) und Unbekanntes haben keinen; dort würde die
/// Shell "in den Papierkorb" stillschweigend endgültig löschen.
pub fn drive_type_has_recycle_bin(drive_type: u32) -> bool {
    drive_type == 3
}

/// Fehler "kein Papierkorb" (`ErrorKind::Unsupported`); die GUI zeigt dafür einen
/// eigenen Hinweis und verweist auf "Endgültig löschen" (mit zweiter Bestätigung).
pub fn no_recycle_bin_error(path: &str) -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        format!("kein Papierkorb auf diesem Laufwerk: {path}"),
    )
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

#[cfg(test)]
mod guard_tests {
    use super::*;
    use crate::protect::Reason;

    fn temp_files() -> (tempfile::TempDir, Vec<String>) {
        let dir = tempfile::tempdir().expect("tempdir");
        // Lange Form (Windows-CI nutzt sonst evtl. 8.3-Kurznamen wie RUNNER~1).
        let base = std::fs::canonicalize(dir.path()).expect("canonicalize");
        let mut v = Vec::new();
        for n in ["a.txt", "b.txt"] {
            let p = base.join(n);
            std::fs::write(&p, b"x").expect("write");
            v.push(p.to_string_lossy().into_owned());
        }
        (dir, v)
    }

    fn exists(paths: &[String]) -> bool {
        paths.iter().all(|p| std::path::Path::new(p).exists())
    }

    #[test]
    fn mixed_batch_deletes_nothing() {
        let env = ProtectEnv::windows_default();
        let (_d, files) = temp_files();
        let mut batch = files.clone();
        batch.push(r"C:\Windows\System32".into());
        batch.push(r"C:\pagefile.sys".into());
        let mut ran = false;
        let r = guarded(&batch, &env, |ps| {
            ran = true;
            for p in ps {
                std::fs::remove_file(p)?;
            }
            Ok(())
        });
        let err = r.expect_err("muss abbrechen");
        assert_eq!(err.kind(), io::ErrorKind::PermissionDenied);
        let pe = err
            .get_ref()
            .and_then(|e| e.downcast_ref::<ProtectError>())
            .expect("ProtectError");
        assert_eq!(pe.reason, Reason::WindowsDir);
        assert_eq!(pe.blocked_count, 2);
        assert!(!ran);
        assert!(exists(&files));
        // Auch die echten Löschfunktionen brechen vor dem Plattform-Code ab.
        for r in [
            delete_permanently_with_env(&batch, &env),
            move_to_recycle_bin_with_env(&batch, &env),
        ] {
            assert_eq!(
                r.expect_err("abbruch").kind(),
                io::ErrorKind::PermissionDenied
            );
        }
        assert!(exists(&files));
    }

    #[test]
    fn free_batch_runs() {
        let env = ProtectEnv::windows_default();
        let (_d, files) = temp_files();
        assert!(guard_paths(&files, &env).is_ok());
        guarded(&files, &env, |ps| {
            for p in ps {
                std::fs::remove_file(p)?;
            }
            Ok(())
        })
        .expect("löschen");
        assert!(files.iter().all(|p| !std::path::Path::new(p).exists()));
        assert!(guard_paths(&[], &env).is_ok());
    }

    #[test]
    fn guard_rejects_classics() {
        let env = ProtectEnv::windows_default();
        for p in [
            r"C:\",
            r"\\?\C:\Windows",
            r"c:/program files/x",
            r"C:\Users",
            r"C:\Users\joschy",
            "relativ",
            r"C:\Daten\..\Windows",
        ] {
            let e = guard_paths(&[p.to_string()], &env).expect_err(p);
            assert_eq!(e.kind(), io::ErrorKind::PermissionDenied);
            assert!(!e.to_string().is_empty());
        }
        assert!(guard_paths(&[r"C:\Windows.old".into()], &env).is_ok());
        assert!(std::ptr::eq(protect_env(), protect_env()));
    }
}
