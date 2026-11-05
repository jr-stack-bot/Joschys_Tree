// Joschys_Tree – Aufräum-Assistent: Kandidaten für den Auswahlkorb finden – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Aufräum-Assistent (reine Logik). Ein Durchlauf über den Baum ordnet jedes
//! Element höchstens **einer** Kategorie zu (keine Doppelzählung), in dieser
//! Vorrang-Reihenfolge: UserTemp > BrowserCache > CrashDumps > OldInstallers >
//! OldDownloads > TempLikeFiles > LargeOldFiles > EmptyFolders. Ein als Kandidat
//! gewählter Ordner deckt seinen ganzen Teilbaum ab (es werden nur oberste
//! Elemente gemeldet).
//!
//! Sicherheit: Kein Kandidat ist `Blocked`; Ordner-Kandidaten enthalten auch
//! keine blockierten Nachkommen. Elemente mit ACCESS_DENIED werden nie
//! vorgeschlagen (Inhalt unbekannt). Info-Gruppen (`info_only`) zeigen auf
//! geschützte Elemente (hiberfil.sys, pagefile.sys, Windows.old, $Recycle.Bin);
//! die GUI bietet dort kein Löschen an, sondern einen Hinweis.

use std::ops::ControlFlow;

use crate::i18n::Key;
use crate::model::{FileTime, NodeFlags, NodeId, Tree};
use crate::protect::{
    NPath, NormEnv, ProtectEnv, Protection, Reason, Visit, contains_blocked_norm, walk,
};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum CleanupKind {
    UserTemp,
    BrowserCache,
    CrashDumps,
    OldDownloads,
    OldInstallers,
    LargeOldFiles,
    TempLikeFiles,
    EmptyFolders,
    InfoHibernation,
    InfoPagefile,
    InfoWindowsOld,
    InfoRecycleBin,
}

impl CleanupKind {
    pub const ALL: [CleanupKind; 12] = [
        CleanupKind::UserTemp,
        CleanupKind::BrowserCache,
        CleanupKind::CrashDumps,
        CleanupKind::OldDownloads,
        CleanupKind::OldInstallers,
        CleanupKind::LargeOldFiles,
        CleanupKind::TempLikeFiles,
        CleanupKind::EmptyFolders,
        CleanupKind::InfoHibernation,
        CleanupKind::InfoPagefile,
        CleanupKind::InfoWindowsOld,
        CleanupKind::InfoRecycleBin,
    ];

    /// Nur Hinweis, kein Löschen über Joschys_Tree.
    pub fn is_info(self) -> bool {
        matches!(
            self,
            CleanupKind::InfoHibernation
                | CleanupKind::InfoPagefile
                | CleanupKind::InfoWindowsOld
                | CleanupKind::InfoRecycleBin
        )
    }

    /// Titel der Gruppe.
    pub fn title_key(self) -> Key {
        match self {
            CleanupKind::UserTemp => Key::CleanUserTemp,
            CleanupKind::BrowserCache => Key::CleanBrowserCache,
            CleanupKind::CrashDumps => Key::CleanCrashDumps,
            CleanupKind::OldDownloads => Key::CleanOldDownloads,
            CleanupKind::OldInstallers => Key::CleanOldInstallers,
            CleanupKind::LargeOldFiles => Key::CleanLargeOldFiles,
            CleanupKind::TempLikeFiles => Key::CleanTempLikeFiles,
            CleanupKind::EmptyFolders => Key::CleanEmptyFolders,
            CleanupKind::InfoHibernation => Key::CleanInfoHibernation,
            CleanupKind::InfoPagefile => Key::CleanInfoPagefile,
            CleanupKind::InfoWindowsOld => Key::CleanInfoWindowsOld,
            CleanupKind::InfoRecycleBin => Key::CleanInfoRecycleBin,
        }
    }

    /// Ein-Satz-Hinweis (bei Info-Gruppen: was der Nutzer stattdessen tun kann).
    pub fn hint_key(self) -> Key {
        match self {
            CleanupKind::UserTemp => Key::CleanUserTempHint,
            CleanupKind::BrowserCache => Key::CleanBrowserCacheHint,
            CleanupKind::CrashDumps => Key::CleanCrashDumpsHint,
            CleanupKind::OldDownloads => Key::CleanOldDownloadsHint,
            CleanupKind::OldInstallers => Key::CleanOldInstallersHint,
            CleanupKind::LargeOldFiles => Key::CleanLargeOldFilesHint,
            CleanupKind::TempLikeFiles => Key::CleanTempLikeFilesHint,
            CleanupKind::EmptyFolders => Key::CleanEmptyFoldersHint,
            CleanupKind::InfoHibernation => Key::CleanInfoHibernationHint,
            CleanupKind::InfoPagefile => Key::CleanInfoPagefileHint,
            CleanupKind::InfoWindowsOld => Key::CleanInfoWindowsOldHint,
            CleanupKind::InfoRecycleBin => Key::CleanInfoRecycleBinHint,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CleanupGroup {
    pub kind: CleanupKind,
    /// Oberste Elemente, keine verschachtelten Doppelten.
    pub nodes: Vec<NodeId>,
    pub size: u64,
    pub allocated: u64,
    /// Anzahl Dateien (bei EmptyFolders: Anzahl Ordner inkl. leerer Unterordner).
    pub count: u64,
    pub info_only: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CleanupOptions {
    pub temp_min_age_days: u32,
    pub downloads_min_age_days: u32,
    pub installer_min_age_days: u32,
    pub large_min_bytes: u64,
    pub large_min_age_days: u32,
}

impl Default for CleanupOptions {
    fn default() -> Self {
        CleanupOptions {
            temp_min_age_days: 2,
            downloads_min_age_days: 180,
            installer_min_age_days: 90,
            large_min_bytes: 1 << 30,
            large_min_age_days: 365,
        }
    }
}

const MIB: u64 = 1 << 20;
const DAY_TICKS: i64 = 86_400 * 10_000_000;
const INSTALLER_EXTS: &[&str] = &["iso", "img", "msi", "exe", "msix", "zip", "7z", "rar"];
const TEMP_LIKE_EXTS: &[&str] = &["tmp", "log", "bak", "old", "chk", "dmp"];
const CACHE_DIRS: &[&str] = &["cache", "code cache", "gpucache"];

/// `t` ist bekannt und älter als `days` Tage vor `now`.
fn older_than(t: FileTime, now: FileTime, days: u32) -> bool {
    t.0 > 0 && now.0.saturating_sub(t.0) > i64::from(days) * DAY_TICKS
}

/// Jüngstes Änderungsdatum im Teilbaum (0 = irgendwo unbekannt).
fn newest_modified(tree: &Tree, id: NodeId) -> FileTime {
    let mut newest = tree.node(id).modified;
    let mut st = vec![id];
    while let Some(x) = st.pop() {
        let m = tree.node(x).modified;
        if m.0 == 0 {
            return FileTime(0);
        }
        newest = newest.max(m);
        st.extend(tree.children(x));
    }
    newest
}

/// Leerer Ordner: keine Dateien, Größe 0, und weder er noch ein Nachkomme ist
/// Link oder unlesbar (sonst wäre "leer" nur scheinbar).
fn truly_empty(tree: &Tree, id: NodeId) -> bool {
    let n = tree.node(id);
    if !n.is_dir() || n.file_count != 0 || n.size != 0 {
        return false;
    }
    let mut st = vec![id];
    while let Some(x) = st.pop() {
        let f = tree.node(x).flags;
        if !f.contains(NodeFlags::DIR)
            || f.intersects(NodeFlags::REPARSE | NodeFlags::ACCESS_DENIED | NodeFlags::OFFLINE)
        {
            return false;
        }
        st.extend(tree.children(x));
    }
    true
}

fn rel_str(rel: &[String]) -> Vec<&str> {
    rel.iter().map(String::as_str).collect()
}

fn is_browser_cache(rel: &[&str]) -> bool {
    let n = rel.len();
    if n < 4 || rel[0] != "appdata" || rel[1] != "local" {
        return false;
    }
    let last = rel[n - 1];
    // Firefox: AppData\Local\Mozilla\Firefox\Profiles\<p>\cache2
    if n == 7 && rel[2..5] == ["mozilla", "firefox", "profiles"] && last == "cache2" {
        return true;
    }
    // Chromium: AppData\Local\<Hersteller…>\User Data\<Profil>\{Cache, Code Cache, GPUCache}
    if n < 5 || !CACHE_DIRS.contains(&last) || rel[n - 3] != "user data" {
        return false;
    }
    let vendor = &rel[2..n - 3];
    matches!(
        vendor,
        ["google", "chrome"] | ["microsoft", "edge"] | ["bravesoftware", "brave-browser"]
    )
}

fn kind_for(
    tree: &Tree,
    id: NodeId,
    path: &NPath,
    prot: Protection,
    env: &NormEnv,
    now: FileTime,
    opts: &CleanupOptions,
) -> Option<CleanupKind> {
    let n = tree.node(id);
    if prot.is_blocked() || n.flags.contains(NodeFlags::ACCESS_DENIED) {
        return None;
    }
    let is_dir = n.is_dir();
    let ext = tree.ext_of(id);
    let rel = env.profile_rel(path).map(rel_str);
    if let Some(r) = rel.as_deref() {
        // Inhalt von <Profil>\AppData\Local\Temp
        if r.len() == 4 && r[..3] == ["appdata", "local", "temp"] {
            let t = if is_dir {
                newest_modified(tree, id)
            } else {
                n.modified
            };
            return older_than(t, now, opts.temp_min_age_days).then_some(CleanupKind::UserTemp);
        }
        if is_dir && is_browser_cache(r) {
            return Some(CleanupKind::BrowserCache);
        }
        if r.len() == 4 && r[..3] == ["appdata", "local", "crashdumps"] {
            return Some(CleanupKind::CrashDumps);
        }
    }
    if !is_dir && ext == "dmp" {
        return Some(CleanupKind::CrashDumps);
    }
    if !is_dir
        && let Some(r) = rel.as_deref()
        && r.len() >= 2
        && r[0] == "downloads"
    {
        if INSTALLER_EXTS.contains(&ext)
            && n.size > 50 * MIB
            && older_than(n.modified, now, opts.installer_min_age_days)
        {
            return Some(CleanupKind::OldInstallers);
        }
        if older_than(n.modified, now, opts.downloads_min_age_days) {
            return Some(CleanupKind::OldDownloads);
        }
    }
    if !is_dir && TEMP_LIKE_EXTS.contains(&ext) && n.size > 10 * MIB {
        return Some(CleanupKind::TempLikeFiles);
    }
    if !is_dir
        && n.size >= opts.large_min_bytes
        && older_than(n.modified, now, opts.large_min_age_days)
    {
        return Some(CleanupKind::LargeOldFiles);
    }
    if is_dir
        && id != tree.root()
        && prot != Protection::Caution(Reason::UserLibrary)
        && truly_empty(tree, id)
    {
        return Some(CleanupKind::EmptyFolders);
    }
    None
}

fn info_kind(path: &NPath) -> Option<CleanupKind> {
    if path.root.len() != 2 || path.comps.len() != 1 {
        return None;
    }
    match path.comps[0].as_str() {
        "hiberfil.sys" => Some(CleanupKind::InfoHibernation),
        "pagefile.sys" | "swapfile.sys" => Some(CleanupKind::InfoPagefile),
        "windows.old" => Some(CleanupKind::InfoWindowsOld),
        "$recycle.bin" => Some(CleanupKind::InfoRecycleBin),
        _ => None,
    }
}

/// Sucht Aufräum-Kandidaten. Ergebnis nach `allocated` absteigend, leere Gruppen fehlen.
pub fn find_cleanup(
    tree: &Tree,
    env: &ProtectEnv,
    now: FileTime,
    opts: &CleanupOptions,
) -> Vec<CleanupGroup> {
    let ne = NormEnv::new(env);
    let mut groups: Vec<CleanupGroup> = CleanupKind::ALL
        .iter()
        .map(|&kind| CleanupGroup {
            kind,
            nodes: Vec::new(),
            size: 0,
            allocated: 0,
            count: 0,
            info_only: kind.is_info(),
        })
        .collect();
    let idx = |k: CleanupKind| CleanupKind::ALL.iter().position(|&x| x == k).unwrap_or(0);
    if tree.nodes.is_empty() {
        return Vec::new();
    }
    walk::<()>(tree, tree.root(), &ne, |id, path, prot| {
        let kind = info_kind(path).or_else(|| kind_for(tree, id, path, prot, &ne, now, opts));
        let Some(kind) = kind else {
            // Unter vererbt gesperrten Bereichen gibt es keine Kandidaten.
            return ControlFlow::Continue(match prot {
                Protection::Blocked(r) if r.is_inherited() => Visit::Skip,
                _ => Visit::Descend,
            });
        };
        let n = tree.node(id);
        if !kind.is_info() && n.is_dir() && contains_blocked_norm(tree, id, &ne) {
            // Ordner mit geschützten Nachkommen nicht als Ganzes vorschlagen.
            return ControlFlow::Continue(Visit::Descend);
        }
        let g = &mut groups[idx(kind)];
        g.nodes.push(id);
        g.size += n.size;
        g.allocated += n.allocated;
        g.count += match (kind, n.is_dir()) {
            (CleanupKind::EmptyFolders, _) => 1 + u64::from(n.dir_count),
            (_, true) => u64::from(n.file_count),
            (_, false) => 1,
        };
        ControlFlow::Continue(Visit::Skip)
    });
    groups.retain(|g| !g.nodes.is_empty());
    groups.sort_by_key(|g| std::cmp::Reverse(g.allocated));
    groups
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::{Lang, tr};
    use crate::model::{EntryInfo, ScanMeta, ScanMode, TreeBuilder};
    use crate::protect::classify;

    const NOW_SECS: i64 = 1_793_318_400; // 2026-10-30

    fn days_ago(d: i64) -> FileTime {
        FileTime::from_unix_secs(NOW_SECS - d * 86_400)
    }

    fn now() -> FileTime {
        FileTime::from_unix_secs(NOW_SECS)
    }

    struct B {
        b: TreeBuilder,
    }

    impl B {
        fn new() -> Self {
            B {
                b: TreeBuilder::new(ScanMeta::new("C:\\", ScanMode::Walk), "C:\\"),
            }
        }
        fn dir(&mut self, parent: NodeId, name: &str) -> NodeId {
            self.b.add_dir(
                parent,
                name,
                EntryInfo {
                    modified: days_ago(1000),
                    ..Default::default()
                },
            )
        }
        fn dir_flags(&mut self, parent: NodeId, name: &str, flags: NodeFlags) -> NodeId {
            self.b.add_dir(
                parent,
                name,
                EntryInfo {
                    modified: days_ago(1000),
                    flags,
                    ..Default::default()
                },
            )
        }
        fn path(&mut self, path: &str) -> NodeId {
            let mut cur = 0;
            for c in path.split('\\') {
                cur = self.dir(cur, c);
            }
            cur
        }
        fn file(&mut self, parent: NodeId, name: &str, size: u64, age_days: i64) -> NodeId {
            self.file_flags(parent, name, size, age_days, NodeFlags::empty())
        }
        fn file_flags(
            &mut self,
            parent: NodeId,
            name: &str,
            size: u64,
            age_days: i64,
            flags: NodeFlags,
        ) -> NodeId {
            self.b.add_file(
                parent,
                name,
                EntryInfo {
                    size,
                    allocated: size,
                    modified: days_ago(age_days),
                    flags,
                    ..Default::default()
                },
            )
        }
    }

    fn env() -> ProtectEnv {
        ProtectEnv::windows_default()
    }

    fn group(gs: &[CleanupGroup], k: CleanupKind) -> Option<&CleanupGroup> {
        gs.iter().find(|g| g.kind == k)
    }

    fn nodes(gs: &[CleanupGroup], k: CleanupKind) -> Vec<NodeId> {
        group(gs, k).map(|g| g.nodes.clone()).unwrap_or_default()
    }

    fn run(t: &Tree) -> Vec<CleanupGroup> {
        find_cleanup(t, &env(), now(), &CleanupOptions::default())
    }

    #[test]
    fn user_temp() {
        let mut b = B::new();
        let temp = b.path(r"Users\joschy\AppData\Local\Temp");
        let old = b.file(temp, "old.tmp", 100, 10);
        let fresh = b.file(temp, "fresh.tmp", 100, 0);
        let sub = b.dir(temp, "sub");
        b.file(sub, "a", 5, 30);
        let sub_new = b.dir(temp, "subnew");
        b.file(sub_new, "b", 5, 0); // jüngste Datei ist frisch -> Ordner bleibt
        let t = b.b.finish();
        let gs = run(&t);
        let n = nodes(&gs, CleanupKind::UserTemp);
        assert!(n.contains(&old) && n.contains(&sub));
        assert!(!n.contains(&fresh) && !n.contains(&sub_new));
        assert_eq!(group(&gs, CleanupKind::UserTemp).map(|g| g.count), Some(2));
    }

    #[test]
    fn browser_cache() {
        let mut b = B::new();
        let chrome = b.path(r"Users\joschy\AppData\Local\Google\Chrome\User Data\Default\Cache");
        b.file(chrome, "data_1", 5000, 0);
        let edge =
            b.path(r"Users\anna\AppData\Local\Microsoft\Edge\User Data\Profile 1\Code Cache");
        b.file(edge, "x", 10, 0);
        let ff = b.path(r"Users\joschy\AppData\Local\Mozilla\Firefox\Profiles\ab12.default\cache2");
        b.file(ff, "y", 10, 0);
        // negativ: Lesezeichen/Cookies und fremder Pfad
        let bm =
            b.path(r"Users\joschy\AppData\Local\Google\Chrome\User Data\Default\Bookmarks_dir");
        b.file(bm, "z", 10, 0);
        let other = b.path(r"Backup\Google\Chrome\User Data\Default\Cache");
        b.file(other, "w", 10, 0);
        let t = b.b.finish();
        let gs = run(&t);
        let n = nodes(&gs, CleanupKind::BrowserCache);
        assert_eq!(n.len(), 3, "{n:?}");
        assert!(n.contains(&chrome) && n.contains(&edge) && n.contains(&ff));
        assert!(!n.contains(&bm) && !n.contains(&other));
    }

    #[test]
    fn crash_dumps() {
        let mut b = B::new();
        let cd = b.path(r"Users\joschy\AppData\Local\CrashDumps");
        let d1 = b.file(cd, "app.exe.123.dmp", 1000, 0);
        let bk = b.dir(0, "Backup");
        let d2 = b.file(bk, "x.dmp", 10, 0);
        let win = b.dir(0, "Windows");
        let d3 = b.file(win, "MEMORY.DMP", 10_000, 0);
        let t = b.b.finish();
        let gs = run(&t);
        let n = nodes(&gs, CleanupKind::CrashDumps);
        assert!(n.contains(&d1) && n.contains(&d2));
        assert!(!n.contains(&d3));
    }

    #[test]
    fn downloads_and_installers_no_double_count() {
        let mut b = B::new();
        let dl = b.path(r"Users\joschy\Downloads");
        let iso = b.file(dl, "win.iso", 5000 * MIB, 400); // Installer, auch groß und alt
        let small_old = b.file(dl, "brief.pdf", 1000, 200);
        let new_inst = b.file(dl, "neu.msi", 60 * MIB, 10);
        let tiny_inst = b.file(dl, "tool.exe", MIB, 100); // zu klein -> OldDownloads? nein, < 180 Tage
        let sub = b.dir(dl, "alt");
        let nested = b.file(sub, "a.txt", 10, 365);
        let t = b.b.finish();
        let gs = run(&t);
        let inst = nodes(&gs, CleanupKind::OldInstallers);
        let old = nodes(&gs, CleanupKind::OldDownloads);
        assert_eq!(inst, vec![iso]);
        assert!(old.contains(&small_old) && old.contains(&nested));
        assert!(!old.contains(&iso), "keine Doppelzählung");
        assert!(!old.contains(&new_inst) && !inst.contains(&new_inst));
        assert!(!old.contains(&tiny_inst) && !inst.contains(&tiny_inst));
        assert!(!nodes(&gs, CleanupKind::LargeOldFiles).contains(&iso));
        // Summe aller Gruppen <= Gesamtgröße (nichts doppelt)
        let total: u64 = gs.iter().map(|g| g.size).sum();
        assert!(total <= t.node(0).size);
        let mut all: Vec<NodeId> = gs.iter().flat_map(|g| g.nodes.clone()).collect();
        let len = all.len();
        all.sort_unstable();
        all.dedup();
        assert_eq!(all.len(), len);
    }

    #[test]
    fn large_old_files() {
        let mut b = B::new();
        let bk = b.dir(0, "Backup");
        let big_old = b.file(bk, "image.vhdx", 2 << 30, 400);
        let big_new = b.file(bk, "neu.vhdx", 2 << 30, 10);
        let small_old = b.file(bk, "klein.bin", 100 * MIB, 800);
        let pf = b.path(r"Program Files\Spiel");
        let big_pf = b.file(pf, "data.pak", 4 << 30, 900);
        let t = b.b.finish();
        let gs = run(&t);
        let n = nodes(&gs, CleanupKind::LargeOldFiles);
        assert_eq!(n, vec![big_old]);
        assert!(!n.contains(&big_new) && !n.contains(&small_old) && !n.contains(&big_pf));
    }

    #[test]
    fn temp_like_files() {
        let mut b = B::new();
        let bk = b.dir(0, "Daten");
        let log = b.file(bk, "server.LOG", 20 * MIB, 0);
        let small = b.file(bk, "klein.log", MIB, 0);
        let other = b.file(bk, "film.mkv", 20 * MIB, 0);
        let pd = b.path(r"ProgramData\Microsoft\Logs");
        let blocked_log = b.file(pd, "big.log", 50 * MIB, 0);
        let t = b.b.finish();
        let gs = run(&t);
        let n = nodes(&gs, CleanupKind::TempLikeFiles);
        assert_eq!(n, vec![log]);
        assert!(!n.contains(&small) && !n.contains(&other) && !n.contains(&blocked_log));
    }

    #[test]
    fn empty_folders() {
        let mut b = B::new();
        let bk = b.dir(0, "Backup");
        let e1 = b.dir(bk, "leer");
        b.dir(e1, "auch_leer"); // verschachtelt -> nur e1
        let full = b.dir(bk, "voll");
        b.file(full, "x", 1, 0);
        let link = b.dir_flags(bk, "Link", NodeFlags::REPARSE);
        let denied = b.dir_flags(bk, "Gesperrt", NodeFlags::ACCESS_DENIED);
        let music = b.path(r"Users\joschy\Music");
        let ad = b.path(r"Users\joschy\AppData\Roaming"); // blockiert (AppDataRoot), leer
        let t = b.b.finish();
        let gs = run(&t);
        let n = nodes(&gs, CleanupKind::EmptyFolders);
        assert_eq!(n, vec![e1]);
        assert!(!n.contains(&link) && !n.contains(&denied) && !n.contains(&music));
        assert!(!n.contains(&ad) && !n.contains(&full) && !n.contains(&0));
        assert_eq!(
            group(&gs, CleanupKind::EmptyFolders).map(|g| g.count),
            Some(2)
        );
    }

    #[test]
    fn info_groups() {
        let mut b = B::new();
        let h = b.file_flags(
            0,
            "hiberfil.sys",
            8 << 30,
            1,
            NodeFlags::SYSTEM | NodeFlags::HIDDEN,
        );
        let p = b.file_flags(
            0,
            "pagefile.sys",
            4 << 30,
            1,
            NodeFlags::SYSTEM | NodeFlags::HIDDEN,
        );
        let s = b.file(0, "swapfile.sys", 1 << 20, 1);
        let wo = b.dir(0, "Windows.old");
        let wof = b.file(wo, "big.dat", 3 << 30, 900); // nicht in LargeOldFiles
        let rb = b.dir(0, "$Recycle.Bin");
        b.file(rb, "$R1.iso", 100, 1);
        let bk = b.dir(0, "Backup");
        b.file(bk, "pagefile.sys", 10, 1); // nicht in der Wurzel -> keine Info
        let t = b.b.finish();
        let gs = run(&t);
        assert_eq!(nodes(&gs, CleanupKind::InfoHibernation), vec![h]);
        assert_eq!(nodes(&gs, CleanupKind::InfoPagefile), vec![p, s]);
        assert_eq!(nodes(&gs, CleanupKind::InfoWindowsOld), vec![wo]);
        assert_eq!(nodes(&gs, CleanupKind::InfoRecycleBin), vec![rb]);
        assert!(gs.iter().filter(|g| g.kind.is_info()).all(|g| g.info_only));
        assert!(!nodes(&gs, CleanupKind::LargeOldFiles).contains(&wof));
        // sortiert nach allocated absteigend
        assert!(gs.windows(2).all(|w| w[0].allocated >= w[1].allocated));
        assert_eq!(gs[0].kind, CleanupKind::InfoHibernation);
    }

    #[test]
    fn blocked_never_suggested() {
        let mut b = B::new();
        let win_temp = b.path(r"Windows\Temp");
        b.file(win_temp, "x.tmp", 50 * MIB, 100);
        b.file(win_temp, "y.dmp", 50 * MIB, 100);
        let pf = b.path(r"Program Files\X");
        b.file(pf, "huge.bin", 5 << 30, 900);
        b.dir(pf, "empty");
        let sysd = b.dir_flags(0, "SysDir", NodeFlags::SYSTEM);
        b.file(sysd, "old.log", 50 * MIB, 900);
        let temp = b.path(r"Users\joschy\AppData\Local\Temp");
        let tsub = b.dir(temp, "mit_system");
        b.file_flags(tsub, "lock.dat", 1, 100, NodeFlags::SYSTEM);
        let hive = b.path(r"Users\joschy");
        b.file(hive, "NTUSER.DAT", 50 * MIB, 900);
        b.file(0, "bootmgr.log", 20 * MIB, 1); // frei, darf vorkommen
        let t = b.b.finish();
        let gs = run(&t);
        let e = env();
        for g in gs.iter().filter(|g| !g.info_only) {
            for &id in &g.nodes {
                assert!(
                    !classify(&t, id, &e).is_blocked(),
                    "{:?} {}",
                    g.kind,
                    t.full_path(id)
                );
                assert!(!crate::protect::contains_blocked(&t, id, &e));
            }
        }
        assert!(!nodes(&gs, CleanupKind::UserTemp).contains(&tsub));
        assert_eq!(nodes(&gs, CleanupKind::TempLikeFiles).len(), 1);
    }

    #[test]
    fn empty_tree_and_options() {
        let t = B::new().b.finish();
        assert!(run(&t).is_empty());
        let o = CleanupOptions::default();
        assert_eq!(o.temp_min_age_days, 2);
        assert_eq!(o.large_min_bytes, 1 << 30);
        assert!(!older_than(FileTime(0), now(), 1));
        assert!(older_than(days_ago(3), now(), 2));
        assert!(!older_than(days_ago(1), now(), 2));
    }

    #[test]
    fn i18n_for_every_kind() {
        for k in CleanupKind::ALL {
            for lang in Lang::ALL {
                assert!(!tr(lang, k.title_key()).trim().is_empty(), "{k:?}");
                assert!(!tr(lang, k.hint_key()).trim().is_empty(), "{k:?}");
            }
        }
        assert!(tr(Lang::De, CleanupKind::InfoHibernation.hint_key()).contains("powercfg -h off"));
    }
}
