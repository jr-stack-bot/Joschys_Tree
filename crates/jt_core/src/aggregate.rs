// Joschys_Tree – Größen/Cluster/Zähler nach oben rechnen, Prozent, Sortierung – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::cmp::Ordering;

use crate::model::{NO_NODE, NodeId, Tree};

/// Bottom-up size/allocated/file_count/dir_count für alle Ordner neu berechnen.
/// Dateiwerte (size/allocated) bleiben unverändert; Ordnerwerte = Summe der Kinder.
pub fn recompute(tree: &mut Tree) {
    if tree.nodes.is_empty() {
        return;
    }
    // Post-Order iterativ (Knoten sind nicht zwingend topologisch sortiert).
    let mut order: Vec<NodeId> = Vec::with_capacity(tree.nodes.len());
    let mut stack: Vec<NodeId> = vec![tree.root()];
    while let Some(id) = stack.pop() {
        order.push(id);
        let mut c = tree.nodes[id as usize].first_child;
        while c != NO_NODE {
            stack.push(c);
            c = tree.nodes[c as usize].next_sibling;
        }
    }
    for &id in order.iter().rev() {
        let n = &tree.nodes[id as usize];
        if !n.is_dir() {
            continue;
        }
        let (mut size, mut alloc, mut files, mut dirs) = (0u64, 0u64, 0u32, 0u32);
        let mut c = n.first_child;
        while c != NO_NODE {
            let k = &tree.nodes[c as usize];
            size += k.size;
            alloc += k.allocated;
            if k.is_dir() {
                dirs += 1 + k.dir_count;
                files += k.file_count;
            } else {
                files += 1;
            }
            c = k.next_sibling;
        }
        let n = &mut tree.nodes[id as usize];
        n.size = size;
        n.allocated = alloc;
        n.file_count = files;
        n.dir_count = dirs;
    }
}

/// Anteil der Größe am Elternordner in Prozent (0.0–100.0). Wurzel = 100, leerer Elternordner = 0.
pub fn percent_of_parent(tree: &Tree, id: NodeId) -> f32 {
    let n = tree.node(id);
    if n.parent == NO_NODE {
        return 100.0;
    }
    let p = tree.node(n.parent).size;
    if p == 0 {
        0.0
    } else {
        (n.size as f64 * 100.0 / p as f64) as f32
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SortKey {
    Name,
    Size,
    Allocated,
    Files,
    Modified,
}

/// Vergleicht zwei Namen ohne Groß-/Kleinschreibung und ohne Allocation.
pub(crate) fn cmp_name_ci(a: &str, b: &str) -> Ordering {
    if a.is_ascii() && b.is_ascii() {
        let (ab, bb) = (a.as_bytes(), b.as_bytes());
        for (x, y) in ab.iter().zip(bb) {
            match x.to_ascii_lowercase().cmp(&y.to_ascii_lowercase()) {
                Ordering::Equal => {}
                o => return o,
            }
        }
        return ab.len().cmp(&bb.len());
    }
    a.chars()
        .flat_map(char::to_lowercase)
        .cmp(b.chars().flat_map(char::to_lowercase))
}

/// Direkte Kinder sortiert. Bei Gleichstand entscheidet der Name (immer aufsteigend), dann die Id.
pub fn sorted_children(tree: &Tree, id: NodeId, key: SortKey, desc: bool) -> Vec<NodeId> {
    let mut v: Vec<NodeId> = tree.children(id).collect();
    let nodes = &tree.nodes;
    v.sort_unstable_by(|&a, &b| {
        let (na, nb) = (&nodes[a as usize], &nodes[b as usize]);
        let primary = match key {
            SortKey::Name => cmp_name_ci(&na.name, &nb.name),
            SortKey::Size => na.size.cmp(&nb.size),
            SortKey::Allocated => na.allocated.cmp(&nb.allocated),
            SortKey::Files => na.file_count.cmp(&nb.file_count),
            SortKey::Modified => na.modified.cmp(&nb.modified),
        };
        let primary = if desc { primary.reverse() } else { primary };
        primary
            .then_with(|| {
                if key == SortKey::Name {
                    Ordering::Equal
                } else {
                    cmp_name_ci(&na.name, &nb.name)
                }
            })
            .then(a.cmp(&b))
    });
    v
}

/// Ruft `f` für jeden Nachfahren von `scope` auf (Pre-Order, ohne `scope` selbst).
/// Bricht ab, sobald `f` false liefert. Nur eine Allocation (der Stapel).
pub(crate) fn for_each_descendant(tree: &Tree, scope: NodeId, mut f: impl FnMut(NodeId) -> bool) {
    let nodes = &tree.nodes;
    if scope as usize >= nodes.len() {
        return;
    }
    let mut stack: Vec<NodeId> = Vec::with_capacity(64);
    let fc = nodes[scope as usize].first_child;
    if fc != NO_NODE {
        stack.push(fc);
    }
    // Stapel enthält "nächster zu besuchender Knoten"; Geschwister werden nachgeschoben.
    while let Some(id) = stack.pop() {
        let n = &nodes[id as usize];
        if n.next_sibling != NO_NODE {
            stack.push(n.next_sibling);
        }
        if !f(id) {
            return;
        }
        if n.first_child != NO_NODE {
            stack.push(n.first_child);
        }
    }
}
