// Joschys_Tree – Top-Dateien, Dateitypen-Statistik, Alters-Histogramm – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use crate::aggregate::for_each_descendant;
use crate::filter::{CompiledFilter, Filter};
use crate::model::{FileTime, NodeId, Tree};

/// Die `n` größten Dateien (nur Dateien, keine Ordner), nach Größe absteigend.
/// Bounded-Min-Heap: O(N log n) Zeit, O(n) Speicher. Gleich große Dateien: kleinere Id zuerst.
pub fn top_files(tree: &Tree, n: usize, filter: Option<&Filter>) -> Vec<NodeId> {
    if n == 0 || tree.nodes.is_empty() {
        return Vec::new();
    }
    let compiled = filter.map(|f| CompiledFilter::new(f, tree, crate::filter::now()));
    // Min-Heap über (size, Reverse(id)): Kopf = kleinster Kandidat, bei Gleichstand größte Id.
    let mut heap: BinaryHeap<Reverse<(u64, Reverse<NodeId>)>> =
        BinaryHeap::with_capacity(n.min(tree.nodes.len()) + 1);
    for_each_descendant(tree, tree.root(), |id| {
        let node = &tree.nodes[id as usize];
        if node.is_dir() {
            return true;
        }
        let key = (node.size, Reverse(id));
        if heap.len() == n {
            // Billiger Vorab-Test, bevor der Filter läuft.
            if let Some(Reverse(min)) = heap.peek()
                && key <= *min
            {
                return true;
            }
        }
        if let Some(cf) = &compiled
            && !cf.matches(tree, id)
        {
            return true;
        }
        if heap.len() == n {
            heap.pop();
        }
        heap.push(Reverse(key));
        true
    });
    let mut v: Vec<(u64, Reverse<NodeId>)> = heap.into_iter().map(|Reverse(k)| k).collect();
    v.sort_unstable_by(|a, b| b.cmp(a));
    v.into_iter().map(|(_, Reverse(id))| id).collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypeStat {
    pub ext: String,
    pub files: u64,
    pub size: u64,
    pub allocated: u64,
}

/// Statistik je Endung unter `scope`, nach size absteigend (Gleichstand: Endung aufsteigend).
/// Dateien ohne Endung erscheinen mit `ext == ""`.
pub fn file_types(tree: &Tree, scope: NodeId) -> Vec<TypeStat> {
    // Indexiert über ext_id: keine Hash-Lookups, keine Strings je Datei.
    let mut acc: Vec<(u64, u64, u64)> = vec![(0, 0, 0); tree.extensions.len().max(1)];
    let mut visit = |id: NodeId| {
        let n = &tree.nodes[id as usize];
        if !n.is_dir() {
            let a = &mut acc[n.ext_id as usize];
            a.0 += 1;
            a.1 += n.size;
            a.2 += n.allocated;
        }
    };
    if (scope as usize) < tree.nodes.len() && !tree.nodes[scope as usize].is_dir() {
        visit(scope);
    } else {
        for_each_descendant(tree, scope, |id| {
            visit(id);
            true
        });
    }
    let mut out: Vec<TypeStat> = acc
        .iter()
        .enumerate()
        .filter(|(_, a)| a.0 > 0)
        .map(|(i, a)| TypeStat {
            ext: tree.extensions[i].to_string(),
            files: a.0,
            size: a.1,
            allocated: a.2,
        })
        .collect();
    out.sort_unstable_by(|a, b| b.size.cmp(&a.size).then_with(|| a.ext.cmp(&b.ext)));
    out
}

const DAY_SECS: i64 = 86_400;
/// Obergrenzen der Altersklassen in Tagen: 1 Monat, 3 Monate, 6 Monate, 1 Jahr, 3 Jahre.
const AGE_LIMITS_DAYS: [i64; 5] = [30, 91, 182, 365, 3 * 365];

/// Bytes je Altersklasse (nach `modified`): <1M, <3M, <6M, <1J, <3J, >=3J.
/// Unbekannte Zeit (0) zählt als ">=3J"; Zeiten in der Zukunft als "<1M".
pub fn age_histogram(tree: &Tree, scope: NodeId, now: FileTime) -> [u64; 6] {
    let mut h = [0u64; 6];
    let now_s = now.to_unix_secs();
    let mut visit = |id: NodeId| {
        let n = &tree.nodes[id as usize];
        if n.is_dir() {
            return;
        }
        let bucket = if n.modified.0 == 0 {
            5
        } else {
            let age_days = (now_s - n.modified.to_unix_secs()) / DAY_SECS;
            AGE_LIMITS_DAYS
                .iter()
                .position(|&lim| age_days < lim)
                .unwrap_or(5)
        };
        h[bucket] += n.size;
    };
    if (scope as usize) < tree.nodes.len() && !tree.nodes[scope as usize].is_dir() {
        visit(scope);
    } else {
        for_each_descendant(tree, scope, |id| {
            visit(id);
            true
        });
    }
    h
}
