// Joschys_Tree – Verlaufs-/Wachstumsanalyse über mehrere Snapshots – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Verlauf eines Pfades über mehrere Snapshots desselben Ziels und die
//! am stärksten gewachsenen Einträge zwischen ältestem und neuestem Snapshot.
//! Zeitpunkt eines Snapshots ist `meta.started`.

use std::collections::HashMap;

use crate::model::{FileTime, NodeId, Tree};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HistoryPoint {
    pub time: FileTime,
    pub size: u64,
    pub allocated: u64,
    pub files: u64,
}

/// Nach Zeit sortiert. Fehlt der Pfad in einem Snapshot, ist der Punkt 0.
/// `files` ist bei Ordnern die rekursive Dateizahl, bei einer Datei 1.
pub fn series_for_path(snapshots: &[Tree], path: &str) -> Vec<HistoryPoint> {
    let mut out: Vec<HistoryPoint> = snapshots
        .iter()
        .map(|t| {
            let time = t.meta.started;
            match t.find_path(path) {
                Some(id) => {
                    let n = t.node(id);
                    HistoryPoint {
                        time,
                        size: n.size,
                        allocated: n.allocated,
                        files: if n.is_dir() { n.file_count as u64 } else { 1 },
                    }
                }
                None => HistoryPoint {
                    time,
                    size: 0,
                    allocated: 0,
                    files: 0,
                },
            }
        })
        .collect();
    out.sort_by_key(|p| p.time);
    out
}

/// Alle Knoten genau in Tiefe `depth` (Wurzel = 0):
/// Schlüssel = kleingeschriebener relativer Pfad, Wert = (voller Pfad, Größe).
fn level_map(tree: &Tree, depth: u32) -> HashMap<String, (String, u64)> {
    let mut out = HashMap::new();
    if tree.nodes.is_empty() {
        return out;
    }
    let mut stack: Vec<(NodeId, u32, String)> = vec![(tree.root(), 0, String::new())];
    while let Some((id, d, key)) = stack.pop() {
        if d == depth {
            out.insert(key, (tree.full_path(id), tree.node(id).size));
            continue;
        }
        for c in tree.children(id) {
            let mut k = key.clone();
            k.push('\\');
            k.push_str(&tree.node(c).name.to_lowercase());
            stack.push((c, d + 1, k));
        }
    }
    out
}

/// Die `n` Einträge in Tiefe `depth` (Wurzel = 0, 1 = direkte Kinder) mit dem größten
/// Wachstum zwischen ältestem und neuestem Snapshot (Bytes, neu - alt).
/// Nur positives Wachstum, absteigend sortiert. Weniger als 2 Snapshots: leer.
pub fn top_growers(snapshots: &[Tree], depth: u32, n: usize) -> Vec<(String, i64)> {
    if snapshots.len() < 2 || n == 0 {
        return Vec::new();
    }
    let (Some(first), Some(last)) = (
        snapshots.iter().min_by_key(|t| t.meta.started),
        snapshots.iter().rev().max_by_key(|t| t.meta.started),
    ) else {
        return Vec::new();
    };
    let old = level_map(first, depth);
    let new = level_map(last, depth);
    let mut out: Vec<(String, i64)> = Vec::new();
    for (key, (path, size)) in &new {
        let before = old.get(key).map_or(0, |(_, s)| *s);
        let g = (*size as i128 - before as i128).clamp(0, i64::MAX as i128) as i64;
        if g > 0 {
            out.push((path.clone(), g));
        }
    }
    // Entfernte Einträge sind geschrumpft und damit nie "Wachstum".
    out.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    out.truncate(n);
    out
}
