// Joschys_Tree – Sprachen und zentrale Texttabelle de/en – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Zentrale Texttabelle. Jeder Key steht genau einmal in `keys!` mit (de, en).
//! Neue Keys einfach am Tabellenende anhängen; Enum, `all_keys()` und `tr()`
//! werden daraus erzeugt. Platzhalter wie `{n}` ersetzt der Aufrufer.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default, Serialize, Deserialize)]
pub enum Lang {
    #[default]
    De,
    En,
}

impl Lang {
    pub const ALL: [Lang; 2] = [Lang::De, Lang::En];

    /// Sprachkürzel ("de"/"en"), z. B. für HTML `lang` oder Einstellungen.
    pub fn code(self) -> &'static str {
        match self {
            Lang::De => "de",
            Lang::En => "en",
        }
    }

    /// Anzeigename der Sprache in ihrer eigenen Sprache.
    pub fn native_name(self) -> &'static str {
        match self {
            Lang::De => "Deutsch",
            Lang::En => "English",
        }
    }
}

macro_rules! keys {
    ($($k:ident => ($de:expr, $en:expr),)*) => {
        /// Alle GUI-Texte. Auftrag D pflegt die Liste.
        #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
        pub enum Key { $($k,)* }

        const ALL_KEYS: &[Key] = &[$(Key::$k,)*];

        /// Eine große match-Tabelle: Key => (de, en).
        pub fn tr(lang: Lang, key: Key) -> &'static str {
            let (de, en): (&'static str, &'static str) = match key {
                $(Key::$k => ($de, $en),)*
            };
            match lang {
                Lang::De => de,
                Lang::En => en,
            }
        }
    };
}

keys! {
    // ---- Allgemein / Branding ----
    AppTitle => ("Joschys_Tree @ Joschy", "Joschys_Tree @ Joschy"),
    AppName => ("Joschys_Tree", "Joschys_Tree"),
    Branding => ("@ Joschy", "@ Joschy"),
    Ok => ("OK", "OK"),
    Cancel => ("Abbrechen", "Cancel"),
    Close => ("Schließen", "Close"),
    Yes => ("Ja", "Yes"),
    No => ("Nein", "No"),
    Apply => ("Übernehmen", "Apply"),
    Reset => ("Zurücksetzen", "Reset"),
    Save => ("Speichern", "Save"),
    Open => ("Öffnen", "Open"),
    Browse => ("Durchsuchen …", "Browse …"),
    Start => ("Start", "Start"),
    Stop => ("Stopp", "Stop"),
    Refresh => ("Aktualisieren", "Refresh"),
    Loading => ("Wird geladen …", "Loading …"),
    Working => ("Wird bearbeitet …", "Working …"),
    Done => ("Fertig", "Done"),
    Unknown => ("unbekannt", "unknown"),
    None => ("keine", "none"),
    All => ("Alle", "All"),
    Copy => ("Kopieren", "Copy"),

    // ---- Menü ----
    MenuFile => ("Datei", "File"),
    MenuScan => ("Scannen", "Scan"),
    MenuScanDrive => ("Laufwerk scannen", "Scan drive"),
    MenuScanFolder => ("Ordner scannen …", "Scan folder …"),
    MenuRescan => ("Erneut scannen", "Rescan"),
    MenuStopScan => ("Scan abbrechen", "Stop scan"),
    MenuOpenSnapshot => ("Snapshot öffnen …", "Open snapshot …"),
    MenuSaveSnapshot => ("Snapshot speichern …", "Save snapshot …"),
    MenuExport => ("Exportieren …", "Export …"),
    MenuExit => ("Beenden", "Exit"),
    MenuView => ("Ansicht", "View"),
    MenuTheme => ("Design", "Theme"),
    ThemeDark => ("Dunkel", "Dark"),
    ThemeLight => ("Hell", "Light"),
    MenuLanguage => ("Sprache", "Language"),
    LangGerman => ("Deutsch", "German"),
    LangEnglish => ("Englisch", "English"),
    MenuTools => ("Extras", "Tools"),
    MenuFindDuplicates => ("Duplikate suchen", "Find duplicates"),
    MenuCompare => ("Mit Snapshot vergleichen …", "Compare with snapshot …"),
    MenuHistory => ("Verlauf laden …", "Load history …"),
    MenuRestartAsAdmin => ("Als Administrator neu starten", "Restart as administrator"),
    MenuHelp => ("Hilfe", "Help"),
    MenuAbout => ("Über Joschys_Tree", "About Joschys_Tree"),
    MenuShowHidden => ("Versteckte Dateien anzeigen", "Show hidden files"),
    MenuShowAllocated => ("Belegten Speicher anzeigen", "Show allocated size"),
    MenuSettings => ("Einstellungen …", "Settings …"),

    // ---- Ziel-Auswahl ----
    TargetLabel => ("Ziel:", "Target:"),
    TargetDrives => ("Laufwerke", "Drives"),
    TargetPathHint => ("Pfad oder UNC-Freigabe, z. B. \\\\server\\freigabe", "Path or UNC share, e.g. \\\\server\\share"),
    TargetRecent => ("Zuletzt verwendet", "Recent"),
    TargetPreferMft => ("MFT verwenden (schnell, nur Admin/NTFS)", "Use MFT (fast, admin/NTFS only)"),
    TargetScanAds => ("Alternative Datenströme erfassen (langsamer)", "Collect alternate data streams (slower)"),
    NoScanYet => ("Noch kein Scan. Ziel wählen und \"Scannen\" klicken.", "No scan yet. Choose a target and click \"Scan\"."),

    // ---- Tabs ----
    TabTree => ("Baum", "Tree"),
    TabTreemap => ("Treemap", "Treemap"),
    TabTopFiles => ("Top 100", "Top 100"),
    TabFileTypes => ("Dateitypen", "File types"),
    TabSearch => ("Suche", "Search"),
    TabDuplicates => ("Duplikate", "Duplicates"),
    TabDelta => ("Vergleich/Verlauf", "Compare/History"),
    TabDetails => ("Details", "Details"),

    // ---- Spaltenköpfe (GUI und Export) ----
    ColName => ("Name", "Name"),
    ColPath => ("Pfad", "Path"),
    ColSize => ("Größe", "Size"),
    ColAllocated => ("Belegt", "Allocated"),
    ColPercent => ("Anteil", "Percent"),
    ColPercentOfParent => ("% vom Ordner", "% of parent"),
    ColFiles => ("Dateien", "Files"),
    ColFolders => ("Ordner", "Folders"),
    ColModified => ("Geändert", "Modified"),
    ColCreated => ("Erstellt", "Created"),
    ColAccessed => ("Letzter Zugriff", "Last accessed"),
    ColAttributes => ("Attribute", "Attributes"),
    ColOwner => ("Besitzer", "Owner"),
    ColExtension => ("Endung", "Extension"),
    ColType => ("Typ", "Type"),
    ColRank => ("Rang", "Rank"),
    ColAds => ("Datenströme", "Streams"),
    ColGroup => ("Gruppe", "Group"),
    ColHash => ("Prüfsumme", "Hash"),
    ColCount => ("Anzahl", "Count"),
    ColSavings => ("Einsparung", "Savings"),
    ColChange => ("Änderung", "Change"),
    ColOldSize => ("Alte Größe", "Old size"),
    ColNewSize => ("Neue Größe", "New size"),
    ColDifference => ("Differenz", "Difference"),
    ColTime => ("Zeitpunkt", "Time"),
    ColGrowth => ("Wachstum", "Growth"),
    ColAge => ("Alter", "Age"),
    TypeFolder => ("Ordner", "Folder"),
    TypeFile => ("Datei", "File"),
    NoExtension => ("(ohne Endung)", "(no extension)"),

    // ---- Attribute ----
    AttrHidden => ("Versteckt", "Hidden"),
    AttrSystem => ("System", "System"),
    AttrReadonly => ("Schreibgeschützt", "Read-only"),
    AttrReparse => ("Verknüpfung (nicht verfolgt)", "Link (not followed)"),
    AttrCompressed => ("Komprimiert", "Compressed"),
    AttrSparse => ("Sparse", "Sparse"),
    AttrHasAds => ("Mit Datenströmen", "Has streams"),
    AttrAccessDenied => ("Zugriff verweigert", "Access denied"),
    AttrOffline => ("Offline", "Offline"),

    // ---- Kontextmenü ----
    CtxOpenExplorer => ("Im Explorer öffnen", "Open in Explorer"),
    CtxCopyPath => ("Pfad kopieren", "Copy path"),
    CtxRecycle => ("In den Papierkorb", "Move to Recycle Bin"),
    CtxDeletePermanently => ("Endgültig löschen …", "Delete permanently …"),
    CtxShowDetails => ("Details anzeigen", "Show details"),
    CtxScanThisFolder => ("Diesen Ordner scannen", "Scan this folder"),
    CtxExpandAll => ("Alle aufklappen", "Expand all"),
    CtxCollapseAll => ("Alle zuklappen", "Collapse all"),

    // ---- Löschdialog (2-stufig) ----
    DelTitle => ("Endgültig löschen", "Delete permanently"),
    DelStep1Text => ("Folgende Einträge werden endgültig gelöscht und landen NICHT im Papierkorb:", "The following items will be deleted permanently and will NOT go to the Recycle Bin:"),
    DelStep1Count => ("{n} Einträge, zusammen {size}", "{n} items, {size} in total"),
    DelContinue => ("Weiter …", "Continue …"),
    DelStep2Title => ("Letzte Bestätigung", "Final confirmation"),
    DelStep2Text => ("Zum Bestätigen bitte LÖSCHEN eintippen:", "To confirm, please type DELETE:"),
    DelConfirmWord => ("LÖSCHEN", "DELETE"),
    DelConfirmButton => ("Endgültig löschen", "Delete permanently"),
    DelWrongWord => ("Eingabe stimmt nicht überein.", "Input does not match."),
    DelRecycleTitle => ("In den Papierkorb verschieben", "Move to Recycle Bin"),
    DelRecycleText => ("{n} Einträge in den Papierkorb verschieben?", "Move {n} items to the Recycle Bin?"),
    DelDone => ("{n} Einträge gelöscht.", "{n} items deleted."),
    DelRecycled => ("{n} Einträge in den Papierkorb verschoben.", "{n} items moved to the Recycle Bin."),
    DelFailed => ("Löschen fehlgeschlagen: {err}", "Delete failed: {err}"),
    DelAdminOnly => ("Löschen ist nur als Administrator möglich.", "Deleting is only possible as administrator."),

    // ---- Export-Dialog ----
    ExpTitle => ("Exportieren", "Export"),
    ExpFormat => ("Format", "Format"),
    ExpFormatCsv => ("CSV (Text)", "CSV (text)"),
    ExpFormatXlsx => ("Excel (XLSX)", "Excel (XLSX)"),
    ExpFormatHtml => ("HTML-Bericht", "HTML report"),
    ExpFormatPdf => ("PDF-Bericht", "PDF report"),
    ExpContent => ("Inhalt", "Content"),
    ExpContentTree => ("Baum", "Tree"),
    ExpContentTopFiles => ("Top-100-Dateien", "Top 100 files"),
    ExpContentFileTypes => ("Dateitypen", "File types"),
    ExpContentDuplicates => ("Duplikate", "Duplicates"),
    ExpContentDelta => ("Vergleich", "Comparison"),
    ExpMaxDepth => ("Maximale Tiefe", "Maximum depth"),
    ExpReportTitle => ("Berichtstitel", "Report title"),
    ExpButton => ("Exportieren", "Export"),
    ExpRunning => ("Export läuft …", "Exporting …"),
    ExpDone => ("Export gespeichert: {path}", "Export saved: {path}"),
    ExpFailed => ("Export fehlgeschlagen: {err}", "Export failed: {err}"),
    ExpCreatedWith => ("Erstellt mit Joschys_Tree @ Joschy", "Created with Joschys_Tree @ Joschy"),
    ExpPage => ("Seite", "Page"),
    ExpScanRoot => ("Ziel", "Target"),
    ExpScanDate => ("Scan vom", "Scanned on"),
    ExpRows => ("Zeilen", "Rows"),
    ExpTitleTree => ("Ordnerstruktur", "Folder structure"),
    ExpTitleTopFiles => ("Größte Dateien", "Largest files"),
    ExpTitleFileTypes => ("Dateitypen", "File types"),
    ExpTitleDuplicates => ("Doppelte Dateien", "Duplicate files"),
    ExpTitleDelta => ("Änderungen", "Changes"),

    // ---- Suche / Filter ----
    FltTitle => ("Filter", "Filter"),
    FltName => ("Name (Platzhalter * ?)", "Name (wildcards * ?)"),
    FltMinSize => ("Mindestgröße", "Minimum size"),
    FltMaxSize => ("Höchstgröße", "Maximum size"),
    FltOlderThan => ("Älter als (Tage)", "Older than (days)"),
    FltNewerThan => ("Neuer als (Tage)", "Newer than (days)"),
    FltAgeField => ("Bezugsdatum", "Date field"),
    FltAgeModified => ("Geändert", "Modified"),
    FltAgeAccessed => ("Letzter Zugriff", "Last accessed"),
    FltAgeCreated => ("Erstellt", "Created"),
    FltExtensions => ("Endungen (z. B. jpg, png)", "Extensions (e.g. jpg, png)"),
    FltFilesOnly => ("Nur Dateien", "Files only"),
    FltIncludeHidden => ("Versteckte einbeziehen", "Include hidden"),
    FltApply => ("Suchen", "Search"),
    FltClear => ("Filter leeren", "Clear filter"),
    FltResults => ("{n} Treffer", "{n} results"),
    FltLimitReached => ("Trefferlimit erreicht, Filter verfeinern.", "Result limit reached, refine the filter."),
    FltInvalidGlob => ("Ungültiges Namensmuster", "Invalid name pattern"),
    FltUnitBytes => ("Bytes", "bytes"),
    SearchPlaceholder => ("Suchen …", "Search …"),

    // ---- Top 100 / Dateitypen / Alter ----
    TopTitle => ("Die 100 größten Dateien", "The 100 largest files"),
    TypesTitle => ("Dateitypen nach Größe", "File types by size"),
    AgeTitle => ("Alter der Daten", "Data age"),
    AgeLt1M => ("< 1 Monat", "< 1 month"),
    AgeLt3M => ("1–3 Monate", "1–3 months"),
    AgeLt6M => ("3–6 Monate", "3–6 months"),
    AgeLt1Y => ("6–12 Monate", "6–12 months"),
    AgeLt3Y => ("1–3 Jahre", "1–3 years"),
    AgeGe3Y => ("≥ 3 Jahre", "≥ 3 years"),

    // ---- Duplikate ----
    DupTitle => ("Doppelte Dateien", "Duplicate files"),
    DupStart => ("Duplikate suchen", "Find duplicates"),
    DupMinSize => ("Mindestgröße", "Minimum size"),
    DupPhaseSize => ("Gruppiere nach Größe …", "Grouping by size …"),
    DupPhasePartial => ("Prüfe Dateianfänge …", "Checking file heads …"),
    DupPhaseFull => ("Berechne Prüfsummen …", "Computing hashes …"),
    DupProgress => ("{done} von {total} geprüft", "{done} of {total} checked"),
    DupGroups => ("{n} Gruppen", "{n} groups"),
    DupSavings => ("Mögliche Einsparung: {size}", "Potential savings: {size}"),
    DupNone => ("Keine Duplikate gefunden.", "No duplicates found."),
    DupCancelled => ("Suche abgebrochen, Teilergebnis.", "Search cancelled, partial result."),

    // ---- Delta / Verlauf ----
    DeltaTitle => ("Vergleich mit Snapshot", "Comparison with snapshot"),
    DeltaChooseOld => ("Älteren Snapshot wählen …", "Choose older snapshot …"),
    DeltaMinChange => ("Mindeständerung", "Minimum change"),
    DeltaAdded => ("Neu", "Added"),
    DeltaRemoved => ("Entfernt", "Removed"),
    DeltaGrown => ("Gewachsen", "Grown"),
    DeltaShrunk => ("Geschrumpft", "Shrunk"),
    DeltaNone => ("Keine Änderungen.", "No changes."),
    DeltaSummary => ("{n} Änderungen, netto {size}", "{n} changes, net {size}"),
    HistTitle => ("Verlauf", "History"),
    HistChooseFolder => ("Snapshot-Ordner wählen …", "Choose snapshot folder …"),
    HistSnapshots => ("{n} Snapshots geladen", "{n} snapshots loaded"),
    HistTopGrowers => ("Stärkstes Wachstum", "Top growers"),
    HistDepth => ("Ordnertiefe", "Folder depth"),
    HistNeedTwo => ("Für den Verlauf werden mindestens zwei Snapshots desselben Ziels benötigt.", "History needs at least two snapshots of the same target."),

    // ---- Detailpanel ----
    DetTitle => ("Details", "Details"),
    DetOwner => ("Besitzer", "Owner"),
    DetPermissions => ("Berechtigungen", "Permissions"),
    DetPrincipal => ("Konto", "Principal"),
    DetAllow => ("Zulassen", "Allow"),
    DetDeny => ("Verweigern", "Deny"),
    DetRights => ("Rechte", "Rights"),
    DetInherited => ("Geerbt", "Inherited"),
    DetStreams => ("Alternative Datenströme", "Alternate data streams"),
    DetNoStreams => ("Keine alternativen Datenströme", "No alternate data streams"),
    DetTimes => ("Zeitstempel", "Timestamps"),
    DetLoad => ("Laden", "Load"),
    DetAccessNote => ("Windows pflegt \"Letzter Zugriff\" oft nicht (NtfsDisableLastAccessUpdate).", "Windows often does not maintain \"Last accessed\" (NtfsDisableLastAccessUpdate)."),
    DetNotAvailable => ("Nicht verfügbar", "Not available"),

    // ---- Fußzeile / Status ----
    FooterBrand => ("Joschys_Tree @ Joschy", "Joschys_Tree @ Joschy"),
    FooterAdmin => ("Admin", "Admin"),
    FooterReadOnly => ("Nur-Lesen", "Read-only"),
    FooterMode => ("Scan-Modus", "Scan mode"),
    ModeMft => ("MFT", "MFT"),
    ModeWalk => ("Walk", "Walk"),
    ModeMftLong => ("MFT (schnell, direkt vom Volume)", "MFT (fast, read from volume)"),
    ModeWalkLong => ("Walk (Verzeichnisse einzeln gelesen)", "Walk (directories read one by one)"),
    ModeFallback => ("MFT nicht möglich, Walk wird verwendet.", "MFT not possible, using walk."),
    StatusReady => ("Bereit", "Ready"),
    StatusScanning => ("Scanne …", "Scanning …"),
    StatusScanProgress => ("{entries} Einträge, {size} – {current}", "{entries} entries, {size} – {current}"),
    StatusScanDone => ("Scan fertig: {entries} Einträge in {secs} s", "Scan finished: {entries} entries in {secs} s"),
    StatusScanCancelled => ("Scan abgebrochen", "Scan cancelled"),
    StatusErrors => ("{n} Zugriffsfehler", "{n} access errors"),
    StatusVolume => ("{free} frei von {total}", "{free} free of {total}"),
    StatusCluster => ("Clustergröße", "Cluster size"),
    StatusSelected => ("{n} ausgewählt", "{n} selected"),
    ReadOnlyHint => ("Nur-Lesen-Modus: Löschen ist deaktiviert. Für MFT-Scan und Löschen als Administrator starten.", "Read-only mode: deleting is disabled. Run as administrator for MFT scan and deleting."),

    // ---- Treemap ----
    TreemapZoomOut => ("Eine Ebene höher", "Up one level"),
    TreemapTooltip => ("{name}: {size}", "{name}: {size}"),
    TreemapEmpty => ("Keine Daten für die Treemap", "No data for the treemap"),

    // ---- Über-Dialog ----
    AboutTitle => ("Über Joschys_Tree", "About Joschys_Tree"),
    AboutText => ("Schneller, portabler Festplatten-Analyzer für Windows. @ Joschy", "Fast, portable disk space analyzer for Windows. @ Joschy"),
    AboutVersion => ("Version", "Version"),
    AboutLicense => ("Lizenz: AGPL-3.0-or-later", "License: AGPL-3.0-or-later"),

    // ---- Einstellungen ----
    SetTitle => ("Einstellungen", "Settings"),
    SetSaved => ("Einstellungen gespeichert", "Settings saved"),
    SetSaveFailed => ("Einstellungen konnten nicht gespeichert werden: {err}", "Could not save settings: {err}"),

    // ---- Snapshots ----
    SnapSaved => ("Snapshot gespeichert: {path}", "Snapshot saved: {path}"),
    SnapLoaded => ("Snapshot geladen: {path}", "Snapshot loaded: {path}"),
    SnapFileFilter => ("Joschys_Tree-Scan (*.jtscan)", "Joschys_Tree scan (*.jtscan)"),

    // ---- Fehlermeldungen ----
    ErrTitle => ("Fehler", "Error"),
    ErrScanFailed => ("Scan fehlgeschlagen: {err}", "Scan failed: {err}"),
    ErrPathNotFound => ("Pfad nicht gefunden: {path}", "Path not found: {path}"),
    ErrAccessDenied => ("Zugriff verweigert: {path}", "Access denied: {path}"),
    ErrSnapshotLoad => ("Snapshot konnte nicht geladen werden: {err}", "Could not load snapshot: {err}"),
    ErrSnapshotSave => ("Snapshot konnte nicht gespeichert werden: {err}", "Could not save snapshot: {err}"),
    ErrSnapshotVersion => ("Snapshot hat eine nicht unterstützte Version.", "Snapshot has an unsupported version."),
    ErrSnapshotMagic => ("Datei ist kein Joschys_Tree-Snapshot.", "File is not a Joschys_Tree snapshot."),
    ErrSnapshotOtherRoot => ("Snapshot gehört zu einem anderen Ziel: {path}", "Snapshot belongs to another target: {path}"),
    ErrExplorer => ("Explorer konnte nicht geöffnet werden: {err}", "Could not open Explorer: {err}"),
    ErrClipboard => ("Zwischenablage nicht verfügbar", "Clipboard not available"),
    ErrNoScan => ("Kein Scan vorhanden.", "No scan available."),
    ErrUnsupported => ("Auf diesem System nicht unterstützt.", "Not supported on this system."),
    ErrElevate => ("Neustart als Administrator fehlgeschlagen: {err}", "Restart as administrator failed: {err}"),
    ErrGeneric => ("Unerwarteter Fehler: {err}", "Unexpected error: {err}"),
}

/// Für den Vollständigkeitstest.
pub fn all_keys() -> &'static [Key] {
    ALL_KEYS
}
