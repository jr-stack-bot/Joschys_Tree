<!-- Joschys_Tree – Bauplan  @ Joschy -->
# Joschys_Tree – Bauplan  (@ Joschy)

Schneller, portabler Festplatten-Analyzer für Windows (WizTree + TreeSize Pro). Rust + egui/eframe, eine einzelne .exe, nur GUI.
Stand: 2026-10-30. Dieses Dokument ist verbindlich für alle Bauaufträge.

---

## 0. Umgebung (per Bash geprüft)

| Prüfung | Ergebnis |
|---|---|
| `rustc --version` | 1.97.0 (stable), cargo 1.97.0, clippy/rustfmt vorhanden |
| Installierte Targets | nur `x86_64-unknown-linux-gnu` |
| `rustup target add x86_64-pc-windows-gnu` | **schlägt fehl**: Proxy blockt `static.rust-lang.org` (403) |
| mingw (`x86_64-w64-mingw32-gcc`) | war nicht da, per apt nachinstalliert (13.2), nützt ohne Rust-std für Windows aber nichts |
| `cargo-xwin` / `cross` / MSVC-SDK | nicht vorhanden, Docker/Downloads ebenfalls blockiert |
| crates.io (Index + Download) | erreichbar. Probe-Build mit allen gewählten Crates (`cargo check`) auf Linux: **OK** |

**Fazit: Cross-Compile nach Windows ist hier NICHT möglich.** `cfg(windows)`-Code kann lokal nicht einmal typgeprüft werden.
Folge: Die Windows-Teile werden nur von GitHub Actions (`windows-latest`) gebaut und mit clippy geprüft. Jeder Push muss dort grün sein. Lokal: `cargo test` / `cargo clippy` für Linux.

---

## 1. Workspace- und Modulstruktur

Ein Cargo-Workspace mit **zwei Crates**. So bleibt die Kernlogik frei von GUI- und Windows-Abhängigkeiten und lässt sich auf Linux testen.

```
joschys_tree/
├─ Cargo.toml                 Workspace (members = crates/*), [profile.release] opt-level=3, lto="fat", codegen-units=1, strip=true, panic="abort"
├─ rust-toolchain.toml        channel = "1.99.1" (fest; Mindestversion 1.95, kommt von egui 0.37)
├─ README.md  LICENSE (AGPL-3.0, Volltext)  BUILD.md
├─ .github/workflows/build.yml
├─ crates/
│  ├─ jt_core/                Bibliothek, KEINE GUI, KEIN windows-Crate außer in platform/ und scan/mft*
│  │  ├─ Cargo.toml
│  │  └─ src/
│  │     ├─ lib.rs            pub mod-Liste, Re-Exports, BRANDING-Konstante
│  │     ├─ model.rs          Datenmodell (Abschnitt 2): Node, Tree, NodeId, FileTime, ScanMeta, Builder
│  │     ├─ aggregate.rs      Größen/Cluster/Zähler nach oben rechnen, Prozent, Sortierung, Pfad-Rekonstruktion
│  │     ├─ stats.rs          Top-100-Dateien, Dateitypen-Statistik, Alters-Histogramm
│  │     ├─ filter.rs         Filter-Ausdruck (Name/Glob, Größe, Alter, Typ, Attribute), Suche
│  │     ├─ duplicates.rs     Duplikat-Suche: Größe → 4-KiB-Teil-Hash → Voll-Hash (xxh3-128), parallel
│  │     ├─ snapshot.rs       Scan speichern/laden (.jtscan = Header + postcard + lz4)
│  │     ├─ delta.rs          Delta-Vergleich zweier Trees (per Pfad), Ergebnisliste
│  │     ├─ history.rs        Verlaufs-/Wachstumsanalyse über N Snapshots eines Ziels
│  │     ├─ export/
│  │     │  ├─ mod.rs         ExportRequest, ExportFormat, gemeinsame Zeilen-Erzeugung (Rows)
│  │     │  ├─ csv.rs         CSV (UTF-8 mit BOM, Semikolon bei de, Komma bei en)
│  │     │  ├─ xlsx.rs        Excel via rust_xlsxwriter
│  │     │  ├─ html.rs        selbst erzeugtes HTML, Fuß "@ Joschy"
│  │     │  └─ pdf.rs         PDF via pdf-writer (Tabelle, Standard-Font Helvetica/WinAnsi), Fuß "@ Joschy"
│  │     ├─ i18n.rs           Lang-Enum, Key-Enum, zentrale Texttabelle de/en, tr()
│  │     ├─ format.rs         Bytes→"1,23 GiB", Datum, Zahlen je Sprache
│  │     ├─ scan/
│  │     │  ├─ mod.rs         Scanner-Trait, ScanRequest, ScanProgress, Auswahl MFT↔Walk mit Fallback
│  │     │  ├─ walk.rs        paralleler Verzeichnis-Scan (std::fs + rayon), plattformneutral; unter Windows mit FindFirstFileExW-Pfad (cfg)
│  │     │  ├─ mft_parse.rs   REINER Parser für MFT-Records (Fixup, Attribute 0x10/0x30/0x80, Runlists) – plattformneutral, mit Testdaten
│  │     │  └─ mft_win.rs     #[cfg(windows)] Volume öffnen (\\.\C:), $MFT lesen, an mft_parse füttern
│  │     └─ platform/
│  │        ├─ mod.rs         Platform-Trait-Funktionen + Stub für Nicht-Windows
│  │        └─ win.rs         #[cfg(windows)] Admin-Check, Papierkorb, endgültig löschen, Explorer öffnen, Owner/ACL, ADS, Cluster-Größe, lange Pfade
│  └─ jt_gui/                 Binary "JoschysTree.exe"
│     ├─ Cargo.toml
│     ├─ build.rs             #[cfg(windows)] Manifest einbetten (embed-manifest): longPathAware, asInvoker, PerMonitorV2 DPI
│     └─ src/
│        ├─ main.rs           eframe-Start, #![windows_subsystem="windows"], Fenstertitel "Joschys_Tree @ Joschy"
│        ├─ app.rs            AppState, Tab-Steuerung, Hintergrund-Jobs (Scan/Duplikate/Export) über Kanäle
│        ├─ theme.rs          Dunkel/Hell (Standard dunkel), Farbpalette für Treemap
│        ├─ settings.rs       Einstellungen (Sprache, Theme, letzte Ziele) als JSON neben der exe bzw. %APPDATA%
│        ├─ views/
│        │  ├─ tree_view.rs   Baumtabelle (virtuell, egui_extras::TableBuilder), Spalten, Rechtsklick-Menü
│        │  ├─ treemap.rs     Squarified-Layout (Layout-Funktion reine Logik + Zeichnen)
│        │  ├─ top_files.rs   Top-100
│        │  ├─ file_types.rs  Dateitypen-Statistik
│        │  ├─ search.rs      Suche + Filter-Panel
│        │  ├─ duplicates.rs  Duplikat-Ergebnisse
│        │  ├─ delta.rs       Delta- und Verlaufsansicht
│        │  └─ details.rs     Detailpanel: Owner, ACL, ADS, Zeiten
│        ├─ dialogs.rs        Lösch-Bestätigung (2-stufig), Export-Dialog, Über-Dialog
│        └─ footer.rs         Statuszeile + Fußzeile "Joschys_Tree @ Joschy · Admin/Nur-Lesen · Scan-Modus"
```

Regel: **Jede Quelldatei** beginnt mit `// Joschys_Tree – <Datei-Zweck> – @ Joschy` und `// SPDX-License-Identifier: AGPL-3.0-or-later`. Ein Test (`tests/branding.rs`) prüft das für alle `*.rs`.

---

## 2. Zentrales Datenmodell und Schnittstellen

Diese Signaturen legt **Auftrag A** zuerst als Skelett an (mit `todo!()` wo nötig). Danach ändert niemand sie ohne Absprache. Alle Typen in `jt_core`.

### 2.1 model.rs

```rust
pub const BRANDING: &str = "@ Joschy";

pub type NodeId = u32;                       // Index in Tree::nodes, 0 = Wurzel
pub const NO_NODE: NodeId = u32::MAX;

#[derive(Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct FileTime(pub i64);                // Windows FILETIME (100 ns seit 1601), 0 = unbekannt
impl FileTime { pub fn to_unix_secs(self) -> i64; pub fn from_unix_secs(s: i64) -> Self; }

bitflags! { #[derive(Serialize, Deserialize)] pub struct NodeFlags: u16 {
    const DIR = 1; const HIDDEN = 2; const SYSTEM = 4; const READONLY = 8;
    const REPARSE = 16;      // Junction/Symlink: wird NICHT verfolgt
    const COMPRESSED = 32; const SPARSE = 64; const HAS_ADS = 128;
    const ACCESS_DENIED = 256; const OFFLINE = 512; } }

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Node {
    pub name: Box<str>,      // nur der Name, kein Pfad (spart RAM)
    pub parent: NodeId,
    pub first_child: NodeId, // verkettete Kinderliste: first_child / next_sibling
    pub next_sibling: NodeId,
    pub size: u64,           // logische Größe (bei Ordnern: Summe, von aggregate gesetzt)
    pub allocated: u64,      // belegter Speicher auf Platte (Cluster-gerundet bzw. echte Allocation)
    pub file_count: u32,     // bei Ordnern rekursiv; Datei = 0
    pub dir_count: u32,
    pub created: FileTime, pub modified: FileTime, pub accessed: FileTime,
    pub flags: NodeFlags,
    pub ext_id: u16,         // Index in Tree::extensions, 0 = ohne Endung
    pub ads_size: u64,       // Summe alternativer Datenströme (0 wenn keine/unbekannt)
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum ScanMode { Mft, Walk }

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ScanMeta {
    pub root_path: String,         // z. B. "C:\\" oder "\\\\server\\share"
    pub mode: ScanMode,
    pub started: FileTime, pub duration_ms: u64,
    pub cluster_size: u32,         // 0 = unbekannt (dann allocated = size)
    pub volume_total: u64, pub volume_free: u64,
    pub errors: u32,               // Anzahl Zugriffsfehler
    pub tool_version: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Tree {
    pub nodes: Vec<Node>,
    pub extensions: Vec<Box<str>>, // [0] = "" ; kleingeschrieben, ohne Punkt
    pub meta: ScanMeta,
}
impl Tree {
    pub fn root(&self) -> NodeId;                                  // immer 0
    pub fn node(&self, id: NodeId) -> &Node;
    pub fn children(&self, id: NodeId) -> ChildIter<'_>;
    pub fn full_path(&self, id: NodeId) -> String;                 // Windows-Trenner '\'
    pub fn find_path(&self, path: &str) -> Option<NodeId>;         // case-insensitiv
    pub fn ext_of(&self, id: NodeId) -> &str;
}

/// Wird von beiden Scannern benutzt; garantiert konsistente Trees.
pub struct TreeBuilder { /* intern: nodes, ext-HashMap */ }
impl TreeBuilder {
    pub fn new(meta: ScanMeta, root_name: &str) -> Self;
    pub fn add_dir(&mut self, parent: NodeId, name: &str, info: EntryInfo) -> NodeId;
    pub fn add_file(&mut self, parent: NodeId, name: &str, info: EntryInfo) -> NodeId;
    /// MFT: Einträge kommen in beliebiger Reihenfolge -> Eltern später verknüpfen
    pub fn add_orphan(&mut self, key: u64, parent_key: u64, name: &str, info: EntryInfo, is_dir: bool);
    pub fn finish(self) -> Tree;   // verknüpft Waisen, ruft aggregate::recompute
}

#[derive(Clone, Default, Debug)]
pub struct EntryInfo { pub size: u64, pub allocated: u64, pub created: FileTime,
    pub modified: FileTime, pub accessed: FileTime, pub flags: NodeFlags, pub ads_size: u64 }
```

Speicherbedarf: ca. 90 Byte + Name pro Eintrag, das sind etwa 300 MB bei 3 Mio. Einträgen. Akzeptiert.

### 2.2 Schnittstellen der Module

```rust
// aggregate.rs
pub fn recompute(tree: &mut Tree);                         // Bottom-up size/allocated/counts
pub fn percent_of_parent(tree: &Tree, id: NodeId) -> f32;
pub enum SortKey { Name, Size, Allocated, Files, Modified }
pub fn sorted_children(tree: &Tree, id: NodeId, key: SortKey, desc: bool) -> Vec<NodeId>;

// stats.rs
pub fn top_files(tree: &Tree, n: usize, filter: Option<&Filter>) -> Vec<NodeId>;   // n=100
pub struct TypeStat { pub ext: String, pub files: u64, pub size: u64, pub allocated: u64 }
pub fn file_types(tree: &Tree, scope: NodeId) -> Vec<TypeStat>;                    // nach size absteigend
pub fn age_histogram(tree: &Tree, scope: NodeId, now: FileTime) -> [u64; 6];       // <1M,<3M,<6M,<1J,<3J,>=3J (Bytes)

// filter.rs
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Filter { pub name_glob: Option<String>, pub min_size: Option<u64>, pub max_size: Option<u64>,
    pub older_than_days: Option<u32>, pub newer_than_days: Option<u32>, pub age_field: AgeField,
    pub extensions: Vec<String>, pub files_only: bool, pub include_hidden: bool }
pub enum AgeField { Modified, Accessed, Created }
impl Filter { pub fn matches(&self, tree: &Tree, id: NodeId, now: FileTime) -> bool; }
pub fn search(tree: &Tree, scope: NodeId, f: &Filter, now: FileTime, limit: usize) -> Vec<NodeId>;

// duplicates.rs
pub trait FileReader: Sync { fn read_head(&self, path: &str, n: usize) -> io::Result<Vec<u8>>;
                             fn hash_full(&self, path: &str, cancel: &AtomicBool) -> io::Result<u128>; }
pub struct StdFileReader;   // std::fs + xxh3
pub struct DupGroup { pub size: u64, pub hash: u128, pub files: Vec<NodeId> }
pub fn find_duplicates(tree: &Tree, scope: NodeId, min_size: u64, reader: &dyn FileReader,
                       progress: &dyn Fn(u64, u64), cancel: &AtomicBool) -> Vec<DupGroup>;  // nach Einsparpotenzial sortiert

// snapshot.rs
pub fn save(tree: &Tree, path: &Path) -> Result<(), SnapError>;   // Magic "JTSCAN01", Version-u16, lz4(postcard(Tree))
pub fn load(path: &Path) -> Result<Tree, SnapError>;

// delta.rs
pub enum ChangeKind { Added, Removed, Grown, Shrunk }
pub struct DeltaEntry { pub path: String, pub kind: ChangeKind, pub old_size: u64, pub new_size: u64, pub is_dir: bool }
pub fn compare(old: &Tree, new: &Tree, min_change: u64) -> Vec<DeltaEntry>;   // nach |Δ| absteigend

// history.rs
pub struct HistoryPoint { pub time: FileTime, pub size: u64, pub allocated: u64, pub files: u64 }
pub fn series_for_path(snapshots: &[Tree], path: &str) -> Vec<HistoryPoint>;  // nach Zeit sortiert
pub fn top_growers(snapshots: &[Tree], depth: u32, n: usize) -> Vec<(String, i64)>;

// export/mod.rs
pub enum ExportFormat { Csv, Xlsx, Html, Pdf }
pub enum ExportContent { Tree { root: NodeId, max_depth: u32 }, TopFiles(Vec<NodeId>),
                         FileTypes(Vec<TypeStat>), Duplicates(Vec<DupGroup>), Delta(Vec<DeltaEntry>) }
pub struct ExportRequest<'a> { pub tree: Option<&'a Tree>, pub content: ExportContent, pub lang: Lang, pub title: String }
pub fn export(req: &ExportRequest, fmt: ExportFormat, out: &mut dyn Write) -> Result<(), ExportError>;
pub(crate) struct Table { pub headers: Vec<String>, pub rows: Vec<Vec<Cell>> }  // gemeinsam für alle Formate
pub(crate) enum Cell { Text(String), Bytes(u64), Int(i64), Percent(f32), Time(FileTime) }

// i18n.rs
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)] pub enum Lang { De, En }  // Default De
#[derive(Clone, Copy, PartialEq, Eq, Debug)] pub enum Key { AppTitle, MenuScan, ColName, ColSize, ... }
pub fn tr(lang: Lang, key: Key) -> &'static str;   // eine große match-Tabelle: Key => (de, en)
pub fn all_keys() -> &'static [Key];               // für Vollständigkeitstest

// format.rs
pub fn bytes(n: u64, lang: Lang) -> String;  pub fn time(t: FileTime, lang: Lang) -> String;  pub fn number(n: u64, lang: Lang) -> String;

// scan/mod.rs
pub struct ScanRequest { pub root: String, pub prefer_mft: bool, pub follow_reparse: bool /*immer false in UI*/ }
#[derive(Clone, Default)] pub struct ScanProgress { pub entries: u64, pub bytes: u64, pub current: String, pub phase: Phase }
pub fn scan(req: &ScanRequest, progress: &dyn Fn(&ScanProgress), cancel: &AtomicBool) -> Result<Tree, ScanError>;
//   Logik: prefer_mft && platform::is_admin() && Laufwerk lokal && NTFS -> mft; bei ERR -> walk (meta.mode = Walk)
pub(crate) fn scan_walk(req, progress, cancel) -> Result<Tree, ScanError>;
#[cfg(windows)] pub(crate) fn scan_mft(req, progress, cancel) -> Result<Tree, ScanError>;

// scan/mft_parse.rs  (plattformneutral, testbar)
pub struct MftRecord { pub record_no: u64, pub parent_ref: u64, pub name: String, pub is_dir: bool,
                       pub in_use: bool, pub size: u64, pub allocated: u64, pub times: [FileTime;3],
                       pub flags: NodeFlags, pub ads_size: u64, pub base_record: u64 }
pub fn apply_fixup(buf: &mut [u8], bytes_per_sector: usize) -> Result<(), MftError>;
pub fn parse_record(buf: &[u8], record_no: u64) -> Result<Option<MftRecord>, MftError>;
pub fn parse_runlist(data: &[u8]) -> Result<Vec<(u64 /*lcn*/, u64 /*len*/)>, MftError>;
pub fn records_to_tree(records: impl Iterator<Item = MftRecord>, meta: ScanMeta, root_name: &str) -> Tree;

// platform/mod.rs  – auf Nicht-Windows Stubs (is_admin=false, Rest -> Err(Unsupported))
pub fn is_admin() -> bool;
pub fn move_to_recycle_bin(paths: &[String]) -> io::Result<()>;
pub fn delete_permanently(paths: &[String]) -> io::Result<()>;
pub fn open_in_explorer(path: &str) -> io::Result<()>;        // explorer /select,"<pfad>"
pub struct SecurityInfo { pub owner: String, pub aces: Vec<Ace> }
pub struct Ace { pub principal: String, pub allow: bool, pub rights: String, pub inherited: bool }
pub fn security_info(path: &str) -> io::Result<SecurityInfo>;
pub struct StreamInfo { pub name: String, pub size: u64 }
pub fn alternate_streams(path: &str) -> io::Result<Vec<StreamInfo>>;
pub fn volume_info(root: &str) -> io::Result<(String /*fs*/, u32 /*cluster*/, u64 /*total*/, u64 /*free*/)>;
pub fn long_path(path: &str) -> String;                       // "\\?\C:\..." bzw. "\\?\UNC\server\share\..."
pub fn list_drives() -> Vec<String>;
```

Lösch-Regel steht im Code an **einer** Stelle: `jt_gui::app::can_delete() = platform::is_admin()`. Ist der Wert false, werden die Menüpunkte gar nicht erst gerendert.

---

## 3. Gewählte Crates

Versionen am 2026-10-30 im crates.io-Index geprüft. Probe-`cargo check` auf Linux mit allen Crates: OK.

| Crate | Version | Wo | Begründung |
|---|---|---|---|
| eframe / egui / egui_extras | 0.37 | gui | Vorgabe. egui_extras liefert die virtuelle Tabelle (Millionen Zeilen). Features: `default-features=false, ["default_fonts","wgpu","accesskit"]`, für Linux-Dev zusätzlich `x11`,`wayland` (target-cfg). |
| rfd | 0.16 | gui | nativer Datei-Dialog (Speichern/Öffnen von Export und Snapshot) |
| serde (+derive), serde_json | 1 | core/gui | Snapshot, Einstellungen |
| postcard | 1 (`use-std`) | core | kompaktes Binärformat, wenige Abhängigkeiten (statt bincode 3, dessen API sich gerade ändert) |
| lz4_flex | 0.14 | core | reines Rust, sehr schnell, keine C-Abhängigkeit (statt zstd) |
| xxhash-rust | 0.8 (`xxh3`) | core | schnellster Hash für Duplikate, reines Rust. Kein Krypto nötig. |
| rayon | 1.12 | core | paralleler Walk, paralleles Hashen |
| crossbeam-channel | 0.5 | gui | Fortschritt und Ergebnisse von Hintergrund-Threads |
| bitflags | 2 | core | NodeFlags |
| globset | 0.4 | core | Name-Glob im Filter |
| chrono | 0.4 (ohne default, `std`,`clock`) | core | Datumsformat |
| rust_xlsxwriter | 0.102 (`default-features=false`) | core | xlsx ohne LibreOffice/C, braucht nur `zip` |
| pdf-writer | 0.15 | core | sehr klein (4 Abhängigkeiten), Low-Level-PDF. Für reine Tabellen reicht der Standard-Font Helvetica. Umlaute über WinAnsi-Encoding. printpdf wäre schwerer. |
| csv | — | — | **nicht** verwendet. CSV wird selbst geschrieben (20 Zeilen, Quoting nach RFC 4180). |
| windows | 0.62 (Features gezielt: Win32_Storage_FileSystem, Win32_Security, Win32_Security_Authorization, Win32_System_Ioctl, Win32_System_IO, Win32_UI_Shell, Win32_Foundation, Win32_System_Threading) | core, nur `[target.'cfg(windows)'.dependencies]` | offiziell, kommt über wgpu ohnehin ins Paket, also keine Zusatzkosten |
| embed-manifest | 1.5 | gui build-dep | Manifest (longPathAware, DPI) ohne rc.exe. Läuft unter MSVC und GNU. |
| tempfile (dev) | 3 | Tests | Test-Verzeichnisbäume |

Bewusst **nicht** verwendet:
- `trash`: Papierkorb wird über `IFileOperation`/`SHFileOperationW` mit `FOFX_RECYCLEONDELETE` selbst angesprochen. Das spart eine Abhängigkeit und funktioniert auch mit `\\?\`-Pfaden.
- `ntfs`/`mft`-Crates: Der eigene Parser ist schneller (nur die benötigten Attribute) und bleibt testbar.

---

## 4. Bauaufträge (5, parallel nach Schritt A0)

**A0 (Pflicht, zuerst, ca. 1 h, gehört zu Auftrag A):** Workspace, beide Cargo.toml, alle Dateien aus Abschnitt 1 mit Kopfkommentar, alle öffentlichen Signaturen aus Abschnitt 2 als Skelett (`todo!()`), `cargo check` grün. Danach starten B–E parallel. Jeder Auftrag ändert nur **seine** Dateien. Fehlt etwas an einer Schnittstelle, wird es gemeldet statt fremde Dateien anzufassen.

### Auftrag A – Kern-Datenmodell, Aggregation, Statistik, Filter, Snapshot
- **Dateien:** `Cargo.toml` (Workspace), `crates/jt_core/Cargo.toml`, `lib.rs`, `model.rs`, `aggregate.rs`, `stats.rs`, `filter.rs`, `snapshot.rs`, `format.rs`, `tests/branding.rs`, `tests/core_*.rs`
- **Schnittstelle:** Abschnitt 2.1 und 2.2 (model, aggregate, stats, filter, snapshot, format). Dazu ein Test-Helfer `model::testutil::sample_tree()` (pub, `#[doc(hidden)]`), den alle anderen Aufträge benutzen.
- **Fertig, wenn:** `cargo test -p jt_core` grün mit mindestens diesen Tests: (1) Aggregation eines 3-Ebenen-Baums ergibt korrekte Summen für size/allocated/file_count/dir_count; (2) `full_path`/`find_path` (case-insensitiv); (3) `top_files(100)` sortiert absteigend und liefert nur Dateien; (4) Filter für Größe, Alter, Endung und Glob (je 1 positiver und 1 negativer Fall); (5) Snapshot-Roundtrip `save`→`load` ist gleich, bei falschem Magic kommt ein Fehler; (6) `format::bytes(1536, De) == "1,5 KiB"`; (7) Branding-Test: jede `*.rs` im Repo enthält "@ Joschy" in den ersten 3 Zeilen. Dazu `cargo clippy -- -D warnings` sauber.

### Auftrag B – Scanner (Walk, MFT-Parser, MFT-Windows) und Plattform-Schicht
- **Dateien:** `scan/mod.rs`, `scan/walk.rs`, `scan/mft_parse.rs`, `scan/mft_win.rs`, `platform/mod.rs`, `platform/win.rs`, `tests/scan_*.rs`, `tests/data/mft_*.bin` (kleine, selbst erzeugte Records)
- **Schnittstelle:** `scan::scan`, `mft_parse::*`, alle `platform::*` (Abschnitt 2.2). Nutzt nur `TreeBuilder`.
- **Vorgaben:** Walk mit rayon (pro Unterordner eine Aufgabe), Reparse-Points nicht verfolgen, Zugriffsfehler zählen und `ACCESS_DENIED` setzen, nie abbrechen. Unter Windows ausschließlich `\\?\`-Pfade (lange Pfade). MFT: `CreateFileW("\\\\.\\C:")`, `FSCTL_GET_NTFS_VOLUME_DATA`, `$MFT` über die eigene Runlist in 1-MiB-Blöcken lesen, Records parallel parsen, `$ATTRIBUTE_LIST` für Basis-Records berücksichtigen, Hardlinks nur einmal zählen (erster Name). Fallback-Kette: nicht Admin / kein NTFS / UNC / Fehler → Walk.
- **Fertig, wenn:** Linux: `cargo test -p jt_core scan` grün mit (1) Walk über ein tempfile-Verzeichnis (20 Dateien, 3 Ebenen, ein Symlink wird nicht verfolgt) ergibt die erwarteten Größen und Zähler; (2) `apply_fixup` + `parse_record` auf einem hand-erzeugten 1024-Byte-Record (Datei mit $FILE_NAME + residentem $DATA + einem ADS) liefern Name, Größe, Parent und ads_size korrekt; (3) `parse_runlist` für 3 bekannte Byte-Folgen (inkl. negativer Offsets und sparse); (4) `records_to_tree` verknüpft Waisen korrekt; (5) Abbruch über `cancel` beendet Walk in unter 1 s. Windows (CI): `cargo clippy --target x86_64-pc-windows-msvc -- -D warnings` grün.

### Auftrag C – Duplikate, Delta, Verlauf
- **Dateien:** `duplicates.rs`, `delta.rs`, `history.rs`, `tests/dup_*.rs`, `tests/delta_*.rs`
- **Schnittstelle:** wie Abschnitt 2.2. `FileReader`-Trait, damit Tests ohne echte Dateien laufen können (Mock).
- **Fertig, wenn:** Tests grün: (1) 3 gleiche + 2 gleich große, aber unterschiedliche Dateien (echte tempfiles) ergeben genau 1 Gruppe mit 3 Dateien; (2) Dateien unter `min_size` werden ignoriert; (3) Mock-Reader zählt Aufrufe: Dateien mit eindeutiger Größe werden nie gelesen; (4) `cancel` liefert ein Teilergebnis ohne Panik; (5) `compare` erkennt Added/Removed/Grown/Shrunk in einem konstruierten Baum-Paar, Sortierung nach |Δ|; (6) `series_for_path` über 3 Snapshots liefert 3 zeitlich sortierte Punkte, fehlender Pfad ergibt size 0.

### Auftrag D – i18n und Exporte (CSV, XLSX, HTML, PDF)
- **Dateien:** `i18n.rs`, `export/mod.rs`, `export/csv.rs`, `export/xlsx.rs`, `export/html.rs`, `export/pdf.rs`, `tests/export_*.rs`, `tests/i18n.rs`
- **Schnittstelle:** `i18n::{Lang, Key, tr, all_keys}`, `export::export`. Die Key-Liste deckt alle Texte der GUI ab. D legt sie an und pflegt sie. E meldet fehlende Keys an D, bei Engpass ergänzt E sie direkt am Tabellenende (die einzige erlaubte Ausnahme, Konflikt trivial).
- **Vorgaben:** HTML ist eine einzelne Datei mit inline CSS, ohne externe Ressourcen, HTML-escaped, Fuß "Erstellt mit Joschys_Tree @ Joschy · <Datum>". PDF in A4 quer, Seitenumbruch, Kopfzeile auf jeder Seite, Fuß "@ Joschy · Seite n/m". XLSX mit Spaltenformaten (Bytes als Zahl, Datum als Datum), AutoFilter, fixierter Kopfzeile. CSV: UTF-8-BOM, Trennzeichen `;` (de) bzw. `,` (en).
- **Fertig, wenn:** Tests grün: (1) `all_keys()` hat für jeden Key nicht-leere de- und en-Texte; (2) CSV mit Feld `a;"b` wird korrekt gequotet und beginnt mit BOM; (3) HTML enthält "@ Joschy" und escaped `<script>`; (4) XLSX-Bytes beginnen mit `PK`, Größe über 0, und `zip`-Lesen zeigt `xl/worksheets/sheet1.xml`; (5) PDF beginnt mit `%PDF-`, enthält "@ Joschy" (unkomprimierter Content-Stream im Test-Modus) und hat bei 500 Zeilen mehr als 1 Seite; (6) Umlaute "Größe" landen in HTML, CSV und XLSX unverändert.

### Auftrag E – GUI (eframe), Theme, Einstellungen, Treemap
- **Dateien:** alles unter `crates/jt_gui/` (main, app, theme, settings, views/*, dialogs, footer, build.rs)
- **Schnittstelle:** konsumiert nur `jt_core`. Scan, Duplikate und Export laufen in `std::thread` mit `crossbeam-channel`, die UI blockiert nie. Treemap-Layout als reine Funktion `fn squarify(items: &[(NodeId,u64)], rect: Rect) -> Vec<(NodeId, Rect)>` mit Unit-Test.
- **Vorgaben:** Fenstertitel `"Joschys_Tree @ Joschy – <Ziel>"`. Fußzeile immer sichtbar mit "@ Joschy", Modus (MFT/Walk), Admin/Nur-Lesen. Standard: Dunkel und Deutsch, umschaltbar im Menü, Einstellung wird gespeichert. Ziel-Auswahl: Laufwerksliste + freies Feld (UNC). Tabs: Baum | Treemap | Top 100 | Dateitypen | Suche | Duplikate | Vergleich/Verlauf. Rechtsklick: Im Explorer öffnen, Pfad kopieren, (nur Admin) In Papierkorb, (nur Admin) Endgültig löschen → Dialog 1 → zweiter Dialog mit Tippen von "LÖSCHEN"/"DELETE". Nach dem Löschen Knoten entfernen und `aggregate::recompute`. Detailpanel lädt Owner/ACL/ADS bei Bedarf (lazy).
- **Fertig, wenn:** (1) `cargo build -p jt_gui` auf Linux grün; (2) `cargo test -p jt_gui` grün (squarify: Summe der Flächen gleich Gesamtfläche ±0,1 %, keine Überlappung; `can_delete()==false` auf Linux; Settings-Roundtrip); (3) kein `tr()`-Literal fehlt: grep-Test, der keine deutschen/englischen String-Literale in `views/` außer über `tr()` findet (Whitelist für Formatstrings); (4) im Windows-CI-Build entsteht `JoschysTree.exe`.

### Auftrag F – Repo-Rahmen: CI, README, LICENSE, BUILD
- **Dateien:** `.github/workflows/build.yml`, `README.md`, `BUILD.md`, `LICENSE`, `rust-toolchain.toml`, `.gitignore`
- **Vorgaben CI:** zwei Jobs. `linux-test` (ubuntu-latest: fmt-check, clippy, `cargo test --workspace`) und `windows-build` (windows-latest, Target `x86_64-pc-windows-msvc`, `RUSTFLAGS=-C target-feature=+crt-static` für eine exe ohne VC-Runtime, `cargo clippy` + `cargo build --release`, `actions/upload-artifact` mit `JoschysTree.exe`). Bei Tag `v*` zusätzlich ein Release mit der exe (`softprops/action-gh-release`). Cache `Swatinem/rust-cache`. Actions mit fester Version (Tag).
- **LICENSE:** offizieller AGPL-3.0-Volltext (lokal z. B. aus npm-Paket `spdx-license-list`, Datei `licenses/AGPL-3.0-only.json`, weil gnu.org hier gesperrt ist).
- **README:** Zweck, Funktionen, Screenshot-Platzhalter, Hinweis Admin = MFT + Löschen, Nur-Lese-Modus, Lizenz, "@ Joschy".
- **Fertig, wenn:** `build.yml` ist gültiges YAML (python `yaml.safe_load`), enthält beide Jobs und den Upload-Schritt; LICENSE beginnt mit "GNU AFFERO GENERAL PUBLIC LICENSE" / "Version 3, 19 November 2007"; README und BUILD enthalten "@ Joschy". Endgültig fertig erst, wenn nach dem ersten Push beide CI-Jobs grün sind.

**Reihenfolge:** A0 → {A, B, C, D, E, F parallel} → Integration (E bindet alles an) → Windows-CI grün → manuelle Abnahme auf Windows.

---

## 5. Teststrategie

**Linux, automatisch (`cargo test --workspace`, auch im CI-Job `linux-test`):**
- model/aggregate/stats/filter/snapshot/format (A), Walk-Scanner auf tempfile-Bäumen und MFT-Parser auf Byte-Fixtures (B), Duplikate/Delta/Verlauf (C), i18n-Vollständigkeit und alle 4 Exportformate strukturell (D), squarify, Settings, `can_delete()==false` (E), Branding-Test über alle `.rs` (A).
- `cargo clippy --workspace -- -D warnings`, `cargo fmt --check`.

**Windows CI (automatisch):** clippy + Release-Build der `cfg(windows)`-Teile. `cargo test -p jt_core` läuft dort auch, dazu ein Windows-Test, der `scan` mit `prefer_mft=false` über `%TEMP%` laufen lässt.

**Windows, manuell (Checkliste in BUILD.md):**
1. Ohne Admin starten: Walk-Modus, keine Lösch-Menüpunkte, Fußzeile zeigt "Nur-Lesen".
2. Als Admin C:\ scannen: MFT-Modus, Zeit notieren (Ziel: unter 15 s bei 1 Mio. Dateien auf SSD). Summe mit Explorer-Eigenschaften vergleichen (±1 %).
3. `\\server\share` und `\\pc\c$` scannen: automatischer Walk-Fallback.
4. FAT32-/exFAT-USB-Stick: Fallback.
5. Pfad über 300 Zeichen anlegen: wird gefunden, Export enthält ihn, Explorer-Öffnen funktioniert.
6. Datei mit ADS (`echo x > a.txt:geheim`): Detailpanel zeigt den Stream.
7. Owner/ACL eines Systemordners wird angezeigt.
8. Löschen: Papierkorb funktioniert. Endgültig nur nach zweiter Bestätigung.
9. Theme- und Sprachwechsel, Neustart behält die Einstellung.
10. Exporte in Excel, Browser und PDF-Reader öffnen: "@ Joschy" im Fuß.
11. Snapshot speichern, Dateien ändern, neu scannen, Delta stimmt.
12. Server 2016 (RDP, ohne GPU): exe startet (wgpu/DX12-WARP bzw. Fallback).

---

## 6. Risiken und offene Punkte (selbst entschieden)

| # | Risiko / Frage | Entscheidung |
|---|---|---|
| 1 | Kein Cross-Compile hier, `cfg(windows)` lokal nicht prüfbar | CI auf `windows-latest` ist die einzige Prüfinstanz für Windows-Code. Windows-Code klein halten, Logik (MFT-Parser, Pfad-Umwandlung) plattformneutral testbar machen. |
| 2 | Grafik-Backend auf Servern/RDP/VMs ohne GPU: OpenGL (glow) fällt oft auf GDI 1.1 zurück und startet nicht | **wgpu** (DX12 mit WARP-Software-Fallback). glow nicht einkompiliert. Kostet ca. 3–5 MB exe-Größe, dafür startet es zuverlässig auf Server 2016/2025. |
| 3 | exe-Größe | Ziel unter 15 MB (lto, strip, opt-level 3). Kein UPX (Virenscanner-Fehlalarme). |
| 4 | MSVC oder GNU-Target | **MSVC** im CI mit `+crt-static`. Ergibt eine portable exe ohne Runtime-DLLs, der Standard für wgpu/windows-rs. |
| 5 | Admin-Rechte: Manifest `requireAdministrator`? | **`asInvoker`**. Das Tool startet immer, Admin über "Als Administrator ausführen". In der GUI gibt es den Knopf "Als Admin neu starten" (`ShellExecuteW` mit `runas`). Erfüllt die Vorgabe "ohne Admin = Nur-Lesen". |
| 6 | Hardlinks und Reparse-Points doppelt gezählt | Hardlink nur beim ersten Namen zählen. Junctions/Symlinks werden nie verfolgt, nur als `REPARSE` angezeigt. |
| 7 | Belegter Speicher im Walk-Modus | Lokal: `GetCompressedFileSizeW` gerundet auf Clustergröße (`GetDiskFreeSpaceW`). UNC: auf Clustergröße des Shares runden, falls abfragbar, sonst `allocated = size`. |
| 8 | MFT-Scan liefert kein "Besitzer/ACL" | Owner/ACL werden **lazy** im Detailpanel und optional als Spalte für sichtbare Zeilen geladen, nicht beim Scan (sonst zu langsam). |
| 9 | ADS im Walk-Modus teuer (`FindFirstStreamW` pro Datei) | MFT-Modus: kostenlos aus Records. Walk: Option "ADS erfassen" (Standard aus), sonst nur lazy im Detailpanel. |
| 10 | Letzter Zugriff ist oft deaktiviert (NtfsDisableLastAccessUpdate) | Wird angezeigt wie gelesen. Tooltip-Hinweis, dass Windows den Wert evtl. nicht pflegt. |
| 11 | PDF-Schrift und Umlaute | Standard-Font Helvetica mit WinAnsiEncoding (deckt de/en ab). Kein Font-Embedding. Zeichen außerhalb von WinAnsi werden durch `?` ersetzt (nur im PDF). |
| 12 | RAM bei sehr großen Volumes (über 10 Mio. Dateien) | Kompakte Node-Struktur (Abschnitt 2.1). Namen als `Box<str>`. Mögliche spätere Optimierung: String-Arena. Für v1 akzeptiert. |
| 13 | Snapshot-Format-Änderungen | Header mit Versionsnummer. Bei neuer Version wird ein alter Snapshot mit klarer Meldung abgelehnt (keine Migration in v1). |
| 14 | Verlauf braucht mehrere Snapshots | Verlauf = Ordner wählen, alle `.jtscan` desselben `root_path` werden geladen. Keine Datenbank, kein Hintergrunddienst. |
| 15 | Codesignierung | Keine. Mögliche SmartScreen-Warnung steht in der README. |
| 16 | Lizenz-Kennung | `AGPL-3.0-or-later` in Cargo.toml und den SPDX-Köpfen. LICENSE-Datei mit dem Volltext v3. |
| 17 | Kommandozeile | Keine CLI. `main.rs` wertet keine Argumente aus. |
