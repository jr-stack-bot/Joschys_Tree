// Joschys_Tree – Löschschutz: Windows-relevante Pfade erkennen (Frei/Vorsicht/Geschützt) – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Löschschutz. Reine, plattformneutrale Logik: Windows-Pfade werden als Text
//! bewertet und sind deshalb auch auf Linux testbar.
//!
//! # Normalisierung
//! * Vergleich case-insensitiv, `/` und `\` gleichwertig, doppelte und
//!   nachlaufende Trenner werden ignoriert.
//! * Präfixe `\\?\`, `\\.\`, `\??\` und `\\?\UNC\` werden entfernt.
//! * Nachlaufende Punkte/Leerzeichen einer Komponente werden abgeschnitten
//!   (Win32 tut das auch: `C:\Windows.` ist `C:\Windows`).
//! * Administrative Freigaben werden aufgelöst: `\\server\c$\…` gilt als `C:\…`,
//!   `\\server\admin$\…` als `%SystemRoot%\…`.
//! * Pfade ohne Laufwerk, aber mit führendem `\` liegen auf dem Systemlaufwerk.
//!   Relative Pfade, `.`/`..`-Segmente und unzulässige Zeichen (`<>:"|?*`,
//!   Steuerzeichen) sind ungültig → **Blocked (InvalidPath)**.
//!
//! # Regeln
//! **Blocked** (nie löschbar):
//! * Wurzel jedes Laufwerks und jeder Freigabe (`VolumeRoot`).
//! * Komponenten im 8.3-Kurznamen-Format (`PROGRA~1`) in den ersten drei Ebenen (`ShortName`).
//! * `%SystemRoot%` samt allem darunter (Temp, WinSxS, System32 …) (`WindowsDir`).
//! * `Program Files` / `Program Files (x86)` samt Inhalt (`ProgramFiles`) –
//!   Programme werden über die Windows-Einstellungen deinstalliert.
//! * `ProgramData` selbst (`ProgramDataRoot`), `ProgramData\Microsoft` samt Inhalt (`ProgramDataMicrosoft`).
//! * In der Wurzel eines Laufwerks: `System Volume Information`, `Recovery`, `Boot`,
//!   `EFI`, `Config.Msi`, `Documents and Settings` und alles, was mit `$` beginnt
//!   (`$Recycle.Bin`, `$WinREAgent`, NTFS-Metadateien wie `$MFT`) samt Inhalt
//!   (`SystemFolder`/`SystemFile`), dazu `pagefile.sys`, `hiberfil.sys`,
//!   `swapfile.sys`, `bootmgr`, `BOOTNXT`, `bootsect.bak` (`SystemFile`).
//! * Der Ordner `Users` selbst (`UsersDir`) und jede Profil-Wurzel
//!   (`Users\<name>`, auch Default, Public, All Users, Default User) (`ProfileRoot`).
//! * Im Profil: `NTUSER.DAT*`, `UsrClass.dat*` (`RegistryHive`); die Ordner `AppData`,
//!   `AppData\Local`, `\Roaming`, `\LocalLow` und `AppData\Local\Temp` selbst
//!   (`AppDataRoot`, nur der Ordner, nicht der Inhalt); `AppData\Local\Microsoft\Windows`
//!   samt Inhalt (`ProfileSystem`).
//! * Die eigene exe, ihr Ordner und alle Ordner darüber (`OwnProgram`, nur das Element selbst).
//! * Jedes Element mit SYSTEM-Attribut; das gilt auch für alles darunter (`SystemFlag`).
//!
//! **Caution** (löschbar, die GUI warnt zusätzlich):
//! * Junctions/Symlinks (`ReparsePoint`, nur der Link wird entfernt).
//! * `Windows.old` in der Laufwerkswurzel (`WindowsOld`).
//! * Alles unter `AppData` außer `AppData\Local\Temp\…` (`AppData`).
//! * Profil-Bibliotheken Dokumente/Desktop/Bilder/Videos/Musik/OneDrive samt Inhalt (`UserLibrary`).
//! * Alles unter `ProgramData` außer `Microsoft` (`ProgramDataApp`).
//! * `Windows`, `Program Files*`, `ProgramData` in der Wurzel eines Nicht-Systemlaufwerks (`ForeignSystem`).
//! * Versteckte Elemente (auch geerbt vom Elternordner) außerhalb von `temp`/`tmp`-Ordnern (`Hidden`).
//!
//! **Free**: alles andere (z. B. `C:\Backup`, Downloads, Temp-Inhalte, Datenlaufwerke).
//!
//! Vererbung: Ein Element ist Blocked, wenn ein Elternpfad mit einer vererbenden
//! Regel blockiert ist (Präfixlogik). Ein Ordner mit blockierten Nachkommen ist
//! deshalb nicht selbst blockiert; dafür gibt es [`contains_blocked`].

use std::ops::ControlFlow;

use crate::i18n::{Key, Lang, tr};
use crate::model::{NO_NODE, NodeFlags, NodeId, Tree};

/// Grund einer Schutzstufe.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Reason {
    // ---- Blocked ----
    VolumeRoot,
    WindowsDir,
    ProgramFiles,
    ProgramDataRoot,
    ProgramDataMicrosoft,
    SystemFolder,
    SystemFile,
    UsersDir,
    ProfileRoot,
    RegistryHive,
    AppDataRoot,
    ProfileSystem,
    SystemFlag,
    InvalidPath,
    ShortName,
    OwnProgram,
    // ---- Caution ----
    AppData,
    WindowsOld,
    ProgramDataApp,
    UserLibrary,
    ReparsePoint,
    Hidden,
    ForeignSystem,
}

impl Reason {
    pub const ALL: [Reason; 23] = [
        Reason::VolumeRoot,
        Reason::WindowsDir,
        Reason::ProgramFiles,
        Reason::ProgramDataRoot,
        Reason::ProgramDataMicrosoft,
        Reason::SystemFolder,
        Reason::SystemFile,
        Reason::UsersDir,
        Reason::ProfileRoot,
        Reason::RegistryHive,
        Reason::AppDataRoot,
        Reason::ProfileSystem,
        Reason::SystemFlag,
        Reason::InvalidPath,
        Reason::ShortName,
        Reason::OwnProgram,
        Reason::AppData,
        Reason::WindowsOld,
        Reason::ProgramDataApp,
        Reason::UserLibrary,
        Reason::ReparsePoint,
        Reason::Hidden,
        Reason::ForeignSystem,
    ];

    /// i18n-Key mit kurzer Begründung.
    pub fn key(self) -> Key {
        match self {
            Reason::VolumeRoot => Key::ProtReasonVolumeRoot,
            Reason::WindowsDir => Key::ProtReasonWindowsDir,
            Reason::ProgramFiles => Key::ProtReasonProgramFiles,
            Reason::ProgramDataRoot => Key::ProtReasonProgramDataRoot,
            Reason::ProgramDataMicrosoft => Key::ProtReasonProgramDataMicrosoft,
            Reason::SystemFolder => Key::ProtReasonSystemFolder,
            Reason::SystemFile => Key::ProtReasonSystemFile,
            Reason::UsersDir => Key::ProtReasonUsersDir,
            Reason::ProfileRoot => Key::ProtReasonProfileRoot,
            Reason::RegistryHive => Key::ProtReasonRegistryHive,
            Reason::AppDataRoot => Key::ProtReasonAppDataRoot,
            Reason::ProfileSystem => Key::ProtReasonProfileSystem,
            Reason::SystemFlag => Key::ProtReasonSystemFlag,
            Reason::InvalidPath => Key::ProtReasonInvalidPath,
            Reason::ShortName => Key::ProtReasonShortName,
            Reason::OwnProgram => Key::ProtReasonOwnProgram,
            Reason::AppData => Key::ProtReasonAppData,
            Reason::WindowsOld => Key::ProtReasonWindowsOld,
            Reason::ProgramDataApp => Key::ProtReasonProgramDataApp,
            Reason::UserLibrary => Key::ProtReasonUserLibrary,
            Reason::ReparsePoint => Key::ProtReasonReparsePoint,
            Reason::Hidden => Key::ProtReasonHidden,
            Reason::ForeignSystem => Key::ProtReasonForeignSystem,
        }
    }

    /// Text in der gewünschten Sprache.
    pub fn text(self, lang: Lang) -> &'static str {
        tr(lang, self.key())
    }

    /// Vererbt sich ein Blocked mit diesem Grund auf alles darunter?
    /// (Laufwerkswurzel, Users, Profil-Wurzel usw. gelten nur für das Element selbst.)
    pub fn is_inherited(self) -> bool {
        matches!(
            self,
            Reason::WindowsDir
                | Reason::ProgramFiles
                | Reason::ProgramDataMicrosoft
                | Reason::SystemFolder
                | Reason::ProfileSystem
                | Reason::SystemFlag
                | Reason::InvalidPath
                | Reason::ShortName
        )
    }
}

/// Schutzstufe eines Elements.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Protection {
    Free,
    /// Löschbar, aber die GUI verlangt eine zusätzliche Warnung.
    Caution(Reason),
    /// Nie löschbar.
    Blocked(Reason),
}

impl Protection {
    pub fn is_blocked(self) -> bool {
        matches!(self, Protection::Blocked(_))
    }

    pub fn is_caution(self) -> bool {
        matches!(self, Protection::Caution(_))
    }

    pub fn reason(self) -> Option<Reason> {
        match self {
            Protection::Free => None,
            Protection::Caution(r) | Protection::Blocked(r) => Some(r),
        }
    }

    /// i18n-Key der Stufe (Frei / Vorsicht / Geschützt).
    pub fn level_key(self) -> Key {
        match self {
            Protection::Free => Key::ProtLevelFree,
            Protection::Caution(_) => Key::ProtLevelCaution,
            Protection::Blocked(_) => Key::ProtLevelBlocked,
        }
    }
}

/// Umgebung: wo liegen Windows, Programme, Profile und die eigene exe.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProtectEnv {
    /// z. B. "C:"
    pub system_drive: String,
    /// z. B. "C:\\Windows"
    pub windows_dir: String,
    /// "C:\\Program Files", "C:\\Program Files (x86)"
    pub program_files: Vec<String>,
    /// "C:\\ProgramData"
    pub program_data: String,
    /// "C:\\Users"
    pub users_dir: String,
    /// Zusätzliche Profil-Wurzeln (aktuelles Profil, Public), auch außerhalb von `users_dir`.
    pub user_profiles: Vec<String>,
    /// Pfad der laufenden exe.
    pub own_exe: Option<String>,
}

impl ProtectEnv {
    /// Windows-Standardwerte (Systemlaufwerk C:).
    pub fn windows_default() -> Self {
        ProtectEnv {
            system_drive: "C:".into(),
            windows_dir: r"C:\Windows".into(),
            program_files: vec![r"C:\Program Files".into(), r"C:\Program Files (x86)".into()],
            program_data: r"C:\ProgramData".into(),
            users_dir: r"C:\Users".into(),
            user_profiles: Vec::new(),
            own_exe: None,
        }
    }

    /// Liest SystemRoot, SystemDrive, ProgramFiles, ProgramFiles(x86), ProgramW6432,
    /// ProgramData, USERPROFILE, PUBLIC und die eigene exe. Ohne Windows bzw. ohne
    /// Variablen gelten die Windows-Standardwerte.
    pub fn from_system() -> Self {
        let mut env = Self::windows_default();
        if !cfg!(windows) {
            return env;
        }
        let var = |k: &str| {
            std::env::var(k)
                .ok()
                .map(|s| s.trim().trim_end_matches(['\\', '/']).to_string())
                .filter(|s| !s.is_empty())
        };
        let drive_of = |s: &str| {
            let b = s.as_bytes();
            (b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':').then(|| s[..2].to_string())
        };
        let root = var("SystemRoot").or_else(|| var("windir"));
        if let Some(d) = var("SystemDrive")
            .and_then(|d| drive_of(&d))
            .or_else(|| root.as_deref().and_then(drive_of))
        {
            env.system_drive = d;
        }
        let sd = env.system_drive.clone();
        env.windows_dir = root.unwrap_or_else(|| format!(r"{sd}\Windows"));
        let mut pf: Vec<String> = ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"]
            .iter()
            .filter_map(|k| var(k))
            .collect();
        pf.push(format!(r"{sd}\Program Files"));
        pf.push(format!(r"{sd}\Program Files (x86)"));
        pf.sort_by_key(|s| s.to_lowercase());
        pf.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
        env.program_files = pf;
        env.program_data = var("ProgramData").unwrap_or_else(|| format!(r"{sd}\ProgramData"));
        let profile = var("USERPROFILE");
        let public = var("PUBLIC");
        let parent = |s: &str| s.rfind(['\\', '/']).map(|i| s[..i].to_string());
        env.users_dir = public
            .as_deref()
            .and_then(parent)
            .or_else(|| profile.as_deref().and_then(parent))
            .filter(|p| p.len() > 2)
            .unwrap_or_else(|| format!(r"{sd}\Users"));
        env.user_profiles = profile.into_iter().chain(public).collect();
        env.own_exe = std::env::current_exe()
            .ok()
            .map(|p| p.to_string_lossy().into_owned());
        env
    }
}

// ---------------------------------------------------------------------------
// Normalisierte Pfade
// ---------------------------------------------------------------------------

/// Normalisierter Pfad: Wurzel ("c:" oder "\\server\share") + kleingeschriebene Komponenten.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NPath {
    pub(crate) root: String,
    pub(crate) comps: Vec<String>,
}

impl NPath {
    fn is_drive(&self) -> bool {
        self.root.len() == 2 && self.root.ends_with(':')
    }

    /// `self` liegt in `base` oder ist `base`.
    fn under(&self, base: &NPath) -> bool {
        self.root == base.root && self.comps.starts_with(&base.comps)
    }

    fn same(&self, base: &NPath) -> bool {
        self.root == base.root && self.comps == base.comps
    }
}

/// Eine Pfadkomponente normalisieren; `None` = unzulässig.
pub(crate) fn norm_comp(c: &str) -> Option<String> {
    if c == "." || c == ".." {
        return None;
    }
    if c.chars().any(|ch| {
        (ch as u32) < 0x20 || matches!(ch, '<' | '>' | ':' | '"' | '|' | '?' | '*' | '\\' | '/')
    }) {
        return None;
    }
    let t = c.trim_end_matches(['.', ' ']);
    if t.is_empty() {
        return None;
    }
    Some(t.to_lowercase())
}

fn norm_comps(rest: &str) -> Option<Vec<String>> {
    rest.split('\\')
        .filter(|c| !c.is_empty())
        .map(norm_comp)
        .collect()
}

/// Ohne Kontext (Systemlaufwerk, Windows-Ordner) normalisieren.
fn normalize_with(path: &str, sys_root: &str, windows: Option<&NPath>) -> Option<NPath> {
    let mut p = path.trim().replace('/', "\\");
    for pre in [r"\\?\", r"\\.\", r"\??\"] {
        if p.starts_with(pre) {
            let rest = &p[pre.len()..];
            p = if rest.len() >= 4 && rest[..4].eq_ignore_ascii_case(r"UNC\") {
                format!(r"\\{}", &rest[4..])
            } else {
                rest.to_string()
            };
            break;
        }
    }
    if let Some(rest) = p.strip_prefix(r"\\") {
        let mut it = rest.split('\\').filter(|c| !c.is_empty());
        let server = norm_comp(it.next()?)?;
        let Some(share) = it.next() else {
            return Some(NPath {
                root: format!(r"\\{server}"),
                comps: Vec::new(),
            });
        };
        let share = norm_comp(share)?;
        let tail: Vec<String> = it.map(norm_comp).collect::<Option<_>>()?;
        let sb = share.as_bytes();
        if sb.len() == 2 && sb[0].is_ascii_alphabetic() && sb[1] == b'$' {
            // Administrative Freigabe c$ = Laufwerk C:
            return Some(NPath {
                root: format!("{}:", sb[0] as char),
                comps: tail,
            });
        }
        if share == "admin$" {
            let w = windows?;
            let mut comps = w.comps.clone();
            comps.extend(tail);
            return Some(NPath {
                root: w.root.clone(),
                comps,
            });
        }
        return Some(NPath {
            root: format!(r"\\{server}\{share}"),
            comps: tail,
        });
    }
    let b = p.as_bytes();
    if b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':' {
        if b.len() > 2 && b[2] != b'\\' {
            return None; // "C:relativ"
        }
        return Some(NPath {
            root: p[..2].to_ascii_lowercase(),
            comps: norm_comps(&p[2..])?,
        });
    }
    if p.starts_with('\\') {
        return Some(NPath {
            root: sys_root.to_string(),
            comps: norm_comps(&p)?,
        });
    }
    None
}

/// Vorbereitete Umgebung (einmal normalisiert).
pub(crate) struct NormEnv {
    sys_root: String,
    windows: Option<NPath>,
    program_files: Vec<NPath>,
    program_data: Option<NPath>,
    users: Option<NPath>,
    profiles: Vec<NPath>,
    own_exe: Option<NPath>,
}

impl NormEnv {
    pub(crate) fn new(env: &ProtectEnv) -> Self {
        let sys = env.system_drive.trim().trim_end_matches(['\\', '/']);
        let sb = sys.as_bytes();
        let sys_root = if sb.len() == 2 && sb[0].is_ascii_alphabetic() && sb[1] == b':' {
            sys.to_ascii_lowercase()
        } else {
            "c:".to_string()
        };
        let n = |s: &str| normalize_with(s, &sys_root, None).filter(|p| !p.comps.is_empty());
        let windows = n(&env.windows_dir);
        let nn = |s: &str| {
            normalize_with(s, &sys_root, windows.as_ref()).filter(|p| !p.comps.is_empty())
        };
        NormEnv {
            program_files: env.program_files.iter().filter_map(|s| nn(s)).collect(),
            program_data: nn(&env.program_data),
            users: nn(&env.users_dir),
            profiles: env.user_profiles.iter().filter_map(|s| nn(s)).collect(),
            own_exe: env.own_exe.as_deref().and_then(nn),
            sys_root,
            windows,
        }
    }

    pub(crate) fn normalize(&self, path: &str) -> Option<NPath> {
        normalize_with(path, &self.sys_root, self.windows.as_ref())
    }

    /// Index der ersten Komponente nach der Profil-Wurzel, falls `p` in einem Profil liegt
    /// (`p.comps.len() == idx` heißt: `p` ist die Profil-Wurzel).
    pub(crate) fn profile_start(&self, p: &NPath) -> Option<usize> {
        let mut best: Option<usize> = None;
        if let Some(u) = &self.users
            && p.under(u)
            && p.comps.len() > u.comps.len()
        {
            best = Some(u.comps.len() + 1);
        }
        for pr in &self.profiles {
            if p.under(pr) && best.is_none_or(|b| pr.comps.len() > b) {
                best = Some(pr.comps.len());
            }
        }
        best
    }

    /// Profil-relative Komponenten (leer = Profil-Wurzel).
    pub(crate) fn profile_rel<'a>(&self, p: &'a NPath) -> Option<&'a [String]> {
        self.profile_start(p).map(|i| &p.comps[i..])
    }
}

const ROOT_SYSTEM_DIRS: &[&str] = &[
    "system volume information",
    "recovery",
    "boot",
    "efi",
    "config.msi",
    "documents and settings",
];
const ROOT_SYSTEM_FILES: &[&str] = &[
    "pagefile.sys",
    "hiberfil.sys",
    "swapfile.sys",
    "bootmgr",
    "bootnxt",
    "bootsect.bak",
];
const LIBRARIES: &[&str] = &[
    "documents",
    "desktop",
    "pictures",
    "videos",
    "music",
    "onedrive",
    "dokumente",
    "bilder",
    "musik",
];
const FOREIGN_SYSTEM: &[&str] = &[
    "windows",
    "program files",
    "program files (x86)",
    "programdata",
];

/// 8.3-Kurzname wie `progra~1` oder `longfi~1.txt`.
fn is_short_name(c: &str) -> bool {
    let stem = match c.rsplit_once('.') {
        Some((s, e)) if (1..=3).contains(&e.len()) && !e.contains('~') => s,
        _ => c,
    };
    let Some((head, digits)) = stem.rsplit_once('~') else {
        return false;
    };
    let hl = head.chars().count();
    (1..=8).contains(&hl)
        && !head.contains(['.', ' '])
        && !digits.is_empty()
        && digits.bytes().all(|b| b.is_ascii_digit())
}

fn is_library(name: &str) -> bool {
    LIBRARIES.contains(&name) || name.starts_with("onedrive - ")
}

/// Kern der Bewertung auf einem normalisierten Pfad.
pub(crate) fn eval(p: &NPath, flags: NodeFlags, is_dir: bool, env: &NormEnv) -> Protection {
    use Protection::{Blocked, Caution};
    let c = &p.comps;
    let Some(first) = c.first() else {
        return Blocked(Reason::VolumeRoot);
    };
    if c.iter().take(3).any(|x| is_short_name(x)) {
        return Blocked(Reason::ShortName);
    }
    if env.windows.as_ref().is_some_and(|w| p.under(w)) {
        return Blocked(Reason::WindowsDir);
    }
    if env.program_files.iter().any(|b| p.under(b)) {
        return Blocked(Reason::ProgramFiles);
    }
    let in_pd = env.program_data.as_ref().filter(|pd| p.under(pd));
    if let Some(pd) = in_pd {
        if c.len() == pd.comps.len() {
            return Blocked(Reason::ProgramDataRoot);
        }
        if c[pd.comps.len()] == "microsoft" {
            return Blocked(Reason::ProgramDataMicrosoft);
        }
    }
    if p.is_drive() {
        if first.starts_with('$') {
            return if c.len() == 1 && !is_dir {
                Blocked(Reason::SystemFile)
            } else {
                Blocked(Reason::SystemFolder)
            };
        }
        if ROOT_SYSTEM_DIRS.contains(&first.as_str()) {
            return Blocked(Reason::SystemFolder);
        }
        if c.len() == 1 && ROOT_SYSTEM_FILES.contains(&first.as_str()) {
            return Blocked(Reason::SystemFile);
        }
    }
    if env.users.as_ref().is_some_and(|u| p.same(u)) {
        return Blocked(Reason::UsersDir);
    }
    let rel = env.profile_rel(p);
    if let Some(rel) = rel {
        if rel.is_empty() {
            return Blocked(Reason::ProfileRoot);
        }
        let last = c.last().map(String::as_str).unwrap_or("");
        if last.starts_with("ntuser.dat") || last.starts_with("usrclass.dat") {
            return Blocked(Reason::RegistryHive);
        }
        let r: Vec<&str> = rel.iter().map(String::as_str).collect();
        match r.as_slice() {
            ["appdata"]
            | ["appdata", "local" | "roaming" | "locallow"]
            | ["appdata", "local", "temp"] => return Blocked(Reason::AppDataRoot),
            ["appdata", "local", "microsoft", "windows", ..] => {
                return Blocked(Reason::ProfileSystem);
            }
            _ => {}
        }
    }
    if let Some(exe) = &env.own_exe
        && exe.under(p)
    {
        // exe selbst, ihr Ordner und alle Ordner darüber
        return Blocked(Reason::OwnProgram);
    }
    if flags.contains(NodeFlags::SYSTEM) {
        return Blocked(Reason::SystemFlag);
    }

    // ---- Caution ----
    if flags.contains(NodeFlags::REPARSE) {
        return Caution(Reason::ReparsePoint);
    }
    if p.is_drive() && first == "windows.old" {
        return Caution(Reason::WindowsOld);
    }
    if let Some(rel) = rel {
        if rel[0] == "appdata" {
            let in_temp = rel.len() > 3 && rel[1] == "local" && rel[2] == "temp";
            if !in_temp {
                return Caution(Reason::AppData);
            }
        } else if is_library(&rel[0]) {
            return Caution(Reason::UserLibrary);
        }
    }
    if in_pd.is_some() {
        return Caution(Reason::ProgramDataApp);
    }
    if p.is_drive() && FOREIGN_SYSTEM.contains(&first.as_str()) {
        return Caution(Reason::ForeignSystem);
    }
    if flags.contains(NodeFlags::HIDDEN) && !c.iter().any(|x| x == "temp" || x == "tmp") {
        return Caution(Reason::Hidden);
    }
    Protection::Free
}

/// Bewertet einen Pfad. `flags` sind die Attribute des Elements (SYSTEM, HIDDEN, REPARSE);
/// `is_dir` unterscheidet nur Systemdatei/-ordner im Grund – gesperrt wird unabhängig davon.
pub fn classify_path(path: &str, flags: NodeFlags, is_dir: bool, env: &ProtectEnv) -> Protection {
    let ne = NormEnv::new(env);
    classify_path_norm(path, flags, is_dir, &ne)
}

pub(crate) fn classify_path_norm(
    path: &str,
    flags: NodeFlags,
    is_dir: bool,
    env: &NormEnv,
) -> Protection {
    match env.normalize(path) {
        Some(p) => eval(&p, flags, is_dir, env),
        None => Protection::Blocked(Reason::InvalidPath),
    }
}

/// SYSTEM und HIDDEN vererben sich auf Nachkommen.
const INHERITED_FLAGS: NodeFlags = NodeFlags::SYSTEM.union(NodeFlags::HIDDEN);

fn ancestor_flags(tree: &Tree, id: NodeId) -> NodeFlags {
    let mut f = NodeFlags::empty();
    let mut cur = tree.node(id).parent;
    while cur != NO_NODE {
        f |= tree.node(cur).flags & INHERITED_FLAGS;
        cur = tree.node(cur).parent;
    }
    f
}

/// Bewertet einen Knoten des Baums (voller Pfad, eigene Flags plus vererbte SYSTEM/HIDDEN).
pub fn classify(tree: &Tree, id: NodeId, env: &ProtectEnv) -> Protection {
    let n = tree.node(id);
    classify_path(
        &tree.full_path(id),
        n.flags | ancestor_flags(tree, id),
        n.is_dir(),
        env,
    )
}

/// Was der Besucher von [`walk`] nach einem Knoten will.
pub(crate) enum Visit {
    Descend,
    Skip,
}

/// Tiefensuche ab `start` mit inkrementell gepflegtem Pfad und Schutzstufe je Knoten.
/// Unter einem vererbend blockierten Knoten wird nicht neu bewertet (gleiche Stufe).
pub(crate) fn walk<B>(
    tree: &Tree,
    start: NodeId,
    env: &NormEnv,
    mut f: impl FnMut(NodeId, &NPath, Protection) -> ControlFlow<B, Visit>,
) -> Option<B> {
    let sn = tree.node(start);
    let start_flags = sn.flags | ancestor_flags(tree, start);
    let (mut path, start_prot) = match env.normalize(&tree.full_path(start)) {
        Some(p) => {
            let prot = eval(&p, start_flags, sn.is_dir(), env);
            (p, prot)
        }
        None => (
            NPath {
                root: String::new(),
                comps: Vec::new(),
            },
            Protection::Blocked(Reason::InvalidPath),
        ),
    };
    let base = path.comps.len();
    // (Knoten, Tiefe, vererbte Flags, vererbter Sperrgrund)
    let mut stack: Vec<(NodeId, usize, NodeFlags, Option<Reason>)> = Vec::new();
    let mut visit = |id: NodeId,
                     depth: usize,
                     prot: Protection,
                     flags: NodeFlags,
                     path: &NPath,
                     stack: &mut Vec<(NodeId, usize, NodeFlags, Option<Reason>)>|
     -> Option<B> {
        match f(id, path, prot) {
            ControlFlow::Break(b) => return Some(b),
            ControlFlow::Continue(Visit::Skip) => {}
            ControlFlow::Continue(Visit::Descend) => {
                let inh = match prot {
                    Protection::Blocked(r) if r.is_inherited() => Some(r),
                    _ => None,
                };
                let cf = (flags | tree.node(id).flags) & INHERITED_FLAGS;
                let kids: Vec<NodeId> = tree.children(id).collect();
                for &k in kids.iter().rev() {
                    stack.push((k, depth + 1, cf, inh));
                }
            }
        }
        None
    };
    if let Some(b) = visit(start, 0, start_prot, start_flags, &path, &mut stack) {
        return Some(b);
    }
    while let Some((id, depth, flags, inh)) = stack.pop() {
        let n = tree.node(id);
        path.comps.truncate(base + depth - 1);
        let comp = norm_comp(&n.name);
        path.comps
            .push(comp.clone().unwrap_or_else(|| n.name.to_lowercase()));
        let prot = match (inh, comp) {
            (Some(r), _) => Protection::Blocked(r),
            (None, None) => Protection::Blocked(Reason::InvalidPath),
            (None, Some(_)) => eval(&path, n.flags | flags, n.is_dir(), env),
        };
        if let Some(b) = visit(id, depth, prot, flags, &path, &mut stack) {
            return Some(b);
        }
    }
    None
}

/// Enthält der Teilbaum ab `id` (inklusive `id`) ein blockiertes Element?
/// Für die GUI-Warnung beim Löschen eines Ordners. Bricht beim ersten Treffer ab.
pub fn contains_blocked(tree: &Tree, id: NodeId, env: &ProtectEnv) -> bool {
    contains_blocked_norm(tree, id, &NormEnv::new(env))
}

pub(crate) fn contains_blocked_norm(tree: &Tree, id: NodeId, env: &NormEnv) -> bool {
    walk(tree, id, env, |_, _, prot| {
        if prot.is_blocked() {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(Visit::Descend)
        }
    })
    .is_some()
}

// ---------------------------------------------------------------------------
// Fehler für den Löschweg
// ---------------------------------------------------------------------------

/// Nutzlast des `PermissionDenied`-Fehlers aus `platform::guard_paths`.
/// Die GUI kann sie per `err.get_ref().and_then(|e| e.downcast_ref::<ProtectError>())`
/// auslesen und in ihrer Sprache anzeigen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProtectError {
    /// Erster geschützter Pfad.
    pub path: String,
    pub reason: Reason,
    /// Anzahl aller geschützten Pfade im Auftrag.
    pub blocked_count: usize,
}

impl ProtectError {
    pub fn message(&self, lang: Lang) -> String {
        let mut s = tr(lang, Key::ErrGuardBlocked)
            .replace("{path}", &self.path)
            .replace("{reason}", self.reason.text(lang));
        if self.blocked_count > 1 {
            s.push(' ');
            s.push_str(
                &tr(lang, Key::ErrGuardBlockedMore)
                    .replace("{n}", &(self.blocked_count - 1).to_string()),
            );
        }
        s
    }
}

impl std::fmt::Display for ProtectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message(Lang::De))
    }
}

impl std::error::Error for ProtectError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{EntryInfo, ScanMeta, ScanMode, TreeBuilder};

    fn env() -> ProtectEnv {
        ProtectEnv {
            user_profiles: vec![r"C:\Users\joschy".into()],
            own_exe: Some(r"C:\Tools\JT\JoschysTree.exe".into()),
            ..ProtectEnv::windows_default()
        }
    }

    fn cp(p: &str) -> Protection {
        classify_path(p, NodeFlags::empty(), false, &env())
    }

    fn cd(p: &str) -> Protection {
        classify_path(p, NodeFlags::DIR, true, &env())
    }

    fn blocked(p: &str, r: Reason) {
        assert_eq!(cp(p), Protection::Blocked(r), "{p}");
    }

    fn caution(p: &str, r: Reason) {
        assert_eq!(cp(p), Protection::Caution(r), "{p}");
    }

    fn free(p: &str) {
        assert_eq!(cp(p), Protection::Free, "{p}");
    }

    #[test]
    fn volume_and_share_roots() {
        blocked(r"C:\", Reason::VolumeRoot);
        blocked("C:", Reason::VolumeRoot);
        blocked("D:/", Reason::VolumeRoot);
        blocked(r"\\nas\daten", Reason::VolumeRoot);
        blocked(r"\\nas\daten\\", Reason::VolumeRoot);
        blocked(r"\\nas", Reason::VolumeRoot);
        blocked(r"\\?\UNC\nas\daten", Reason::VolumeRoot);
        blocked(r"\\?\Z:\", Reason::VolumeRoot);
        blocked(r"\", Reason::VolumeRoot);
    }

    #[test]
    fn windows_dir_and_children() {
        blocked(r"C:\Windows", Reason::WindowsDir);
        blocked(r"C:\Windows\System32\drivers\etc\hosts", Reason::WindowsDir);
        blocked(r"C:\Windows\WinSxS", Reason::WindowsDir);
        blocked(r"C:\Windows\Temp\x.tmp", Reason::WindowsDir);
        blocked(r"c:\WINDOWS\system32", Reason::WindowsDir);
        blocked("c:/windows/system32/", Reason::WindowsDir);
        blocked(r"\\?\C:\Windows\explorer.exe", Reason::WindowsDir);
        blocked(r"\Windows\notepad.exe", Reason::WindowsDir);
        blocked(r"C:\Windows.\System32", Reason::WindowsDir);
        blocked(r"C:\Windows \System32", Reason::WindowsDir);
        blocked(r"C:\\Windows\\\System32", Reason::WindowsDir);
    }

    #[test]
    fn admin_shares_resolve() {
        blocked(r"\\localhost\c$\Windows\System32", Reason::WindowsDir);
        blocked(
            r"\\?\UNC\127.0.0.1\C$\Program Files\x",
            Reason::ProgramFiles,
        );
        blocked(r"\\srv\admin$\System32", Reason::WindowsDir);
        blocked(r"\\srv\c$", Reason::VolumeRoot);
        free(r"\\srv\c$\Backup\a.zip");
    }

    #[test]
    fn program_files_and_data() {
        blocked(r"C:\Program Files", Reason::ProgramFiles);
        blocked(
            r"C:\Program Files\Mozilla Firefox\firefox.exe",
            Reason::ProgramFiles,
        );
        blocked(r"C:\Program Files (x86)\Steam", Reason::ProgramFiles);
        blocked(r"C:\ProgramData", Reason::ProgramDataRoot);
        blocked(r"C:\ProgramData\Microsoft", Reason::ProgramDataMicrosoft);
        blocked(
            r"C:\ProgramData\Microsoft\Windows\Start Menu",
            Reason::ProgramDataMicrosoft,
        );
        caution(r"C:\ProgramData\Package Cache", Reason::ProgramDataApp);
        caution(r"C:\ProgramData\Adobe\x\y.dat", Reason::ProgramDataApp);
    }

    #[test]
    fn root_system_items() {
        for p in [
            r"C:\System Volume Information",
            r"C:\$Recycle.Bin",
            r"C:\$Recycle.Bin\S-1-5-21\$R123.zip",
            r"C:\$WinREAgent",
            r"C:\Recovery",
            r"C:\Boot\BCD",
            r"C:\EFI",
            r"C:\Config.Msi",
            r"C:\Documents and Settings",
            r"E:\System Volume Information\x",
        ] {
            assert_eq!(cd(p), Protection::Blocked(Reason::SystemFolder), "{p}");
        }
        for p in [
            r"C:\$MFT",
            r"C:\$LogFile",
            r"C:\$Bitmap",
            r"C:\pagefile.sys",
            r"C:\hiberfil.sys",
            r"C:\swapfile.sys",
            r"C:\bootmgr",
            r"C:\BOOTNXT",
            r"D:\pagefile.sys",
        ] {
            blocked(p, Reason::SystemFile);
        }
        // nur in der Laufwerkswurzel
        free(r"C:\Backup\pagefile.sys");
        free(r"C:\Backup\Boot");
        assert_eq!(cd(r"C:\$Extend"), Protection::Blocked(Reason::SystemFolder));
        caution(r"C:\Windows.old", Reason::WindowsOld);
        caution(r"C:\Windows.old\Windows\System32\x.dll", Reason::WindowsOld);
    }

    #[test]
    fn users_and_profiles() {
        blocked(r"C:\Users", Reason::UsersDir);
        for p in [
            r"C:\Users\joschy",
            r"C:\Users\Default",
            r"C:\Users\Public",
            r"C:\Users\All Users",
            r"C:\Users\Default User",
            r"C:\Users\anna",
        ] {
            blocked(p, Reason::ProfileRoot);
        }
        blocked(r"C:\Users\joschy\NTUSER.DAT", Reason::RegistryHive);
        blocked(r"C:\Users\joschy\ntuser.dat.LOG1", Reason::RegistryHive);
        blocked(
            r"C:\Users\anna\AppData\Local\Microsoft\Windows\UsrClass.dat",
            Reason::RegistryHive,
        );
        blocked(r"C:\Users\anna\Backup\UsrClass.dat", Reason::RegistryHive);
        for p in [
            r"C:\Users\joschy\AppData",
            r"C:\Users\joschy\AppData\Local",
            r"C:\Users\joschy\AppData\Roaming",
            r"C:\Users\joschy\AppData\LocalLow",
            r"C:\Users\joschy\AppData\Local\Temp",
        ] {
            blocked(p, Reason::AppDataRoot);
        }
        blocked(
            r"C:\Users\joschy\AppData\Local\Microsoft\Windows\INetCache\x",
            Reason::ProfileSystem,
        );
        caution(
            r"C:\Users\joschy\AppData\Local\Google\Chrome\User Data",
            Reason::AppData,
        );
        caution(r"C:\Users\joschy\AppData\Roaming\Mozilla", Reason::AppData);
        free(r"C:\Users\joschy\AppData\Local\Temp\abc.tmp");
        free(r"C:\Users\joschy\AppData\Local\Temp\sub\x.log");
        free(r"C:\Users\joschy\Downloads\setup.exe");
        free(r"C:\Users\joschy\Downloads");
    }

    #[test]
    fn libraries_are_caution() {
        for p in [
            r"C:\Users\joschy\Documents",
            r"C:\Users\joschy\Documents\Steuer\2025.pdf",
            r"C:\Users\joschy\Desktop\x.lnk",
            r"C:\Users\joschy\Pictures",
            r"C:\Users\joschy\Videos\film.mp4",
            r"C:\Users\joschy\Music",
            r"C:\Users\joschy\OneDrive",
            r"C:\Users\joschy\OneDrive - Firma\a.docx",
            r"C:\Users\Public\Documents",
        ] {
            caution(p, Reason::UserLibrary);
        }
        free(r"C:\Backup\Documents");
    }

    #[test]
    fn flags_rules() {
        let e = env();
        assert_eq!(
            classify_path(r"C:\Backup\desktop.ini", NodeFlags::SYSTEM, false, &e),
            Protection::Blocked(Reason::SystemFlag)
        );
        assert_eq!(
            classify_path(r"D:\Link", NodeFlags::REPARSE | NodeFlags::DIR, true, &e),
            Protection::Caution(Reason::ReparsePoint)
        );
        assert_eq!(
            classify_path(r"D:\Daten\.git", NodeFlags::HIDDEN, true, &e),
            Protection::Caution(Reason::Hidden)
        );
        assert_eq!(
            classify_path(r"D:\Temp\~x.tmp", NodeFlags::HIDDEN, false, &e),
            Protection::Free
        );
        // Blocked schlägt Caution
        assert_eq!(
            classify_path(r"C:\Windows\x", NodeFlags::REPARSE, false, &e),
            Protection::Blocked(Reason::WindowsDir)
        );
    }

    #[test]
    fn invalid_paths() {
        for p in [
            r"C:\Users\joschy\Downloads\..\..\..\Windows",
            r"C:\Backup\..",
            r"C:\Backup\.\x",
            r"Windows\System32",
            "relativ.txt",
            "C:Windows",
            "",
            r"C:\Backup\a|b",
            r"C:\Backup\a?b",
            "C:\\Backup\\a\u{1}b",
            r"C:\pagefile.sys::$DATA",
            r"\\?\Volume{1234}\x",
            r"\\?\GLOBALROOT\Device\HarddiskVolume1",
            r"C:\Backup\...",
        ] {
            blocked(p, Reason::InvalidPath);
        }
    }

    #[test]
    fn short_names() {
        blocked(r"C:\PROGRA~1", Reason::ShortName);
        blocked(r"C:\PROGRA~2\Steam", Reason::ShortName);
        blocked(r"C:\Users\JOSCHY~1\AppData", Reason::ShortName);
        blocked(r"C:\a\b\LONGFI~1.TXT", Reason::ShortName);
        // ab Ebene 4 nicht mehr
        free(r"C:\a\b\c\FOO~1");
        free(r"C:\Backup\~$brief.docx");
        free(r"C:\Backup\a~b");
    }

    #[test]
    fn own_program() {
        blocked(r"C:\Tools\JT\JoschysTree.exe", Reason::OwnProgram);
        blocked(r"C:\Tools\JT", Reason::OwnProgram);
        blocked(r"c:/tools", Reason::OwnProgram);
        free(r"C:\Tools\JT\settings.json");
        free(r"C:\Tools\Andere");
    }

    #[test]
    fn other_drives_and_spaces() {
        free(r"C:\Backup");
        free(r"C:\Backup\alte Fotos 2019\IMG 1.jpg");
        free(r"D:\Filme\x.mkv");
        free(r"\\nas\daten\x\y");
        caution(r"D:\Windows", Reason::ForeignSystem);
        caution(r"E:\Program Files\x", Reason::ForeignSystem);
        free(r"D:\Users\x");
    }

    #[test]
    fn system_drive_d() {
        let e = ProtectEnv {
            system_drive: "D:".into(),
            windows_dir: r"D:\Windows".into(),
            program_files: vec![r"D:\Program Files".into(), r"D:\Program Files (x86)".into()],
            program_data: r"D:\ProgramData".into(),
            users_dir: r"D:\Users".into(),
            user_profiles: vec![],
            own_exe: None,
        };
        let c = |p: &str| classify_path(p, NodeFlags::empty(), false, &e);
        assert_eq!(
            c(r"D:\Windows\System32"),
            Protection::Blocked(Reason::WindowsDir)
        );
        assert_eq!(c(r"\Windows\x"), Protection::Blocked(Reason::WindowsDir));
        assert_eq!(
            c(r"D:\Program Files\x"),
            Protection::Blocked(Reason::ProgramFiles)
        );
        assert_eq!(
            c(r"D:\Users\joschy"),
            Protection::Blocked(Reason::ProfileRoot)
        );
        assert_eq!(c(r"C:\Windows"), Protection::Caution(Reason::ForeignSystem));
        assert_eq!(c(r"C:\Users\joschy"), Protection::Free);
        assert_eq!(
            c(r"C:\pagefile.sys"),
            Protection::Blocked(Reason::SystemFile)
        );
        assert_eq!(
            c(r"\\srv\admin$\x"),
            Protection::Blocked(Reason::WindowsDir)
        );
    }

    #[test]
    fn defaults() {
        let d = ProtectEnv::windows_default();
        assert_eq!(d.system_drive, "C:");
        assert_eq!(d.windows_dir, r"C:\Windows");
        assert_eq!(d.program_files.len(), 2);
        assert_eq!(d.own_exe, None);
        let s = ProtectEnv::from_system();
        if !cfg!(windows) {
            assert_eq!(s, d);
        } else {
            assert!(s.own_exe.is_some());
            let w = s.windows_dir.clone();
            assert!(classify_path(&w, NodeFlags::DIR, true, &s).is_blocked());
        }
        // kaputte Umgebung: kein Absturz, Laufwerkswurzeln bleiben gesperrt
        let broken = ProtectEnv {
            system_drive: "?".into(),
            windows_dir: "".into(),
            program_files: vec!["relativ".into()],
            program_data: "".into(),
            users_dir: "".into(),
            user_profiles: vec![],
            own_exe: Some("x".into()),
        };
        assert!(classify_path(r"C:\", NodeFlags::empty(), true, &broken).is_blocked());
        assert!(classify_path(r"\\?\C:\$MFT", NodeFlags::empty(), false, &broken).is_blocked());
    }

    #[test]
    fn parent_vs_child_and_tree() {
        let mut b = TreeBuilder::new(ScanMeta::new("C:\\", ScanMode::Walk), "C:\\");
        let dir = EntryInfo::default();
        let backup = b.add_dir(0, "Backup", dir.clone());
        let inner_sys = b.add_dir(
            backup,
            "sys",
            EntryInfo {
                flags: NodeFlags::SYSTEM,
                ..Default::default()
            },
        );
        let under_sys = b.add_file(inner_sys, "a.txt", EntryInfo::default());
        let plain = b.add_dir(backup, "plain", dir.clone());
        let plain_f = b.add_file(plain, "b.txt", EntryInfo::default());
        let win = b.add_dir(0, "Windows", dir.clone());
        let win_f = b.add_file(win, "x.dll", EntryInfo::default());
        let hid = b.add_dir(
            0,
            "Versteckt",
            EntryInfo {
                flags: NodeFlags::HIDDEN,
                ..Default::default()
            },
        );
        let hid_f = b.add_file(hid, "c.txt", EntryInfo::default());
        let t = b.finish();
        let e = env();
        assert_eq!(classify(&t, 0, &e), Protection::Blocked(Reason::VolumeRoot));
        assert_eq!(classify(&t, backup, &e), Protection::Free);
        assert_eq!(
            classify(&t, inner_sys, &e),
            Protection::Blocked(Reason::SystemFlag)
        );
        assert_eq!(
            classify(&t, under_sys, &e),
            Protection::Blocked(Reason::SystemFlag)
        );
        assert_eq!(classify(&t, plain_f, &e), Protection::Free);
        assert_eq!(
            classify(&t, win_f, &e),
            Protection::Blocked(Reason::WindowsDir)
        );
        assert_eq!(classify(&t, hid_f, &e), Protection::Caution(Reason::Hidden));
        assert!(contains_blocked(&t, backup, &e));
        assert!(!contains_blocked(&t, plain, &e));
        assert!(contains_blocked(&t, 0, &e));
        assert!(contains_blocked(&t, win, &e));
        assert!(!contains_blocked(&t, hid, &e));
        // walk liefert dieselbe Blockiert-Aussage wie classify
        let ne = NormEnv::new(&e);
        let mut seen = 0;
        walk::<()>(&t, 0, &ne, |id, _, prot| {
            assert_eq!(prot.is_blocked(), classify(&t, id, &e).is_blocked(), "{id}");
            seen += 1;
            ControlFlow::Continue(Visit::Descend)
        });
        assert_eq!(seen, t.nodes.len());
    }

    #[test]
    fn subtree_root_scan() {
        // Scan-Wurzel ist ein Unterordner
        let mut b = TreeBuilder::new(
            ScanMeta::new(r"C:\Users\joschy", ScanMode::Walk),
            r"C:\Users\joschy",
        );
        let ad = b.add_dir(0, "AppData", EntryInfo::default());
        let dl = b.add_dir(0, "Downloads", EntryInfo::default());
        let f = b.add_file(dl, "x.iso", EntryInfo::default());
        let t = b.finish();
        let e = env();
        assert_eq!(
            classify(&t, 0, &e),
            Protection::Blocked(Reason::ProfileRoot)
        );
        assert_eq!(
            classify(&t, ad, &e),
            Protection::Blocked(Reason::AppDataRoot)
        );
        assert_eq!(classify(&t, f, &e), Protection::Free);
    }

    #[test]
    fn i18n_for_every_reason_and_level() {
        for r in Reason::ALL {
            for lang in Lang::ALL {
                assert!(!r.text(lang).trim().is_empty(), "{r:?} {lang:?}");
            }
        }
        for p in [
            Protection::Free,
            Protection::Caution(Reason::Hidden),
            Protection::Blocked(Reason::WindowsDir),
        ] {
            for lang in Lang::ALL {
                assert!(!tr(lang, p.level_key()).trim().is_empty());
            }
        }
        let e = ProtectError {
            path: r"C:\Windows".into(),
            reason: Reason::WindowsDir,
            blocked_count: 3,
        };
        let m = e.message(Lang::En);
        assert!(m.contains(r"C:\Windows") && m.contains('2'), "{m}");
        assert!(!e.to_string().is_empty());
    }
}
