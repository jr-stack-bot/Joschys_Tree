// Joschys_Tree – Delta-Vergleich zweier Trees – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Vergleicht zwei Scans desselben Ziels per Pfad (case-insensitiv, relativ zur Wurzel).
//! Beide Bäume werden gleichzeitig durchlaufen; Kinder werden über den
//! kleingeschriebenen Namen zugeordnet. Ordner und Dateien werden gemeldet.

use std::collections::HashMap;

use crate::model::{NO_NODE, NodeId, Tree};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangeKind {
    Added,
    Removed,
    Grown,
    Shrunk,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeltaEntry {
    pub path: String,
    pub kind: ChangeKind,
    pub old_size: u64,
    pub new_size: u64,
    pub is_dir: bool,
}

impl DeltaEntry {
    /// Betrag der Änderung in Bytes.
    pub fn abs_delta(&self) -> u64 {
        self.old_size.abs_diff(self.new_size)
    }

    /// Vorzeichenbehaftete Änderung (neu - alt).
    pub fn delta(&self) -> i64 {
        (self.new_size as i128 - self.old_size as i128).clamp(i64::MIN as i128, i64::MAX as i128)
            as i64
    }
}

struct Ctx<'a> {
    old: &'a Tree,
    new: &'a Tree,
    min_change: u64,
    out: Vec<DeltaEntry>,
}

impl Ctx<'_> {
    /// Ganzen Teilbaum als hinzugefügt (from_new) bzw. entfernt melden.
    fn one_side(&mut self, id: NodeId, from_new: bool) {
        let tree = if from_new { self.new } else { self.old };
        let mut stack = vec![id];
        while let Some(x) = stack.pop() {
            let n = tree.node(x);
            if n.size >= self.min_change {
                let (kind, old_size, new_size) = if from_new {
                    (ChangeKind::Added, 0, n.size)
                } else {
                    (ChangeKind::Removed, n.size, 0)
                };
                self.out.push(DeltaEntry {
                    path: tree.full_path(x),
                    kind,
                    old_size,
                    new_size,
                    is_dir: n.is_dir(),
                });
            }
            stack.extend(tree.children(x));
        }
    }

    /// Knotenpaar mit gleichem Pfad vergleichen (rekursiv über einen Stack).
    fn pair(&mut self, o: NodeId, n: NodeId) {
        let mut stack = vec![(o, n)];
        while let Some((o, n)) = stack.pop() {
            let on = self.old.node(o);
            let nn = self.new.node(n);
            if on.is_dir() != nn.is_dir() {
                // Datei wurde zu Ordner (oder umgekehrt): entfernt + hinzugefügt.
                self.one_side(o, false);
                self.one_side(n, true);
                continue;
            }
            let d = on.size.abs_diff(nn.size);
            if d > 0 && d >= self.min_change {
                self.out.push(DeltaEntry {
                    path: self.new.full_path(n),
                    kind: if nn.size > on.size {
                        ChangeKind::Grown
                    } else {
                        ChangeKind::Shrunk
                    },
                    old_size: on.size,
                    new_size: nn.size,
                    is_dir: nn.is_dir(),
                });
            }
            if !nn.is_dir() {
                continue;
            }
            // Kinder über den kleingeschriebenen Namen zuordnen.
            let mut old_kids: HashMap<String, NodeId> = self
                .old
                .children(o)
                .map(|c| (self.old.node(c).name.to_lowercase(), c))
                .collect();
            let mut c = nn.first_child;
            while c != NO_NODE {
                let key = self.new.node(c).name.to_lowercase();
                match old_kids.remove(&key) {
                    Some(oc) => stack.push((oc, c)),
                    None => self.one_side(c, true),
                }
                c = self.new.node(c).next_sibling;
            }
            let mut rest: Vec<NodeId> = old_kids.into_values().collect();
            rest.sort_unstable();
            for oc in rest {
                self.one_side(oc, false);
            }
        }
    }
}

/// Nach |Δ| absteigend (bei Gleichstand nach Pfad).
///
/// Gemeldet wird jede Änderung mit `|Δ| >= min_change`; unveränderte Größen nie.
/// Neue und entfernte Einträge erscheinen samt ihrem Inhalt.
pub fn compare(old: &Tree, new: &Tree, min_change: u64) -> Vec<DeltaEntry> {
    let mut ctx = Ctx {
        old,
        new,
        min_change,
        out: Vec::new(),
    };
    match (old.nodes.is_empty(), new.nodes.is_empty()) {
        (true, true) => {}
        (true, false) => ctx.one_side(new.root(), true),
        (false, true) => ctx.one_side(old.root(), false),
        (false, false) => ctx.pair(old.root(), new.root()),
    }
    let mut out = ctx.out;
    out.sort_by(|a, b| {
        b.abs_delta()
            .cmp(&a.abs_delta())
            .then_with(|| a.path.cmp(&b.path))
    });
    out
}
