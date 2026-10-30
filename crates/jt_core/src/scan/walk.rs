// Joschys_Tree – Paralleler Verzeichnis-Scan (std::fs + rayon) – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Paralleler Walk: pro Unterordner eine rayon-Aufgabe. Reparse-Points (Symlinks,
//! Junctions) werden nicht verfolgt, Zugriffsfehler gezählt und als `ACCESS_DENIED`
//! markiert, der Scan bricht dabei nie ab. Das `cancel`-Flag wird pro Ordner und
//! pro Eintrag geprüft. Fortschritt wird nur im aufrufenden Thread gemeldet.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use rayon::prelude::*;

use super::{Phase, ScanError, ScanProgress, ScanRequest};
use crate::model::{EntryInfo, FileTime, NodeFlags, NodeId, ScanMeta, ScanMode, Tree, TreeBuilder};

/// Schutz gegen Endlos-Schleifen, falls `follow_reparse` doch einmal aktiv ist.
const MAX_DEPTH: u32 = 512;

struct Ctx<'a> {
    cancel: &'a AtomicBool,
    follow_reparse: bool,
    cluster: u64,
    entries: AtomicU64,
    bytes: AtomicU64,
    errors: AtomicU32,
    current: Mutex<String>,
}

impl Ctx<'_> {
    fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }
}

struct WEntry {
    name: String,
    info: EntryInfo,
    is_dir: bool,
    children: Vec<WEntry>,
}

fn round_up(n: u64, cluster: u64) -> u64 {
    if cluster == 0 {
        n
    } else {
        n.div_ceil(cluster).saturating_mul(cluster)
    }
}

#[cfg(not(windows))]
fn sys_time(t: std::io::Result<std::time::SystemTime>) -> FileTime {
    match t
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
    {
        Some(d) => FileTime(
            FileTime::from_unix_secs(d.as_secs() as i64).0 + (d.subsec_nanos() / 100) as i64,
        ),
        None => FileTime::default(),
    }
}

/// Metadaten → EntryInfo (Unix/Linux: Entwicklung und Tests).
#[cfg(not(windows))]
fn entry_info(md: &fs::Metadata, _path: &Path, ctx: &Ctx<'_>) -> EntryInfo {
    use std::os::unix::fs::MetadataExt;
    let mut flags = NodeFlags::empty();
    if md.file_type().is_symlink() {
        flags |= NodeFlags::REPARSE;
    }
    if md.permissions().readonly() {
        flags |= NodeFlags::READONLY;
    }
    let size = if md.is_file() { md.len() } else { 0 };
    let real = md.blocks().saturating_mul(512);
    let allocated = if md.is_file() {
        if real > 0 || size == 0 {
            real
        } else {
            round_up(size, ctx.cluster)
        }
    } else {
        0
    };
    if allocated < size && size > 0 {
        flags |= NodeFlags::SPARSE;
    }
    EntryInfo {
        size,
        allocated,
        created: sys_time(md.created()),
        modified: sys_time(md.modified()),
        accessed: sys_time(md.accessed()),
        flags,
        ads_size: 0,
    }
}

/// Metadaten → EntryInfo (Windows: Attribute, FILETIMEs, komprimierte Größe).
#[cfg(windows)]
fn entry_info(md: &fs::Metadata, path: &Path, ctx: &Ctx<'_>) -> EntryInfo {
    use std::os::windows::fs::MetadataExt;
    let attrs = md.file_attributes();
    let mut flags = super::mft_parse::flags_from_win_attributes(attrs);
    flags.remove(NodeFlags::DIR);
    let is_dir = attrs & 0x10 != 0;
    let size = if is_dir { 0 } else { md.file_size() };
    let allocated = if is_dir {
        0
    } else if flags.intersects(NodeFlags::COMPRESSED | NodeFlags::SPARSE) {
        crate::platform::compressed_size(&path.to_string_lossy())
            .map(|s| round_up(s, ctx.cluster))
            .unwrap_or_else(|_| round_up(size, ctx.cluster))
    } else if flags.contains(NodeFlags::OFFLINE) {
        0
    } else {
        round_up(size, ctx.cluster)
    };
    EntryInfo {
        size,
        allocated,
        created: FileTime(md.creation_time() as i64),
        modified: FileTime(md.last_write_time() as i64),
        accessed: FileTime(md.last_access_time() as i64),
        flags,
        ads_size: 0,
    }
}

/// Liest einen Ordner und verarbeitet die Unterordner parallel.
/// Rückgabe: (Kinder, Ordner war nicht lesbar).
fn walk_dir(path: &Path, depth: u32, ctx: &Ctx<'_>) -> (Vec<WEntry>, bool) {
    if ctx.cancelled() {
        return (Vec::new(), false);
    }
    if let Ok(mut cur) = ctx.current.try_lock() {
        cur.clear();
        cur.push_str(&path.to_string_lossy());
    }
    let rd = match fs::read_dir(path) {
        Ok(rd) => rd,
        Err(_) => {
            ctx.errors.fetch_add(1, Ordering::Relaxed);
            return (Vec::new(), true);
        }
    };
    let mut out: Vec<WEntry> = Vec::new();
    for item in rd {
        if ctx.cancelled() {
            break;
        }
        let de = match item {
            Ok(de) => de,
            Err(_) => {
                ctx.errors.fetch_add(1, Ordering::Relaxed);
                continue;
            }
        };
        let name = de.file_name().to_string_lossy().into_owned();
        let child_path = de.path();
        // DirEntry::metadata folgt Symlinks/Junctions nicht.
        let md = match de.metadata() {
            Ok(md) => md,
            Err(_) => {
                ctx.errors.fetch_add(1, Ordering::Relaxed);
                ctx.entries.fetch_add(1, Ordering::Relaxed);
                out.push(WEntry {
                    name,
                    info: EntryInfo {
                        flags: NodeFlags::ACCESS_DENIED,
                        ..Default::default()
                    },
                    is_dir: false,
                    children: Vec::new(),
                });
                continue;
            }
        };
        let mut info = entry_info(&md, &child_path, ctx);
        let is_link = md.file_type().is_symlink() || info.flags.contains(NodeFlags::REPARSE);
        let is_dir = if is_link {
            info.flags |= NodeFlags::REPARSE;
            // Ziel nur ansehen, um Ordner-Links als Ordner anzuzeigen; nie hineingehen.
            info.size = 0;
            info.allocated = 0;
            md.is_dir()
                || fs::metadata(&child_path)
                    .map(|m| m.is_dir())
                    .unwrap_or(false)
        } else {
            md.is_dir()
        };
        ctx.entries.fetch_add(1, Ordering::Relaxed);
        ctx.bytes.fetch_add(info.size, Ordering::Relaxed);
        out.push(WEntry {
            name,
            info,
            is_dir,
            children: Vec::new(),
        });
    }
    if ctx.cancelled() {
        return (out, false);
    }
    // Unterordner parallel: eine rayon-Aufgabe pro Unterordner.
    out.par_iter_mut()
        .filter(|e| {
            e.is_dir
                && (!e.info.flags.contains(NodeFlags::REPARSE) || ctx.follow_reparse)
                && depth < MAX_DEPTH
        })
        .for_each(|e| {
            let (children, denied) = walk_dir(&path.join(&e.name), depth + 1, ctx);
            e.children = children;
            if denied {
                e.info.flags |= NodeFlags::ACCESS_DENIED;
            }
        });
    (out, false)
}

/// Überträgt die Walk-Ergebnisse iterativ in den TreeBuilder (keine tiefe Rekursion).
fn build(b: &mut TreeBuilder, root_children: Vec<WEntry>) {
    let mut stack: Vec<(NodeId, Vec<WEntry>)> = vec![(0, root_children)];
    while let Some((parent, entries)) = stack.pop() {
        for e in entries {
            if e.is_dir {
                let id = b.add_dir(parent, &e.name, e.info);
                if !e.children.is_empty() {
                    stack.push((id, e.children));
                }
            } else {
                b.add_file(parent, &e.name, e.info);
            }
        }
    }
}

pub(crate) fn scan_walk(
    req: &ScanRequest,
    progress: &dyn Fn(&ScanProgress),
    cancel: &AtomicBool,
) -> Result<Tree, ScanError> {
    let t0 = Instant::now();
    let started = super::now_filetime();
    let root_path = PathBuf::from(crate::platform::long_path(&req.root));
    let root_md = fs::metadata(&root_path).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => ScanError::NotFound(req.root.clone()),
        _ => ScanError::Io(e),
    })?;
    if !root_md.is_dir() {
        return Err(ScanError::NotFound(req.root.clone()));
    }

    let mut meta = ScanMeta::new(req.root.clone(), ScanMode::Walk);
    meta.started = started;
    if let Ok((_fs, cluster, total, free)) = crate::platform::volume_info(&req.root) {
        meta.cluster_size = cluster;
        meta.volume_total = total;
        meta.volume_free = free;
    }

    let ctx = Ctx {
        cancel,
        follow_reparse: req.follow_reparse,
        cluster: meta.cluster_size as u64,
        entries: AtomicU64::new(0),
        bytes: AtomicU64::new(0),
        errors: AtomicU32::new(0),
        current: Mutex::new(String::new()),
    };

    let report = |phase: Phase| {
        let current = ctx.current.lock().map(|c| c.clone()).unwrap_or_default();
        progress(&ScanProgress {
            entries: ctx.entries.load(Ordering::Relaxed),
            bytes: ctx.bytes.load(Ordering::Relaxed),
            current,
            phase,
        });
    };
    report(Phase::Starting);

    // Eigener Pool mit großem Stack (tiefe Ordnerstrukturen). Der Walk läuft in einem
    // Hilfsthread, der aufrufende Thread meldet derweil den Fortschritt.
    let pool = rayon::ThreadPoolBuilder::new()
        .stack_size(16 * 1024 * 1024)
        .thread_name(|i| format!("jt-walk-{i}"))
        .build()
        .map_err(|e| ScanError::Io(std::io::Error::other(e.to_string())))?;
    let (children, root_denied) = std::thread::scope(|s| {
        let (tx, rx) = mpsc::channel();
        let ctx_ref = &ctx;
        let pool_ref = &pool;
        let root_ref = &root_path;
        s.spawn(move || {
            let r = pool_ref.install(|| walk_dir(root_ref, 0, ctx_ref));
            let _ = tx.send(r);
        });
        loop {
            match rx.recv_timeout(Duration::from_millis(100)) {
                Ok(r) => break r,
                Err(mpsc::RecvTimeoutError::Timeout) => report(Phase::Walking),
                Err(mpsc::RecvTimeoutError::Disconnected) => break (Vec::new(), true),
            }
        }
    });

    if cancel.load(Ordering::Relaxed) {
        return Err(ScanError::Cancelled);
    }
    if root_denied && children.is_empty() {
        return Err(ScanError::Io(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            format!("Ordner nicht lesbar: {}", req.root),
        )));
    }

    report(Phase::Building);
    meta.errors = ctx.errors.load(Ordering::Relaxed);
    meta.duration_ms = t0.elapsed().as_millis() as u64;
    let mut b = TreeBuilder::new(meta, &req.root);
    build(&mut b, children);
    report(Phase::Aggregating);
    let tree = b.finish();
    report(Phase::Done);
    Ok(tree)
}
