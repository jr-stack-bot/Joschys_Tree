// Joschys_Tree – Nur Debug-Build: Start-Hook für Bildschirmfotos und Sichtprüfung – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later
//
// Existiert ausschließlich unter #[cfg(debug_assertions)] (siehe main.rs), nie im Release.
// JT_DEBUG_SCAN=<Pfad>   startet beim Programmstart einen Scan dieses Ziels
// JT_DEBUG_TAB=<Name>    tree | treemap | top | types | search | dups | delta
// JT_DEBUG_THEME=<Name>  dark | light
// JT_DEBUG_LANG=<Code>   de | en
// JT_DEBUG_DEMO=1        künstlicher Windows-Baum (C:\Windows, Downloads, Temp …) als
//                        "Live-Scan" mit simulierten Admin-Rechten. Gelöscht wird dabei
//                        nie etwas: die Pfade existieren nicht, und der Löschweg auf
//                        Nicht-Windows ist ein Stub.
// JT_DEBUG_BASKET=1      (mit Demo) einige Elemente in den Korb legen
// JT_DEBUG_COLOR=<Name>  type | age | prot (Treemap-Farbmodus)
// JT_DEBUG_ZOOM=<Pfad>   Treemap-Zoom auf diesen Ordner
// JT_DEBUG_SELECT=<Pfad> Element auswählen
// JT_DEBUG_DIALOG=basket Lösch-Dialog für den Korb öffnen

use std::sync::atomic::{AtomicBool, Ordering};

use jt_core::aggregate::{SortKey, sorted_children};
use jt_core::model::{EntryInfo, ScanMeta, ScanMode, TreeBuilder};
use jt_core::{FileTime, Lang, NodeFlags, NodeId, Tree};

use crate::app::{Action, AppState, Tab};
use crate::settings::TreemapColor;

static DEMO_ADMIN: AtomicBool = AtomicBool::new(false);

/// Simulierte Admin-Rechte (nur Demo im Debug-Build).
pub fn demo_admin() -> bool {
    DEMO_ADMIN.load(Ordering::Relaxed)
}

fn var(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.trim().is_empty())
}

/// Beim Start: Theme, Sprache, Tab und Scan-Ziel aus Umgebungsvariablen.
pub fn on_start(state: &mut AppState) {
    match var("JT_DEBUG_THEME").as_deref() {
        Some("light") => state.settings.dark = false,
        Some("dark") => state.settings.dark = true,
        _ => {}
    }
    match var("JT_DEBUG_LANG").as_deref() {
        Some("en") => state.settings.lang = Lang::En,
        Some("de") => state.settings.lang = Lang::De,
        _ => {}
    }
    if let Some(tab) = var("JT_DEBUG_TAB") {
        state.tab = match tab.as_str() {
            "treemap" => Tab::Treemap,
            "top" => Tab::TopFiles,
            "types" => Tab::FileTypes,
            "search" => Tab::Search,
            "dups" => Tab::Duplicates,
            "delta" => Tab::Delta,
            "clean" => Tab::Cleanup,
            _ => Tab::Tree,
        };
    }
    match var("JT_DEBUG_COLOR").as_deref() {
        Some("age") => state.settings.treemap_color = TreemapColor::Age,
        Some("prot") => state.settings.treemap_color = TreemapColor::Protection,
        Some("type") => state.settings.treemap_color = TreemapColor::FileType,
        _ => {}
    }
    if let Some(target) = var("JT_DEBUG_SCAN") {
        state.target_input = target;
        state.actions.push(Action::StartScan);
    }
    if var("JT_DEBUG_DEMO").as_deref() == Some("1") {
        start_demo(state);
    }
}

fn start_demo(state: &mut AppState) {
    DEMO_ADMIN.store(true, Ordering::Relaxed);
    let tree = demo_tree(jt_core::filter::now());
    state.target_input = tree.meta.root_path.clone();
    state.set_tree(tree, true);
    state.actions.push(Action::StartCleanup);
    let Some(t) = state.tree.clone() else { return };
    let find = |p: &str| t.find_path(p);
    if let Some(z) = var("JT_DEBUG_ZOOM").and_then(|p| find(&p)) {
        state.treemap.root = z;
    }
    if let Some(s) = var("JT_DEBUG_SELECT").and_then(|p| find(&p)) {
        state.selected = Some(s);
        state.tree_view.reveal(&t, s);
    }
    if var("JT_DEBUG_BASKET").as_deref() == Some("1") {
        let mut ids: Vec<NodeId> = [
            r"C:\Users\demo\Downloads\Win11_23H2_German_x64.iso",
            r"C:\Users\demo\Downloads\ubuntu-22.04.3-desktop-amd64.iso",
            r"C:\Users\demo\AppData\Local\Google\Chrome\User Data\Default\Cache",
            r"C:\Backup\Server2016_Export\Abteilung_Vertrieb",
            r"C:\Users\demo\AppData\Roaming\Spotify\Data",
        ]
        .iter()
        .filter_map(|p| find(p))
        .collect();
        // Temp selbst ist geschützt, sein Inhalt nicht.
        if let Some(tmp) = find(r"C:\Users\demo\AppData\Local\Temp") {
            ids.extend(t.children(tmp));
        }
        state.actions.push(Action::AddBasket(ids));
        state.settings.basket_open = true;
    }
    if var("JT_DEBUG_OPEN").as_deref() == Some("1") {
        state
            .cleanup
            .open
            .insert(jt_core::cleanup::CleanupKind::OldInstallers);
    }
    if var("JT_DEBUG_DIALOG").as_deref() == Some("basket") {
        state.actions.push(Action::AskDeleteBasket);
    }
}

/// Einfacher, fester Zufallsgenerator (xorshift) für reproduzierbare Demodaten.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    /// 0.0 .. 1.0
    fn unit(&mut self) -> f64 {
        (self.next() % 10_000) as f64 / 10_000.0
    }
}

const GIB: u64 = 1 << 30;
const MIB: u64 = 1 << 20;

/// Künstlicher Windows-Baum für Bildschirmfotos.
pub fn demo_tree(now: FileTime) -> Tree {
    let day = 86_400i64 * 10_000_000;
    let ago = |d: i64| FileTime(now.0 - d * day);
    let mut meta = ScanMeta::new("C:\\", ScanMode::Mft);
    meta.cluster_size = 4096;
    meta.started = now;
    meta.volume_total = 512 * GIB;
    meta.volume_free = 61 * GIB;
    let mut b = TreeBuilder::new(meta, "C:\\");
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let dir = |flags: NodeFlags, d: i64| EntryInfo {
        modified: ago(d),
        created: ago(d),
        accessed: ago(d),
        flags,
        ..Default::default()
    };
    let file = |size: u64, d: i64, flags: NodeFlags| EntryInfo {
        size,
        allocated: size.div_ceil(4096) * 4096,
        modified: ago(d),
        created: ago(d),
        accessed: ago(d / 2),
        flags,
        ..Default::default()
    };
    let n0 = NodeFlags::empty();
    let hid = NodeFlags::HIDDEN;
    let sys = NodeFlags::SYSTEM | NodeFlags::HIDDEN;
    // Viele Dateien mit Gesamtgröße `total` in einem Ordner.
    let mut many = |b: &mut TreeBuilder,
                    parent: NodeId,
                    stem: &str,
                    ext: &str,
                    n: usize,
                    total: u64,
                    age: (i64, i64),
                    flags: NodeFlags| {
        let weights: Vec<f64> = (0..n).map(|_| 0.05 + rng.unit().powi(3)).collect();
        let sum: f64 = weights.iter().sum();
        for (i, w) in weights.iter().enumerate() {
            let size = ((total as f64) * w / sum) as u64 + 1;
            let d = age.0 + ((age.1 - age.0) as f64 * rng.unit()) as i64;
            b.add_file(parent, &format!("{stem}{i:03}.{ext}"), file(size, d, flags));
        }
    };

    // ---- C:\Windows (gesperrt) ----
    let win = b.add_dir(0, "Windows", dir(n0, 30));
    let s32 = b.add_dir(win, "System32", dir(n0, 30));
    many(
        &mut b,
        s32,
        "api-ms-win-core-",
        "dll",
        60,
        3 * GIB,
        (30, 900),
        n0,
    );
    b.add_file(s32, "ntoskrnl.exe", file(12 * MIB, 30, n0));
    let drv = b.add_dir(s32, "DriverStore", dir(n0, 60));
    many(&mut b, drv, "oem", "inf_cab", 30, 4 * GIB, (60, 700), n0);
    let sxs = b.add_dir(win, "WinSxS", dir(n0, 20));
    many(
        &mut b,
        sxs,
        "amd64_microsoft-windows-",
        "manifest",
        80,
        9 * GIB,
        (20, 1200),
        n0,
    );
    let sd = b.add_dir(win, "SoftwareDistribution", dir(n0, 5));
    many(&mut b, sd, "update", "cab", 12, 3 * GIB, (5, 90), n0);
    let wtemp = b.add_dir(win, "Temp", dir(n0, 3));
    many(&mut b, wtemp, "cab_", "tmp", 10, 800 * MIB, (3, 300), n0);
    let fonts = b.add_dir(win, "Fonts", dir(n0, 400));
    many(&mut b, fonts, "font", "ttf", 40, 900 * MIB, (400, 2000), n0);

    // ---- Programme (gesperrt) ----
    let pf = b.add_dir(0, "Program Files", dir(n0, 40));
    let office = b.add_dir(pf, "Microsoft Office", dir(n0, 40));
    many(&mut b, office, "mso", "dll", 25, 4 * GIB, (40, 400), n0);
    let chrome = b.add_dir(pf, "Google", dir(n0, 10));
    many(&mut b, chrome, "chrome", "dll", 10, 900 * MIB, (10, 60), n0);
    let adobe = b.add_dir(pf, "Adobe", dir(n0, 200));
    many(&mut b, adobe, "acro", "dll", 20, 2 * GIB, (200, 500), n0);
    let pf86 = b.add_dir(0, "Program Files (x86)", dir(n0, 100));
    let steam = b.add_dir(pf86, "Steam", dir(n0, 100));
    let games = b.add_dir(steam, "steamapps", dir(n0, 100));
    many(
        &mut b,
        games,
        "game_data_",
        "pak",
        14,
        38 * GIB,
        (100, 900),
        n0,
    );

    // ---- ProgramData ----
    let pd = b.add_dir(0, "ProgramData", dir(hid, 50));
    let pdm = b.add_dir(pd, "Microsoft", dir(n0, 50));
    many(&mut b, pdm, "defender_", "vdm", 12, 1500 * MIB, (1, 50), n0);
    let pkg = b.add_dir(pd, "Package Cache", dir(n0, 300));
    many(
        &mut b,
        pkg,
        "vc_redist_",
        "msi",
        16,
        2 * GIB,
        (300, 1500),
        n0,
    );

    // ---- Benutzerprofil ----
    let users = b.add_dir(0, "Users", dir(n0, 400));
    let demo = b.add_dir(users, "demo", dir(n0, 400));
    b.add_file(demo, "NTUSER.DAT", file(12 * MIB, 0, hid));
    let dl = b.add_dir(demo, "Downloads", dir(n0, 2));
    b.add_file(dl, "Win11_23H2_German_x64.iso", file(6_600 * MIB, 420, n0));
    b.add_file(
        dl,
        "ubuntu-22.04.3-desktop-amd64.iso",
        file(4_900 * MIB, 760, n0),
    );
    b.add_file(dl, "SQLServer2019-x64-DEU.iso", file(1_500 * MIB, 1100, n0));
    b.add_file(dl, "VMware-workstation-full.exe", file(620 * MIB, 380, n0));
    b.add_file(dl, "Treiber_Drucker.zip", file(410 * MIB, 250, n0));
    many(&mut b, dl, "Rechnung_", "pdf", 25, 90 * MIB, (2, 900), n0);
    b.add_dir(dl, "Neuer Ordner", dir(n0, 500));
    let docs = b.add_dir(demo, "Documents", dir(n0, 3));
    many(&mut b, docs, "Angebot_", "docx", 30, 2 * GIB, (3, 1500), n0);
    let pics = b.add_dir(demo, "Pictures", dir(n0, 10));
    many(&mut b, pics, "IMG_", "jpg", 60, 11 * GIB, (10, 2500), n0);
    let vids = b.add_dir(demo, "Videos", dir(n0, 60));
    many(&mut b, vids, "Urlaub_", "mp4", 12, 21 * GIB, (60, 1800), n0);
    let music = b.add_dir(demo, "Music", dir(n0, 900));
    many(&mut b, music, "track", "mp3", 40, 3 * GIB, (900, 3000), n0);
    let app = b.add_dir(demo, "AppData", dir(hid, 400));
    let local = b.add_dir(app, "Local", dir(n0, 1));
    let temp = b.add_dir(local, "Temp", dir(n0, 1));
    many(&mut b, temp, "~DF", "tmp", 40, 3_400 * MIB, (5, 400), n0);
    let gch = b.add_dir(local, "Google", dir(n0, 1));
    let gc = b.add_dir(gch, "Chrome", dir(n0, 1));
    let ud = b.add_dir(gc, "User Data", dir(n0, 1));
    let def = b.add_dir(ud, "Default", dir(n0, 1));
    let cache = b.add_dir(def, "Cache", dir(n0, 1));
    many(&mut b, cache, "f_", "", 50, 1_800 * MIB, (3, 120), n0);
    let crash = b.add_dir(local, "CrashDumps", dir(n0, 90));
    many(
        &mut b,
        crash,
        "explorer.exe.",
        "dmp",
        6,
        1_100 * MIB,
        (90, 400),
        n0,
    );
    let msw = b.add_dir(local, "Microsoft", dir(n0, 1));
    let mswin = b.add_dir(msw, "Windows", dir(n0, 1));
    many(
        &mut b,
        mswin,
        "explorer_cache_",
        "db",
        8,
        600 * MIB,
        (1, 30),
        n0,
    );
    let roam = b.add_dir(app, "Roaming", dir(n0, 20));
    let spot = b.add_dir(roam, "Spotify", dir(n0, 20));
    let spd = b.add_dir(spot, "Data", dir(n0, 20));
    many(&mut b, spd, "chunk", "file", 20, 2_600 * MIB, (20, 200), n0);

    // ---- Backup auf dem Systemlaufwerk (frei, große Ordner) ----
    let bk = b.add_dir(0, "Backup", dir(n0, 700));
    let exp = b.add_dir(bk, "Server2016_Export", dir(n0, 700));
    for (name, gib, age) in [
        ("Abteilung_Vertrieb", 46u64, 720i64),
        ("Abteilung_Technik", 31, 800),
        ("Buchhaltung_2019", 18, 1900),
        ("Scans", 12, 1400),
    ] {
        let d = b.add_dir(exp, name, dir(n0, age));
        many(
            &mut b,
            d,
            "daten_",
            "zip",
            18,
            gib * GIB,
            (age, age + 600),
            n0,
        );
    }
    let img = b.add_dir(bk, "Systemabbild", dir(n0, 1000));
    many(
        &mut b,
        img,
        "WindowsImageBackup_",
        "vhdx",
        4,
        52 * GIB,
        (1000, 1100),
        n0,
    );
    // Systemdatei im Backup: der Ordner enthält ein gesperrtes Element.
    b.add_file(img, "desktop.ini", file(1024, 1000, sys));

    // ---- Wurzel: Systemdateien, alte Installation, Papierkorb ----
    b.add_file(0, "pagefile.sys", file(16 * GIB, 0, sys));
    b.add_file(0, "hiberfil.sys", file(12_800 * MIB, 0, sys));
    b.add_file(0, "swapfile.sys", file(256 * MIB, 0, sys));
    let old = b.add_dir(0, "Windows.old", dir(n0, 380));
    many(
        &mut b,
        old,
        "old_system_",
        "dll",
        30,
        21 * GIB,
        (380, 900),
        n0,
    );
    let rb = b.add_dir(0, "$Recycle.Bin", dir(sys, 4));
    many(&mut b, rb, "$R", "dat", 10, 1_300 * MIB, (4, 60), n0);
    b.finish()
}

/// Nach dem Scan: zwei Ebenen der größten Ordner aufklappen und einen Eintrag wählen.
pub fn after_scan(state: &mut AppState, tree: &Tree) {
    if var("JT_DEBUG_SCAN").is_none() {
        return;
    }
    let mut cur = tree.root();
    let mut pick = None;
    for _ in 0..2 {
        let kids = sorted_children(tree, cur, SortKey::Size, true);
        pick = kids.get(1).or(kids.first()).copied();
        let Some(dir) = kids.into_iter().find(|&k| tree.node(k).is_dir()) else {
            break;
        };
        state.tree_view.expanded.insert(dir);
        cur = dir;
    }
    if let Some(p) = pick {
        state.selected = Some(p);
        if state.tab == Tab::Treemap {
            state.selected = None;
        }
    }
    state.tree_view.invalidate();
}
