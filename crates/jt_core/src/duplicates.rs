// Joschys_Tree – Duplikat-Suche (Größe, Teil-Hash, Voll-Hash xxh3-128) – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Duplikat-Suche in drei Stufen:
//! 1. Gruppieren nach Größe (Dateien mit eindeutiger Größe werden nie gelesen),
//! 2. xxh3-128 über die ersten 4 KiB,
//! 3. xxh3-128 über die ganze Datei (nur wenn größer als 4 KiB).
//!
//! Stufe 2 und 3 laufen mit rayon parallel. Lesefehler überspringen die Datei,
//! `cancel` liefert die bis dahin bestätigten Gruppen.

use std::collections::HashMap;
use std::io::{self, Read};
use std::sync::atomic::{AtomicBool, Ordering};

use rayon::prelude::*;
use xxhash_rust::xxh3::{xxh3_128, Xxh3};

use crate::model::{NodeFlags, NodeId, Tree, NO_NODE};

/// Größe des Teil-Hashes (Stufe 2).
pub const HEAD_SIZE: usize = 4096;

/// Wie viele Dateien höchstens pro paralleler Runde bearbeitet werden
/// (dazwischen: Fortschritt melden, Abbruch prüfen).
const BATCH_FILES: usize = 256;

pub trait FileReader: Sync {
    fn read_head(&self, path: &str, n: usize) -> io::Result<Vec<u8>>;
    fn hash_full(&self, path: &str, cancel: &AtomicBool) -> io::Result<u128>;
}

/// std::fs + xxh3
pub struct StdFileReader;

impl StdFileReader {
    /// Baumpfad (Windows-Trenner) in einen Pfad für das Betriebssystem umwandeln.
    fn os_path(path: &str) -> String {
        #[cfg(windows)]
        {
            crate::platform::long_path(path)
        }
        #[cfg(not(windows))]
        {
            path.replace('\\', "/")
        }
    }
}

impl FileReader for StdFileReader {
    fn read_head(&self, path: &str, n: usize) -> io::Result<Vec<u8>> {
        let f = std::fs::File::open(Self::os_path(path))?;
        let mut buf = Vec::with_capacity(n);
        f.take(n as u64).read_to_end(&mut buf)?;
        Ok(buf)
    }

    fn hash_full(&self, path: &str, cancel: &AtomicBool) -> io::Result<u128> {
        let mut f = std::fs::File::open(Self::os_path(path))?;
        let mut hasher = Xxh3::new();
        let mut buf = vec![0u8; 1 << 20];
        loop {
            if cancel.load(Ordering::Relaxed) {
                return Err(io::Error::new(io::ErrorKind::Interrupted, "abgebrochen"));
            }
            let n = match f.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => n,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            };
            hasher.update(&buf[..n]);
        }
        Ok(hasher.digest128())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DupGroup {
    pub size: u64,
    pub hash: u128,
    pub files: Vec<NodeId>,
}

impl DupGroup {
    /// Einsparpotenzial: (n-1) * size.
    pub fn savings(&self) -> u64 {
        (self.files.len().saturating_sub(1) as u64).saturating_mul(self.size)
    }
}

/// Alle Dateien unterhalb von `scope` (inklusive) mit `size >= min_size` und `size > 0`.
/// Reparse-Points, Offline-Dateien und Einträge ohne Zugriff werden übersprungen.
fn collect_files(tree: &Tree, scope: NodeId, min_size: u64) -> Vec<NodeId> {
    let mut out = Vec::new();
    if tree.nodes.is_empty() || scope as usize >= tree.nodes.len() {
        return out;
    }
    let skip = NodeFlags::REPARSE | NodeFlags::OFFLINE | NodeFlags::ACCESS_DENIED;
    let mut stack = vec![scope];
    while let Some(id) = stack.pop() {
        let n = tree.node(id);
        if n.is_dir() {
            if n.flags.contains(NodeFlags::REPARSE) {
                continue;
            }
            let mut c = n.first_child;
            while c != NO_NODE {
                stack.push(c);
                c = tree.node(c).next_sibling;
            }
        } else if n.size > 0 && n.size >= min_size && !n.flags.intersects(skip) {
            out.push(id);
        }
    }
    out
}

/// Ergebnis einer Datei in einer Hash-Stufe.
type Hashed = (NodeId, Option<u128>);

/// Bearbeitet einen Block von Größen-Gruppen vollständig (Stufe 2 und 3).
fn process_batch(
    tree: &Tree,
    batch: &[(u64, Vec<NodeId>)],
    reader: &dyn FileReader,
    cancel: &AtomicBool,
    out: &mut Vec<DupGroup>,
) {
    // Stufe 2: Teil-Hash über die ersten 4 KiB.
    let jobs: Vec<(u64, NodeId)> = batch
        .iter()
        .flat_map(|(size, ids)| ids.iter().map(move |&id| (*size, id)))
        .collect();
    let heads: Vec<(u64, Hashed)> = jobs
        .par_iter()
        .map(|&(size, id)| {
            if cancel.load(Ordering::Relaxed) {
                return (size, (id, None));
            }
            let path = tree.full_path(id);
            let h = reader
                .read_head(&path, HEAD_SIZE)
                .ok()
                .map(|b| xxh3_128(&b));
            (size, (id, h))
        })
        .collect();

    let mut by_head: HashMap<(u64, u128), Vec<NodeId>> = HashMap::new();
    for (size, (id, h)) in heads {
        if let Some(h) = h {
            by_head.entry((size, h)).or_default().push(id);
        }
    }

    // Kleine Dateien: Teil-Hash = Voll-Hash, fertig. Große: Stufe 3.
    let mut full_jobs: Vec<(u64, NodeId)> = Vec::new();
    for ((size, h), ids) in by_head {
        if ids.len() < 2 {
            continue;
        }
        if size <= HEAD_SIZE as u64 {
            out.push(DupGroup {
                size,
                hash: h,
                files: ids,
            });
        } else {
            full_jobs.extend(ids.into_iter().map(|id| (size, id)));
        }
    }
    if full_jobs.is_empty() {
        return;
    }

    // Stufe 3: Voll-Hash.
    let fulls: Vec<(u64, Hashed)> = full_jobs
        .par_iter()
        .map(|&(size, id)| {
            if cancel.load(Ordering::Relaxed) {
                return (size, (id, None));
            }
            let path = tree.full_path(id);
            (size, (id, reader.hash_full(&path, cancel).ok()))
        })
        .collect();
    let mut by_full: HashMap<(u64, u128), Vec<NodeId>> = HashMap::new();
    for (size, (id, h)) in fulls {
        if let Some(h) = h {
            by_full.entry((size, h)).or_default().push(id);
        }
    }
    out.extend(
        by_full
            .into_iter()
            .filter(|(_, ids)| ids.len() >= 2)
            .map(|((size, hash), files)| DupGroup { size, hash, files }),
    );
}

/// Nach Einsparpotenzial sortiert.
///
/// `progress(erledigt, gesamt)` zählt Bytes der Kandidaten (Dateien, deren Größe
/// mindestens zweimal vorkommt) und wird nur aus dem aufrufenden Thread gerufen.
pub fn find_duplicates(
    tree: &Tree,
    scope: NodeId,
    min_size: u64,
    reader: &dyn FileReader,
    progress: &dyn Fn(u64, u64),
    cancel: &AtomicBool,
) -> Vec<DupGroup> {
    // Stufe 1: nach Größe gruppieren.
    let mut by_size: HashMap<u64, Vec<NodeId>> = HashMap::new();
    for id in collect_files(tree, scope, min_size) {
        by_size.entry(tree.node(id).size).or_default().push(id);
    }
    let mut groups: Vec<(u64, Vec<NodeId>)> =
        by_size.into_iter().filter(|(_, v)| v.len() >= 2).collect();
    // Größte zuerst: bei Abbruch sind die wertvollsten Gruppen schon fertig.
    groups.sort_unstable_by_key(|g| std::cmp::Reverse(g.0));

    let total: u64 = groups
        .iter()
        .map(|(s, v)| s.saturating_mul(v.len() as u64))
        .fold(0, u64::saturating_add);
    let mut done = 0u64;
    progress(done, total);

    let mut result = Vec::new();
    let mut i = 0;
    while i < groups.len() {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let start = i;
        let mut files = 0;
        while i < groups.len() && (files == 0 || files + groups[i].1.len() <= BATCH_FILES) {
            files += groups[i].1.len();
            i += 1;
        }
        let batch = &groups[start..i];
        process_batch(tree, batch, reader, cancel, &mut result);
        done = batch
            .iter()
            .map(|(s, v)| s.saturating_mul(v.len() as u64))
            .fold(done, u64::saturating_add);
        progress(done.min(total), total);
    }

    for g in &mut result {
        g.files.sort_unstable();
    }
    result.sort_by(|a, b| {
        b.savings()
            .cmp(&a.savings())
            .then(b.size.cmp(&a.size))
            .then(a.files.cmp(&b.files))
    });
    result
}
