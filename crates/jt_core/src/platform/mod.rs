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
use crate::protect::{
    NormEnv, ProtectEnv, ProtectError, Protection, Reason, classify_pair_norm, stricter,
};
use std::io;
use std::sync::OnceLock;

/// Schutz-Umgebung des laufenden Systems, einmalig aus [`ProtectEnv::from_system`].
pub fn protect_env() -> &'static ProtectEnv {
    static ENV: OnceLock<ProtectEnv> = OnceLock::new();
    ENV.get_or_init(ProtectEnv::from_system)
}

/// Attribute und Ordner-Eigenschaft eines Pfads, soweit lesbar (fehlt der Pfad:
/// leer/Datei; sonst unlesbar: HIDDEN).
/// SYSTEM/HIDDEN werden (unter Windows) von den Elternordnern geerbt, soweit deren
/// Attribute lesbar sind; die Laufwerks-/Freigabe-Wurzel zählt nicht (sie trägt
/// unter Windows selbst HIDDEN|SYSTEM).
fn path_facts(path: &str) -> (NodeFlags, bool) {
    let lp = long_path(path);
    let md = match std::fs::symlink_metadata(&lp) {
        Ok(md) => md,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return (NodeFlags::empty(), false),
        // Attribute unlesbar (z. B. Zugriff verweigert): konservativ wie versteckt
        // behandeln (mindestens Vorsicht); gesperrt wird weiter nach dem Pfadtext.
        Err(_) => return (NodeFlags::HIDDEN, false),
    };
    let mut flags = NodeFlags::empty();
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        for anc in std::path::Path::new(&lp).ancestors().skip(1) {
            if anc.parent().is_none() {
                break; // Wurzel
            }
            if let Ok(m) = std::fs::symlink_metadata(anc) {
                let a = m.file_attributes();
                if a & 0x2 != 0 {
                    flags |= NodeFlags::HIDDEN;
                }
                if a & 0x4 != 0 {
                    flags |= NodeFlags::SYSTEM;
                }
            }
        }
    }
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

/// Ergebnis der Pfad-Auflösung für den Löschschutz.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Resolved {
    /// Echter Pfad (Elternordner aufgelöst, Name unverändert).
    Real(String),
    /// Ziel existiert nicht (mehr): nichts zu löschen, nur der Roh-Pfad zählt.
    Gone,
    /// Elternordner nicht auflösbar, Ziel existiert aber → gesperrt.
    Unresolvable,
}

/// Echte Auflösung: `canonicalize` des **Elternordners** plus unveränderter Name.
/// Ein Link selbst wird so nicht verfolgt (er wird gelöscht, nicht sein Ziel), aber
/// subst-/Netzlaufwerke und Junctions im Elternpfad werden auf das echte Ziel
/// aufgelöst. Unter Windows liefert `canonicalize` `\\?\C:\…` bzw. `\\?\UNC\…`;
/// das Präfix entfernt [`strip_long_prefix`] (UNC bleibt UNC).
pub fn resolve_real(path: &str) -> Resolved {
    let lp = long_path(path);
    let p = std::path::Path::new(&lp);
    let (Some(parent), Some(name)) = (p.parent(), p.file_name()) else {
        // Wurzel o. Ä.: kein Elternordner, der Roh-Pfad wird ohnehin bewertet.
        return Resolved::Real(path.to_string());
    };
    match std::fs::canonicalize(parent) {
        Ok(rp) => Resolved::Real(strip_long_prefix(&rp.join(name).to_string_lossy())),
        Err(_) => match std::fs::symlink_metadata(&lp) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => Resolved::Gone,
            _ => Resolved::Unresolvable,
        },
    }
}

/// Prüft alle Pfade, bevor irgendetwas gelöscht wird (alles oder nichts):
/// Ist ein Pfad `Blocked`, endet der Aufruf mit `ErrorKind::PermissionDenied` und einer
/// [`ProtectError`]-Nutzlast (Pfad, Grund, Anzahl). `Caution` ist erlaubt.
/// Bewertet wird der Roh-Pfad **und** der aufgelöste Pfad ([`resolve_real`]); die
/// strengere Stufe gilt.
pub fn guard_paths(paths: &[String], env: &ProtectEnv) -> io::Result<()> {
    guard_paths_with(paths, env, &resolve_real)
}

/// Wie [`guard_paths`] mit eigener Auflösung (Tests: Windows-Szenarien auf Linux).
pub fn guard_paths_with(
    paths: &[String],
    env: &ProtectEnv,
    resolve: &dyn Fn(&str) -> Resolved,
) -> io::Result<()> {
    let ne = NormEnv::new(env);
    let mut first: Option<ProtectError> = None;
    let mut count = 0usize;
    for p in paths {
        let (flags, is_dir) = path_facts(p);
        let prot = match resolve(p) {
            Resolved::Real(r) => classify_pair_norm(p, Some(&r), flags, is_dir, &ne),
            Resolved::Gone => classify_pair_norm(p, None, flags, is_dir, &ne),
            Resolved::Unresolvable => stricter(
                classify_pair_norm(p, None, flags, is_dir, &ne),
                Protection::Blocked(Reason::Unresolvable),
            ),
        };
        if let Protection::Blocked(reason) = prot {
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

    fn reason_of(e: &io::Error) -> Reason {
        e.get_ref()
            .and_then(|e| e.downcast_ref::<ProtectError>())
            .expect("ProtectError")
            .reason
    }

    /// Fake-Auflösung: Präfix ersetzen (Windows-Szenarien auf Linux).
    fn fake(map: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Resolved {
        move |p: &str| {
            for (from, to) in map {
                if p.len() >= from.len() && p[..from.len()].eq_ignore_ascii_case(from) {
                    return Resolved::Real(format!("{to}{}", &p[from.len()..]));
                }
            }
            Resolved::Real(p.to_string())
        }
    }

    #[test]
    fn guard_checks_resolved_path() {
        let env = ProtectEnv::windows_default();
        let one =
            |p: &str, r: &dyn Fn(&str) -> Resolved| guard_paths_with(&[p.to_string()], &env, r);
        // subst X: = C:\
        let subst = fake(&[(r"X:\", r"\\?\C:\")]);
        let e = one(r"X:\Windows\System32", &subst).expect_err("subst");
        assert_eq!(reason_of(&e), Reason::WindowsDir);
        // ohne Auflösung wäre es nur Vorsicht
        assert!(one(r"X:\Windows\System32", &|p: &str| Resolved::Real(p.into())).is_ok());
        // Netzlaufwerk Z: = \\server\c$
        let net = fake(&[(r"Z:\", r"\\?\UNC\server\c$\")]);
        let e = one(r"Z:\Windows\System32", &net).expect_err("netz");
        assert_eq!(reason_of(&e), Reason::WindowsDir);
        // Netzlaufwerk auf normale Freigabe: Systemordner dort gesperrt
        let nas = fake(&[(r"Y:\", r"\\nas\daten\")]);
        let e = one(r"Y:\Program Files\x", &nas).expect_err("nas");
        assert_eq!(reason_of(&e), Reason::AliasedSystem);
        // Freigabe namens C
        let e = one(r"\\pc\C\Windows\System32", &|p: &str| {
            Resolved::Real(p.into())
        })
        .expect_err("share C");
        assert_eq!(reason_of(&e), Reason::AliasedSystem);
        // 8.3-Kurzname tief im Profil
        let e = one(
            r"C:\Users\joschy\AppData\Local\Microsoft\WINDOW~1",
            &|p: &str| Resolved::Real(p.into()),
        )
        .expect_err("kurzname");
        assert_eq!(reason_of(&e), Reason::ShortName);
        // subst auf ein anderes echtes Laufwerk: Vorsicht, erlaubt
        let other = fake(&[(r"X:\", r"D:\")]);
        assert!(one(r"X:\Windows\x", &other).is_ok());
        // nicht auflösbar, Ziel existiert → gesperrt; verschwunden → nichts zu sperren
        let e = one(r"D:\Daten\x", &|_: &str| Resolved::Unresolvable).expect_err("unres");
        assert_eq!(reason_of(&e), Reason::Unresolvable);
        assert!(one(r"D:\Daten\x", &|_: &str| Resolved::Gone).is_ok());
    }

    #[test]
    fn path_facts_missing_and_unreadable() {
        assert_eq!(
            path_facts("/gibt/es/nicht/joschy_x"),
            (NodeFlags::empty(), false)
        );
        // Ein Pfad "unter" einer Datei liefert NotADirectory (kein NotFound) → vorsichtig.
        let dir = tempfile::tempdir().expect("tempdir");
        let f = dir.path().join("datei");
        std::fs::write(&f, b"x").expect("write");
        let below = f.join("kind").to_string_lossy().into_owned();
        if std::fs::symlink_metadata(&below)
            .err()
            .is_some_and(|e| e.kind() != io::ErrorKind::NotFound)
        {
            assert_eq!(path_facts(&below), (NodeFlags::HIDDEN, false));
        }
    }

    #[test]
    fn resolve_real_basics() {
        let dir = tempfile::tempdir().expect("tempdir");
        let base = std::fs::canonicalize(dir.path()).expect("canonicalize");
        let f = base.join("a.txt");
        std::fs::write(&f, b"x").expect("write");
        let fs = f.to_string_lossy().into_owned();
        assert_eq!(resolve_real(&fs), Resolved::Real(strip_long_prefix(&fs)));
        // Elternordner fehlt, Ziel auch → Gone
        let gone = base.join("fehlt").join("b.txt");
        assert_eq!(resolve_real(&gone.to_string_lossy()), Resolved::Gone);
    }

    #[cfg(unix)]
    #[test]
    fn symlink_parent_into_protected_dir() {
        let dir = tempfile::tempdir().expect("tempdir");
        let base = std::fs::canonicalize(dir.path()).expect("canonicalize");
        let win = base.join("win");
        std::fs::create_dir(&win).expect("mkdir");
        let sys = win.join("kernel.dll");
        std::fs::write(&sys, b"x").expect("write");
        let link = base.join("link");
        std::os::unix::fs::symlink(&win, &link).expect("symlink");
        let env = ProtectEnv {
            windows_dir: win.to_string_lossy().into_owned(),
            ..ProtectEnv::windows_default()
        };
        let via_link = link.join("kernel.dll").to_string_lossy().into_owned();
        // Text allein wäre frei, der aufgelöste Pfad liegt im "Windows-Ordner".
        assert!(
            !crate::protect::classify_path(&via_link, NodeFlags::empty(), false, &env).is_blocked()
        );
        let e = guard_paths(std::slice::from_ref(&via_link), &env).expect_err("link");
        assert_eq!(reason_of(&e), Reason::WindowsDir);
        let r = delete_permanently_with_env(std::slice::from_ref(&via_link), &env);
        assert_eq!(
            r.expect_err("abbruch").kind(),
            io::ErrorKind::PermissionDenied
        );
        assert!(sys.exists());
        // Der Link selbst wird nicht verfolgt: löschbar (nur der Link).
        let l = link.to_string_lossy().into_owned();
        assert!(guard_paths(&[l], &env).is_ok());
        assert!(guard_paths(&[win.to_string_lossy().into_owned()], &env).is_err());
    }
}
