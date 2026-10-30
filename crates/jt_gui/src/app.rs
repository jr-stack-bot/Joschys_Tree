// Joschys_Tree – Anwendungszustand, Tabs, Hintergrund-Jobs – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender, unbounded};
use eframe::egui;
use jt_core::delta::DeltaEntry;
use jt_core::duplicates::DupGroup;
use jt_core::export::{ExportContent, ExportRequest};
use jt_core::history::HistoryPoint;
use jt_core::platform::{SecurityInfo, StreamInfo};
use jt_core::scan::{ScanError, ScanProgress, ScanRequest};
use jt_core::{FileTime, Key, Lang, NO_NODE, NodeId, Tree, tr};

use crate::dialogs::{self, ContentKind, ExportDialog};
use crate::settings::Settings;
use crate::views;
use crate::{WINDOW_TITLE, footer, theme};

/// Mindestabstand zwischen zwei Fortschrittsmeldungen aus Hintergrund-Threads.
const PROGRESS_INTERVAL: Duration = Duration::from_millis(80);
/// Höchstzahl Treffer der Suche.
pub const SEARCH_LIMIT: usize = 10_000;

/// Tabs der Hauptansicht.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Tab {
    #[default]
    Tree,
    Treemap,
    TopFiles,
    FileTypes,
    Search,
    Duplicates,
    Delta,
}

/// Lösch-Regel an genau einer Stelle: nur als Admin.
pub fn can_delete() -> bool {
    jt_core::platform::is_admin()
}

/// Ersetzt Platzhalter wie `{n}` in übersetzten Texten.
pub fn fill(template: &str, args: &[(&str, &str)]) -> String {
    let mut s = template.to_string();
    for (k, v) in args {
        s = s.replace(k, v);
    }
    s
}

/// Benutzeraktionen aus den Ansichten; werden nach dem Zeichnen ausgeführt.
#[derive(Clone, Debug)]
pub enum Action {
    Select(NodeId),
    Reveal(NodeId),
    ShowInTreemap(NodeId),
    OpenExplorer(NodeId),
    CopyPath(NodeId),
    AskDelete { id: NodeId, permanent: bool },
    ConfirmDelete { ids: Vec<NodeId>, permanent: bool },
    OpenExport,
    RunExport(ExportDialog),
    StartScan,
    StopScan,
    BrowseFolder,
    OpenSnapshot,
    SaveSnapshot,
    LoadDetails(NodeId),
    StartDuplicates,
    CancelDuplicates,
    ChooseCompare,
    ChooseHistory,
    RestartAsAdmin,
}

/// Art eines Hintergrund-Jobs (für Fehler/Panik-Rückmeldung).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JobKind {
    Scan,
    Io,
    Dup,
    Details,
    Delete,
}

/// Ergebnis der Verlaufsanalyse.
#[derive(Clone, Debug, Default)]
pub struct HistoryResult {
    pub path: String,
    pub snapshots: usize,
    pub series: Vec<HistoryPoint>,
    pub growers: Vec<(String, i64)>,
}

/// Lazy geladene Detailinfos (Owner/ACL/ADS).
#[derive(Clone, Debug)]
pub struct DetailInfo {
    pub security: Result<SecurityInfo, String>,
    pub streams: Result<Vec<StreamInfo>, String>,
}

/// Nachrichten aus Hintergrund-Threads.
pub enum JobMsg {
    ScanProgress(ScanProgress),
    ScanDone(Result<Tree, ScanError>),
    TreeLoaded(Tree, PathBuf),
    SnapshotSaved(PathBuf),
    FolderPicked(PathBuf),
    DupProgress(u64, u64),
    DupDone(Vec<DupGroup>, bool),
    ExportDone(PathBuf),
    DeltaDone(String, Vec<DeltaEntry>, i64),
    HistoryDone(HistoryResult),
    Details(NodeId, u64, DetailInfo),
    DeleteDone {
        gen_: u64,
        removed: Vec<NodeId>,
        permanent: bool,
        error: Option<String>,
    },
    /// Dateidialog abgebrochen.
    DialogCancelled,
    Failed(JobKind, String),
}

/// Laufender Scan.
pub struct ScanJob {
    pub cancel: Arc<AtomicBool>,
    pub progress: ScanProgress,
    pub target: String,
    pub started: Instant,
}

/// Sendet Nachrichten an die UI und weckt sie auf.
#[derive(Clone)]
pub struct Reporter {
    tx: Sender<JobMsg>,
    ctx: egui::Context,
}

impl Reporter {
    pub fn send(&self, msg: JobMsg) {
        let _ = self.tx.send(msg);
        self.ctx.request_repaint();
    }
}

pub struct AppState {
    pub settings: Settings,
    pub tree: Option<Arc<Tree>>,
    /// Wird bei jeder Baumänderung erhöht; Caches vergleichen dagegen.
    pub tree_gen: u64,
    pub tab: Tab,
    pub status: String,
    pub actions: Vec<Action>,
    pub selected: Option<NodeId>,
    pub now: FileTime,
    pub target_input: String,
    pub drives: Vec<String>,
    pub scan: Option<ScanJob>,
    /// Laufender Datei-/Export-Job mit Beschriftung.
    pub io_busy: Option<Key>,
    pub delete_running: bool,
    pub tree_view: views::tree_view::TreeViewState,
    pub treemap: views::treemap::TreemapState,
    pub top: views::top_files::TopState,
    pub types: views::file_types::TypesState,
    pub search: views::search::SearchState,
    pub dups: views::duplicates::DupState,
    pub delta: views::delta::DeltaState,
    pub details: views::details::DetailsState,
    pub dialogs: dialogs::Dialogs,
}

impl AppState {
    pub fn new(settings: Settings) -> Self {
        let drives = jt_core::platform::list_drives();
        let target_input = settings
            .recent_targets
            .first()
            .cloned()
            .or_else(|| drives.first().cloned())
            .unwrap_or_default();
        AppState {
            settings,
            tree: None,
            tree_gen: 1,
            tab: Tab::Tree,
            status: String::new(),
            actions: Vec::new(),
            selected: None,
            now: FileTime::default(),
            target_input,
            drives,
            scan: None,
            io_busy: None,
            delete_running: false,
            tree_view: Default::default(),
            treemap: Default::default(),
            top: Default::default(),
            types: Default::default(),
            search: Default::default(),
            dups: Default::default(),
            delta: Default::default(),
            details: Default::default(),
            dialogs: Default::default(),
        }
    }

    pub fn lang(&self) -> Lang {
        self.settings.lang
    }

    pub fn file_dialog_busy(&self) -> bool {
        self.io_busy.is_some()
    }

    /// Neuen Baum übernehmen und alle Caches zurücksetzen.
    pub fn set_tree(&mut self, tree: Tree) {
        self.tree = Some(Arc::new(tree));
        self.tree_gen += 1;
        self.selected = None;
        self.tree_view = Default::default();
        self.treemap = Default::default();
        self.top = Default::default();
        self.types = Default::default();
        self.search.results.clear();
        self.dups = Default::default();
        self.details = Default::default();
    }

    fn save_settings(&mut self) {
        if let Err(e) = self.settings.save() {
            self.status = fill(
                tr(self.lang(), Key::SetSaveFailed),
                &[("{err}", &e.to_string())],
            );
        }
    }

    fn window_title(&self) -> String {
        let target = match (&self.scan, &self.tree) {
            (Some(job), _) => job.target.clone(),
            (None, Some(t)) => t.meta.root_path.clone(),
            _ => String::new(),
        };
        if target.is_empty() {
            WINDOW_TITLE.to_string()
        } else {
            format!("{WINDOW_TITLE} – {target}")
        }
    }
}

/// Entfernt einen Knoten aus der Kinderliste seines Elternordners (Id bleibt gültig).
pub fn unlink_node(tree: &mut Tree, id: NodeId) {
    let parent = tree.nodes[id as usize].parent;
    if parent == NO_NODE {
        return;
    }
    let next = tree.nodes[id as usize].next_sibling;
    let first = tree.nodes[parent as usize].first_child;
    if first == id {
        tree.nodes[parent as usize].first_child = next;
    } else {
        let mut c = first;
        while c != NO_NODE {
            let n = tree.nodes[c as usize].next_sibling;
            if n == id {
                tree.nodes[c as usize].next_sibling = next;
                break;
            }
            c = n;
        }
    }
    tree.nodes[id as usize].next_sibling = NO_NODE;
}

/// Ist `id` noch über die Wurzel erreichbar?
pub fn is_attached(tree: &Tree, id: NodeId) -> bool {
    let mut cur = id;
    while cur != tree.root() {
        let p = tree.node(cur).parent;
        if p == NO_NODE || !tree.children(p).any(|c| c == cur) {
            return false;
        }
        cur = p;
    }
    true
}

pub struct JtApp {
    pub state: AppState,
    tx: Sender<JobMsg>,
    rx: Receiver<JobMsg>,
    last_title: String,
}

impl JtApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let settings = Settings::load();
        theme::apply(&cc.egui_ctx, settings.dark);
        let (tx, rx) = unbounded();
        JtApp {
            state: AppState::new(settings),
            tx,
            rx,
            last_title: String::new(),
        }
    }

    /// Startet einen Hintergrund-Thread; Panik wird als Fehler gemeldet.
    fn spawn(
        &self,
        ctx: &egui::Context,
        kind: JobKind,
        f: impl FnOnce(&Reporter) -> JobMsg + Send + 'static,
    ) {
        let rep = Reporter {
            tx: self.tx.clone(),
            ctx: ctx.clone(),
        };
        let lang = self.state.lang();
        std::thread::spawn(move || {
            let msg = catch_unwind(AssertUnwindSafe(|| f(&rep))).unwrap_or_else(|p| {
                let err = p
                    .downcast_ref::<&str>()
                    .map(|s| s.to_string())
                    .or_else(|| p.downcast_ref::<String>().cloned())
                    .unwrap_or_default();
                JobMsg::Failed(kind, fill(tr(lang, Key::ErrGeneric), &[("{err}", &err)]))
            });
            rep.send(msg);
        });
    }

    fn poll_jobs(&mut self) {
        let lang = self.state.lang();
        while let Ok(msg) = self.rx.try_recv() {
            let st = &mut self.state;
            match msg {
                JobMsg::ScanProgress(p) => {
                    if let Some(job) = &mut st.scan {
                        job.progress = p;
                    }
                }
                JobMsg::ScanDone(res) => {
                    let job = st.scan.take();
                    match res {
                        Ok(tree) => {
                            let secs = job
                                .map(|j| j.started.elapsed().as_secs_f32())
                                .unwrap_or_default();
                            let entries = jt_core::format::number(tree.nodes.len() as u64, lang);
                            st.status = fill(
                                tr(lang, Key::StatusScanDone),
                                &[("{entries}", &entries), ("{secs}", &format!("{secs:.1}"))],
                            );
                            st.set_tree(tree);
                        }
                        Err(ScanError::Cancelled) => {
                            st.status = tr(lang, Key::StatusScanCancelled).to_string();
                        }
                        Err(e) => {
                            st.status =
                                fill(tr(lang, Key::ErrScanFailed), &[("{err}", &e.to_string())]);
                        }
                    }
                }
                JobMsg::TreeLoaded(tree, path) => {
                    st.io_busy = None;
                    st.target_input = tree.meta.root_path.clone();
                    st.status = fill(
                        tr(lang, Key::SnapLoaded),
                        &[("{path}", &path.display().to_string())],
                    );
                    st.set_tree(tree);
                }
                JobMsg::SnapshotSaved(path) => {
                    st.io_busy = None;
                    st.status = fill(
                        tr(lang, Key::SnapSaved),
                        &[("{path}", &path.display().to_string())],
                    );
                }
                JobMsg::FolderPicked(path) => {
                    st.io_busy = None;
                    st.target_input = path.display().to_string();
                }
                JobMsg::DupProgress(done, total) => {
                    if let Some(p) = &mut st.dups.running {
                        p.done = done;
                        p.total = total;
                    }
                }
                JobMsg::DupDone(groups, cancelled) => {
                    st.dups.running = None;
                    st.dups.set_results(groups, st.tree_gen);
                    if cancelled {
                        st.status = tr(lang, Key::DupCancelled).to_string();
                    }
                }
                JobMsg::ExportDone(path) => {
                    st.io_busy = None;
                    st.status = fill(
                        tr(lang, Key::ExpDone),
                        &[("{path}", &path.display().to_string())],
                    );
                }
                JobMsg::DeltaDone(path, entries, net) => {
                    st.io_busy = None;
                    st.delta.set_compare(path, entries, net);
                }
                JobMsg::HistoryDone(h) => {
                    st.io_busy = None;
                    st.delta.history = Some(h);
                }
                JobMsg::Details(id, gen_, info) => {
                    st.details.loading = None;
                    if gen_ == st.tree_gen {
                        st.details.info = Some((id, info));
                    }
                }
                JobMsg::DeleteDone {
                    gen_,
                    removed,
                    permanent,
                    error,
                } => {
                    st.delete_running = false;
                    if gen_ == st.tree_gen {
                        Self::apply_removal(st, &removed);
                    }
                    let n = removed.len().to_string();
                    st.status = match error {
                        Some(err) => fill(tr(lang, Key::DelFailed), &[("{err}", &err)]),
                        None if permanent => fill(tr(lang, Key::DelDone), &[("{n}", &n)]),
                        None => fill(tr(lang, Key::DelRecycled), &[("{n}", &n)]),
                    };
                }
                JobMsg::DialogCancelled => st.io_busy = None,
                JobMsg::Failed(kind, err) => {
                    match kind {
                        JobKind::Scan => st.scan = None,
                        JobKind::Io => st.io_busy = None,
                        JobKind::Dup => st.dups.running = None,
                        JobKind::Details => st.details.loading = None,
                        JobKind::Delete => st.delete_running = false,
                    }
                    st.status = err;
                }
            }
        }
    }

    /// Gelöschte Knoten aus dem Baum lösen und neu aggregieren.
    fn apply_removal(st: &mut AppState, removed: &[NodeId]) {
        if removed.is_empty() {
            return;
        }
        let Some(arc) = st.tree.as_mut() else {
            return;
        };
        let tree = Arc::make_mut(arc);
        for &id in removed {
            if id != tree.root() {
                unlink_node(tree, id);
            }
        }
        jt_core::aggregate::recompute(tree);
        let tree: &Tree = tree;
        if let Some(sel) = st.selected
            && !is_attached(tree, sel)
        {
            st.selected = None;
        }
        st.search.results.retain(|&id| is_attached(tree, id));
        st.dups.retain_attached(tree);
        if st.treemap.root != 0 && !is_attached(tree, st.treemap.root) {
            st.treemap.root = 0;
        }
        st.tree_gen += 1;
        st.tree_view.invalidate();
        st.treemap.invalidate();
        st.top = Default::default();
        st.types = Default::default();
        st.details = Default::default();
    }

    fn process_actions(&mut self, ctx: &egui::Context) {
        let actions = std::mem::take(&mut self.state.actions);
        for a in actions {
            self.handle_action(ctx, a);
        }
    }

    fn handle_action(&mut self, ctx: &egui::Context, action: Action) {
        let lang = self.state.lang();
        let tree = self.state.tree.clone();
        match action {
            Action::Select(id) => self.state.selected = Some(id),
            Action::Reveal(id) => {
                if let Some(t) = &tree {
                    self.state.tree_view.reveal(t, id);
                    self.state.selected = Some(id);
                    self.state.tab = Tab::Tree;
                }
            }
            Action::ShowInTreemap(id) => {
                self.state.treemap.root = id;
                self.state.selected = Some(id);
                self.state.tab = Tab::Treemap;
            }
            Action::OpenExplorer(id) => {
                if let Some(t) = &tree
                    && let Err(e) = jt_core::platform::open_in_explorer(&t.full_path(id))
                {
                    self.state.status =
                        fill(tr(lang, Key::ErrExplorer), &[("{err}", &e.to_string())]);
                }
            }
            Action::CopyPath(id) => {
                if let Some(t) = &tree {
                    ctx.copy_text(t.full_path(id));
                }
            }
            Action::AskDelete { id, permanent } => {
                if can_delete() && tree.is_some() && !self.state.delete_running {
                    self.state.dialogs.delete = Some(dialogs::DeleteDialog::new(id, permanent));
                } else if !can_delete() {
                    self.state.status = tr(lang, Key::DelAdminOnly).to_string();
                }
            }
            Action::ConfirmDelete { ids, permanent } => {
                if let Some(t) = tree
                    && can_delete()
                {
                    self.start_delete(ctx, t, ids, permanent);
                }
            }
            Action::OpenExport => {
                if tree.is_some() {
                    self.state.dialogs.export = Some(ExportDialog::new(
                        self.state.settings.export_depth,
                        self.state.selected,
                    ));
                }
            }
            Action::RunExport(cfg) => {
                if let Some(t) = tree {
                    self.state.settings.export_depth = cfg.depth;
                    self.state.save_settings();
                    self.start_export(ctx, t, cfg);
                }
            }
            Action::StartScan => self.start_scan(ctx),
            Action::StopScan => {
                if let Some(job) = &self.state.scan {
                    job.cancel.store(true, Ordering::Relaxed);
                }
            }
            Action::BrowseFolder => {
                self.state.io_busy = Some(Key::Working);
                self.spawn(ctx, JobKind::Io, |_| {
                    match rfd::FileDialog::new().pick_folder() {
                        Some(p) => JobMsg::FolderPicked(p),
                        None => JobMsg::DialogCancelled,
                    }
                });
            }
            Action::OpenSnapshot => {
                self.state.io_busy = Some(Key::Loading);
                let filter = tr(lang, Key::SnapFileFilter);
                self.spawn(ctx, JobKind::Io, move |_| {
                    let Some(path) = rfd::FileDialog::new()
                        .add_filter(filter, &["jtscan"])
                        .pick_file()
                    else {
                        return JobMsg::DialogCancelled;
                    };
                    match jt_core::snapshot::load(&path) {
                        Ok(t) => JobMsg::TreeLoaded(t, path),
                        Err(e) => JobMsg::Failed(JobKind::Io, snapshot_error(lang, &e, true)),
                    }
                });
            }
            Action::SaveSnapshot => {
                let Some(t) = tree else { return };
                self.state.io_busy = Some(Key::Working);
                let filter = tr(lang, Key::SnapFileFilter);
                let name = snapshot_file_name(&t);
                self.spawn(ctx, JobKind::Io, move |_| {
                    let Some(path) = rfd::FileDialog::new()
                        .add_filter(filter, &["jtscan"])
                        .set_file_name(name)
                        .save_file()
                    else {
                        return JobMsg::DialogCancelled;
                    };
                    match jt_core::snapshot::save(&t, &path) {
                        Ok(()) => JobMsg::SnapshotSaved(path),
                        Err(e) => JobMsg::Failed(JobKind::Io, snapshot_error(lang, &e, false)),
                    }
                });
            }
            Action::LoadDetails(id) => {
                let Some(t) = tree else { return };
                if self.state.details.loading.is_some() {
                    return;
                }
                self.state.details.loading = Some(id);
                let gen_ = self.state.tree_gen;
                self.spawn(ctx, JobKind::Details, move |_| {
                    let path = t.full_path(id);
                    let info = DetailInfo {
                        security: jt_core::platform::security_info(&path)
                            .map_err(|e| e.to_string()),
                        streams: jt_core::platform::alternate_streams(&path)
                            .map_err(|e| e.to_string()),
                    };
                    JobMsg::Details(id, gen_, info)
                });
            }
            Action::StartDuplicates => {
                let Some(t) = tree else { return };
                if self.state.dups.running.is_some() {
                    return;
                }
                let cancel = Arc::new(AtomicBool::new(false));
                self.state.dups.running = Some(views::duplicates::DupRun {
                    cancel: cancel.clone(),
                    done: 0,
                    total: 0,
                });
                self.state.tab = Tab::Duplicates;
                let min = self.state.settings.dup_min_size;
                self.spawn(ctx, JobKind::Dup, move |rep| {
                    let last = Cell::new(Instant::now() - PROGRESS_INTERVAL);
                    let progress = |done: u64, total: u64| {
                        if last.get().elapsed() >= PROGRESS_INTERVAL {
                            last.set(Instant::now());
                            rep.send(JobMsg::DupProgress(done, total));
                        }
                    };
                    let reader = jt_core::duplicates::StdFileReader;
                    let groups = jt_core::duplicates::find_duplicates(
                        &t,
                        t.root(),
                        min,
                        &reader,
                        &progress,
                        &cancel,
                    );
                    JobMsg::DupDone(groups, cancel.load(Ordering::Relaxed))
                });
            }
            Action::CancelDuplicates => {
                if let Some(r) = &self.state.dups.running {
                    r.cancel.store(true, Ordering::Relaxed);
                }
            }
            Action::ChooseCompare => {
                let Some(t) = tree else { return };
                self.state.io_busy = Some(Key::Working);
                let filter = tr(lang, Key::SnapFileFilter);
                let min = self.state.delta.min_change_mib.saturating_mul(1024 * 1024);
                self.spawn(ctx, JobKind::Io, move |_| {
                    let Some(path) = rfd::FileDialog::new()
                        .add_filter(filter, &["jtscan"])
                        .pick_file()
                    else {
                        return JobMsg::DialogCancelled;
                    };
                    let old = match jt_core::snapshot::load(&path) {
                        Ok(o) => o,
                        Err(e) => {
                            return JobMsg::Failed(JobKind::Io, snapshot_error(lang, &e, true));
                        }
                    };
                    if !same_root(&old.meta.root_path, &t.meta.root_path) {
                        return JobMsg::Failed(
                            JobKind::Io,
                            fill(
                                tr(lang, Key::ErrSnapshotOtherRoot),
                                &[("{path}", &old.meta.root_path)],
                            ),
                        );
                    }
                    let entries = jt_core::delta::compare(&old, &t, min);
                    let net = t.node(t.root()).size as i128 - old.node(old.root()).size as i128;
                    let net = net.clamp(i64::MIN as i128, i64::MAX as i128) as i64;
                    JobMsg::DeltaDone(path.display().to_string(), entries, net)
                });
            }
            Action::ChooseHistory => {
                self.state.io_busy = Some(Key::Loading);
                let depth = self.state.delta.depth;
                let want_root = tree.as_ref().map(|t| t.meta.root_path.clone());
                let sel_path = match (&tree, self.state.selected) {
                    (Some(t), Some(id)) => Some(t.full_path(id)),
                    _ => None,
                };
                self.spawn(ctx, JobKind::Io, move |_| {
                    let Some(dir) = rfd::FileDialog::new().pick_folder() else {
                        return JobMsg::DialogCancelled;
                    };
                    match load_history(&dir, want_root, sel_path, depth) {
                        Some(h) => JobMsg::HistoryDone(h),
                        None => JobMsg::Failed(JobKind::Io, tr(lang, Key::HistNeedTwo).into()),
                    }
                });
            }
            Action::RestartAsAdmin => match restart_as_admin() {
                Ok(()) => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                Err(e) => {
                    self.state.status =
                        fill(tr(lang, Key::ErrElevate), &[("{err}", &e.to_string())]);
                }
            },
        }
    }

    fn start_scan(&mut self, ctx: &egui::Context) {
        if self.state.scan.is_some() {
            return;
        }
        let target = self.state.target_input.trim().to_string();
        if target.is_empty() {
            return;
        }
        self.state.settings.remember_target(&target);
        self.state.save_settings();
        let cancel = Arc::new(AtomicBool::new(false));
        self.state.scan = Some(ScanJob {
            cancel: cancel.clone(),
            progress: ScanProgress::default(),
            target: target.clone(),
            started: Instant::now(),
        });
        self.state.status = tr(self.state.lang(), Key::StatusScanning).to_string();
        let req = ScanRequest {
            root: target,
            prefer_mft: self.state.settings.prefer_mft,
            follow_reparse: false,
        };
        self.spawn(ctx, JobKind::Scan, move |rep| {
            let last = Cell::new(Instant::now() - PROGRESS_INTERVAL);
            let progress = |p: &ScanProgress| {
                if last.get().elapsed() >= PROGRESS_INTERVAL {
                    last.set(Instant::now());
                    rep.send(JobMsg::ScanProgress(p.clone()));
                }
            };
            JobMsg::ScanDone(jt_core::scan::scan(&req, &progress, &cancel))
        });
    }

    fn start_delete(
        &mut self,
        ctx: &egui::Context,
        tree: Arc<Tree>,
        ids: Vec<NodeId>,
        permanent: bool,
    ) {
        let ids: Vec<NodeId> = ids.into_iter().filter(|&id| id != tree.root()).collect();
        if ids.is_empty() || self.state.delete_running {
            return;
        }
        self.state.delete_running = true;
        let gen_ = self.state.tree_gen;
        self.spawn(ctx, JobKind::Delete, move |_| {
            let paths: Vec<String> = ids.iter().map(|&id| tree.full_path(id)).collect();
            let res = if permanent {
                jt_core::platform::delete_permanently(&paths)
            } else {
                jt_core::platform::move_to_recycle_bin(&paths)
            };
            // Nur entfernen, was wirklich nicht mehr existiert (auch bei Teilerfolg).
            let removed = ids
                .iter()
                .zip(&paths)
                .filter(|(_, p)| {
                    std::fs::symlink_metadata(jt_core::platform::long_path(p)).is_err()
                })
                .map(|(&id, _)| id)
                .collect();
            JobMsg::DeleteDone {
                gen_,
                removed,
                permanent,
                error: res.err().map(|e| e.to_string()),
            }
        });
    }

    fn start_export(&mut self, ctx: &egui::Context, tree: Arc<Tree>, cfg: ExportDialog) {
        let lang = self.state.lang();
        let dups = self.state.dups.groups.clone();
        let delta = self.state.delta.entries.clone();
        self.state.io_busy = Some(Key::ExpRunning);
        self.spawn(ctx, JobKind::Io, move |_| {
            let ext = cfg.format.extension();
            let Some(path) = rfd::FileDialog::new()
                .add_filter(dialogs::format_label(lang, cfg.format), &[ext])
                .set_file_name(format!("JoschysTree_Export.{ext}"))
                .save_file()
            else {
                return JobMsg::DialogCancelled;
            };
            let content = match cfg.content {
                ContentKind::Tree => ExportContent::Tree {
                    root: cfg.root,
                    max_depth: cfg.depth,
                },
                ContentKind::TopFiles => {
                    ExportContent::TopFiles(jt_core::stats::top_files(&tree, 100, None))
                }
                ContentKind::FileTypes => {
                    ExportContent::FileTypes(jt_core::stats::file_types(&tree, tree.root()))
                }
                ContentKind::Duplicates => ExportContent::Duplicates(dups),
                ContentKind::Delta => ExportContent::Delta(delta),
            };
            let req = ExportRequest {
                tree: Some(&tree),
                content,
                lang,
                title: cfg.title.clone(),
            };
            let res = std::fs::File::create(&path)
                .map_err(jt_core::export::ExportError::from)
                .and_then(|f| {
                    let mut w = std::io::BufWriter::new(f);
                    jt_core::export::export(&req, cfg.format, &mut w)?;
                    std::io::Write::flush(&mut w)?;
                    Ok(())
                });
            match res {
                Ok(()) => JobMsg::ExportDone(path),
                Err(e) => JobMsg::Failed(
                    JobKind::Io,
                    fill(tr(lang, Key::ExpFailed), &[("{err}", &e.to_string())]),
                ),
            }
        });
    }

    fn menu_bar(&mut self, ui: &mut egui::Ui) {
        let lang = self.state.lang();
        let st = &mut self.state;
        let has_tree = st.tree.is_some();
        let io_free = st.io_busy.is_none();
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button(tr(lang, Key::MenuFile), |ui| {
                if ui
                    .add_enabled(
                        st.scan.is_none(),
                        egui::Button::new(tr(lang, Key::MenuScan)),
                    )
                    .clicked()
                {
                    st.actions.push(Action::StartScan);
                    ui.close();
                }
                if ui
                    .add_enabled(io_free, egui::Button::new(tr(lang, Key::MenuScanFolder)))
                    .clicked()
                {
                    st.actions.push(Action::BrowseFolder);
                    ui.close();
                }
                ui.separator();
                if ui
                    .add_enabled(io_free, egui::Button::new(tr(lang, Key::MenuOpenSnapshot)))
                    .clicked()
                {
                    st.actions.push(Action::OpenSnapshot);
                    ui.close();
                }
                if ui
                    .add_enabled(
                        io_free && has_tree,
                        egui::Button::new(tr(lang, Key::MenuSaveSnapshot)),
                    )
                    .clicked()
                {
                    st.actions.push(Action::SaveSnapshot);
                    ui.close();
                }
                if ui
                    .add_enabled(
                        io_free && has_tree,
                        egui::Button::new(tr(lang, Key::MenuExport)),
                    )
                    .clicked()
                {
                    st.actions.push(Action::OpenExport);
                    ui.close();
                }
                ui.separator();
                if ui.button(tr(lang, Key::MenuExit)).clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
            ui.menu_button(tr(lang, Key::MenuView), |ui| {
                ui.label(tr(lang, Key::MenuTheme));
                let mut dark = st.settings.dark;
                ui.radio_value(&mut dark, true, tr(lang, Key::ThemeDark));
                ui.radio_value(&mut dark, false, tr(lang, Key::ThemeLight));
                if dark != st.settings.dark {
                    st.settings.dark = dark;
                    theme::apply(ui.ctx(), dark);
                    st.save_settings();
                }
                ui.separator();
                ui.label(tr(lang, Key::MenuLanguage));
                let mut l = st.settings.lang;
                ui.radio_value(&mut l, Lang::De, tr(lang, Key::LangGerman));
                ui.radio_value(&mut l, Lang::En, tr(lang, Key::LangEnglish));
                if l != st.settings.lang {
                    st.settings.lang = l;
                    st.save_settings();
                }
            });
            ui.menu_button(tr(lang, Key::MenuTools), |ui| {
                if ui
                    .add_enabled(
                        has_tree && st.dups.running.is_none(),
                        egui::Button::new(tr(lang, Key::MenuFindDuplicates)),
                    )
                    .clicked()
                {
                    st.actions.push(Action::StartDuplicates);
                    ui.close();
                }
                if ui
                    .add_enabled(
                        io_free && has_tree,
                        egui::Button::new(tr(lang, Key::MenuCompare)),
                    )
                    .clicked()
                {
                    st.tab = Tab::Delta;
                    st.actions.push(Action::ChooseCompare);
                    ui.close();
                }
                if ui
                    .add_enabled(io_free, egui::Button::new(tr(lang, Key::MenuHistory)))
                    .clicked()
                {
                    st.tab = Tab::Delta;
                    st.delta.show_history = true;
                    st.actions.push(Action::ChooseHistory);
                    ui.close();
                }
                ui.separator();
                let mut mft = st.settings.prefer_mft;
                if ui
                    .checkbox(&mut mft, tr(lang, Key::TargetPreferMft))
                    .changed()
                {
                    st.settings.prefer_mft = mft;
                    st.save_settings();
                }
                if !jt_core::platform::is_admin()
                    && ui
                        .add_enabled(
                            cfg!(windows),
                            egui::Button::new(tr(lang, Key::MenuRestartAsAdmin)),
                        )
                        .clicked()
                {
                    st.actions.push(Action::RestartAsAdmin);
                    ui.close();
                }
            });
            ui.menu_button(tr(lang, Key::MenuHelp), |ui| {
                if ui.button(tr(lang, Key::MenuAbout)).clicked() {
                    st.dialogs.about = true;
                    ui.close();
                }
            });
        });
    }

    fn target_bar(&mut self, ui: &mut egui::Ui) {
        let lang = self.state.lang();
        let st = &mut self.state;
        ui.horizontal(|ui| {
            ui.label(tr(lang, Key::TargetLabel));
            egui::ComboBox::from_id_salt("jt_target_combo")
                .selected_text(tr(lang, Key::TargetDrives))
                .width(110.0)
                .show_ui(ui, |ui| {
                    for d in &st.drives {
                        if ui.selectable_label(st.target_input == *d, d).clicked() {
                            st.target_input = d.clone();
                        }
                    }
                    if !st.settings.recent_targets.is_empty() {
                        ui.separator();
                        ui.label(tr(lang, Key::TargetRecent));
                        for r in &st.settings.recent_targets {
                            if ui.selectable_label(st.target_input == *r, r).clicked() {
                                st.target_input = r.clone();
                            }
                        }
                    }
                });
            let edit = ui.add(
                egui::TextEdit::singleline(&mut st.target_input)
                    .hint_text(tr(lang, Key::TargetPathHint))
                    .desired_width(320.0),
            );
            let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if ui
                .add_enabled(
                    st.io_busy.is_none(),
                    egui::Button::new(tr(lang, Key::Browse)),
                )
                .clicked()
            {
                st.actions.push(Action::BrowseFolder);
            }
            if st.scan.is_some() {
                ui.spinner();
                if ui.button(tr(lang, Key::MenuStopScan)).clicked() {
                    st.actions.push(Action::StopScan);
                }
            } else if ui.button(tr(lang, Key::MenuScan)).clicked() || enter {
                st.actions.push(Action::StartScan);
            }
            if !jt_core::platform::is_admin() && cfg!(windows) {
                ui.separator();
                if ui
                    .button(tr(lang, Key::MenuRestartAsAdmin))
                    .on_hover_text(tr(lang, Key::ReadOnlyHint))
                    .clicked()
                {
                    st.actions.push(Action::RestartAsAdmin);
                }
            }
        });
    }
}

impl eframe::App for JtApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.poll_jobs();
        self.state.now = jt_core::filter::now();

        let title = self.state.window_title();
        if title != self.last_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.last_title = title;
        }

        egui::Panel::top("jt_menu").show(ui, |ui| self.menu_bar(ui));
        egui::Panel::top("jt_target").show(ui, |ui| self.target_bar(ui));
        egui::Panel::bottom("jt_footer").show(ui, |ui| footer::show(ui, &self.state));
        egui::Panel::right("jt_details")
            .resizable(true)
            .default_size(300.0)
            .show(ui, |ui| views::details::show(ui, &mut self.state));
        egui::CentralPanel::default().show(ui, |ui| views::show(ui, &mut self.state));
        dialogs::show(&ctx, &mut self.state);

        self.process_actions(&ctx);

        let st = &self.state;
        if st.scan.is_some()
            || st.dups.running.is_some()
            || st.io_busy.is_some()
            || st.delete_running
            || st.details.loading.is_some()
        {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
    }
}

fn same_root(a: &str, b: &str) -> bool {
    let n = |s: &str| s.trim_end_matches(['\\', '/']).to_lowercase();
    n(a) == n(b)
}

fn snapshot_error(lang: Lang, e: &jt_core::snapshot::SnapError, load: bool) -> String {
    use jt_core::snapshot::SnapError;
    match e {
        SnapError::BadMagic => tr(lang, Key::ErrSnapshotMagic).to_string(),
        SnapError::UnsupportedVersion(_) => tr(lang, Key::ErrSnapshotVersion).to_string(),
        _ => {
            let key = if load {
                Key::ErrSnapshotLoad
            } else {
                Key::ErrSnapshotSave
            };
            fill(tr(lang, key), &[("{err}", &e.to_string())])
        }
    }
}

/// Vorschlag für den Dateinamen: Ziel + Datum, nur sichere Zeichen.
fn snapshot_file_name(tree: &Tree) -> String {
    let root: String = tree
        .meta
        .root_path
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect();
    let root = root.trim_matches('_');
    let date = chrono_like_date(jt_core::filter::now());
    format!("JoschysTree_{root}_{date}.jtscan")
}

/// JJJJ-MM-TT ohne zusätzliche Abhängigkeit (proleptischer Gregorianischer Kalender).
fn chrono_like_date(t: FileTime) -> String {
    let days = t.to_unix_secs().div_euclid(86_400);
    // Algorithmus von Howard Hinnant (civil_from_days).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

/// Lädt alle .jtscan eines Ordners mit passendem Ziel und rechnet den Verlauf.
fn load_history(
    dir: &std::path::Path,
    want_root: Option<String>,
    sel_path: Option<String>,
    depth: u32,
) -> Option<HistoryResult> {
    let mut snaps: Vec<Tree> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("jtscan"))
        })
        .filter_map(|p| jt_core::snapshot::load(&p).ok())
        .collect();
    let root = want_root.or_else(|| snaps.last().map(|t| t.meta.root_path.clone()))?;
    snaps.retain(|t| same_root(&t.meta.root_path, &root));
    if snaps.len() < 2 {
        return None;
    }
    snaps.sort_by_key(|t| t.meta.started);
    let path = sel_path.unwrap_or(root);
    Some(HistoryResult {
        series: jt_core::history::series_for_path(&snaps, &path),
        growers: jt_core::history::top_growers(&snaps, depth, 50),
        snapshots: snaps.len(),
        path,
    })
}

/// Startet das Programm per ShellExecuteW mit "runas" neu (UAC-Abfrage).
#[cfg(windows)]
fn restart_as_admin() -> std::io::Result<()> {
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "shell32")]
    unsafe extern "system" {
        fn ShellExecuteW(
            hwnd: *mut c_void,
            operation: *const u16,
            file: *const u16,
            parameters: *const u16,
            directory: *const u16,
            show_cmd: i32,
        ) -> *mut c_void;
    }

    let exe = std::env::current_exe()?;
    let file: Vec<u16> = exe.as_os_str().encode_wide().chain(Some(0)).collect();
    let verb: Vec<u16> = "runas".encode_utf16().chain(Some(0)).collect();
    // SAFETY: Beide Puffer sind nullterminiertes UTF-16 und leben bis nach dem Aufruf;
    // die übrigen Zeiger dürfen laut Win32-Doku NULL sein. 1 = SW_SHOWNORMAL.
    let r = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            file.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            1,
        )
    };
    // Laut Doku: Rückgabewert > 32 bedeutet Erfolg.
    if r as usize > 32 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(not(windows))]
fn restart_as_admin() -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "only on Windows",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use jt_core::model::testutil::sample_tree;

    #[test]
    fn can_delete_is_false_on_linux() {
        #[cfg(not(windows))]
        assert!(!can_delete());
        assert_eq!(can_delete(), jt_core::platform::is_admin());
    }

    #[test]
    fn fill_replaces_placeholders() {
        assert_eq!(
            fill("{n} von {total}", &[("{n}", "3"), ("{total}", "7")]),
            "3 von 7"
        );
    }

    #[test]
    fn unlink_and_recompute() {
        let mut t = sample_tree();
        let users = t.find_path("C:\\Users").unwrap();
        let joschy = t.find_path("C:\\Users\\joschy").unwrap();
        let readme = t.find_path("C:\\Users\\readme.md").unwrap();
        unlink_node(&mut t, joschy);
        jt_core::aggregate::recompute(&mut t);
        assert_eq!(t.node(users).size, 500);
        assert_eq!(t.node(0).size, 63_600 - 3_100);
        assert!(!is_attached(&t, joschy));
        assert!(is_attached(&t, readme));
        // letztes Kind entfernen
        let page = t.find_path("C:\\pagefile.sys").unwrap();
        unlink_node(&mut t, page);
        jt_core::aggregate::recompute(&mut t);
        assert_eq!(t.node(0).size, 10_500);
        assert_eq!(t.children(0).count(), 2);
    }

    #[test]
    fn date_format() {
        assert_eq!(chrono_like_date(FileTime::from_unix_secs(0)), "1970-01-01");
        assert_eq!(
            chrono_like_date(FileTime::from_unix_secs(1_793_318_400)),
            "2026-10-30"
        );
    }

    #[test]
    fn root_comparison() {
        assert!(same_root("C:\\", "c:"));
        assert!(!same_root("C:\\", "D:\\"));
    }
}
