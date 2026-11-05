// Joschys_Tree – Auswahlkorb: gesammelte Elemente ohne Doppelzählung – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later
//
// Reine Logik. Invariante: Kein Element im Korb ist Nachkomme eines anderen
// Elements im Korb. Dadurch ist die Summe der belegten Größen ohne Doppelzählung.

use std::collections::HashSet;

use jt_core::{NO_NODE, NodeId, Tree};

#[derive(Default, Debug)]
pub struct Basket {
    items: Vec<NodeId>,
    set: HashSet<NodeId>,
    /// Baum-Generation und Live-Flag, für die der Korb gilt.
    gen_: u64,
    live: bool,
}

impl Basket {
    /// Leeren und an einen neuen Baum binden.
    pub fn reset(&mut self, gen_: u64, live: bool) {
        self.items.clear();
        self.set.clear();
        self.gen_ = gen_;
        self.live = live;
    }

    pub fn gen_(&self) -> u64 {
        self.gen_
    }

    pub fn live(&self) -> bool {
        self.live
    }

    /// Nach dem Entfernen gelöschter Knoten: neue Generation übernehmen.
    pub fn rebind(&mut self, gen_: u64) {
        self.gen_ = gen_;
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn items(&self) -> &[NodeId] {
        &self.items
    }

    pub fn contains(&self, id: NodeId) -> bool {
        self.set.contains(&id)
    }

    /// Liegt ein echter Vorfahr von `id` schon im Korb?
    pub fn covered_by_ancestor(&self, tree: &Tree, id: NodeId) -> bool {
        has_ancestor_in(tree, id, &self.set)
    }

    #[cfg(test)]
    /// Ein Element hinzufügen. Enthaltene Einzelelemente eines Ordners fliegen raus.
    /// `false`, wenn es schon (direkt oder über einen Ordner) im Korb liegt.
    pub fn add(&mut self, tree: &Tree, id: NodeId) -> bool {
        self.add_many(tree, &[id]) == 1
    }

    /// Mehrere Elemente hinzufügen; liefert die Zahl der neu aufgenommenen.
    /// Laufzeit O((alt + neu) * Tiefe).
    pub fn add_many(&mut self, tree: &Tree, ids: &[NodeId]) -> usize {
        let before: HashSet<NodeId> = self.set.clone();
        let mut all = self.set.clone();
        let mut order = self.items.clone();
        for &id in ids {
            if (id as usize) < tree.nodes.len() && all.insert(id) {
                order.push(id);
            }
        }
        let keep: Vec<NodeId> = order
            .into_iter()
            .filter(|&id| !has_ancestor_in(tree, id, &all))
            .collect();
        self.set = keep.iter().copied().collect();
        self.items = keep;
        self.items.iter().filter(|id| !before.contains(id)).count()
    }

    pub fn remove(&mut self, id: NodeId) -> bool {
        if self.set.remove(&id) {
            self.items.retain(|&x| x != id);
            true
        } else {
            false
        }
    }

    pub fn retain(&mut self, mut f: impl FnMut(NodeId) -> bool) {
        self.items.retain(|&id| f(id));
        self.set = self.items.iter().copied().collect();
    }

    /// Belegter Speicher aller Elemente (keine Doppelzählung dank Invariante).
    pub fn total_allocated(&self, tree: &Tree) -> u64 {
        self.items
            .iter()
            .filter(|&&id| (id as usize) < tree.nodes.len())
            .map(|&id| tree.node(id).allocated)
            .sum()
    }
}

fn has_ancestor_in(tree: &Tree, id: NodeId, set: &HashSet<NodeId>) -> bool {
    if (id as usize) >= tree.nodes.len() {
        return false;
    }
    let mut p = tree.node(id).parent;
    while p != NO_NODE {
        if set.contains(&p) {
            return true;
        }
        p = tree.node(p).parent;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use jt_core::model::testutil::sample_tree;

    #[test]
    fn folder_absorbs_children_and_no_double_count() {
        let t = sample_tree();
        let users = t.find_path("C:\\Users").unwrap();
        let joschy = t.find_path("C:\\Users\\joschy").unwrap();
        let jpg = t.find_path("C:\\Users\\joschy\\Bild.JPG").unwrap();
        let readme = t.find_path("C:\\Users\\readme.md").unwrap();
        let page = t.find_path("C:\\pagefile.sys").unwrap();
        let mut b = Basket::default();
        b.reset(1, true);
        assert!(b.add(&t, jpg));
        assert!(b.add(&t, readme));
        assert!(!b.add(&t, jpg), "nicht doppelt");
        assert_eq!(b.len(), 2);
        // Ordner nimmt enthaltene Einzelelemente auf.
        assert!(b.add(&t, users));
        assert_eq!(b.items(), &[users]);
        assert!(b.covered_by_ancestor(&t, joschy));
        assert!(!b.add(&t, joschy), "über Ordner schon enthalten");
        assert_eq!(b.total_allocated(&t), t.node(users).allocated);
        assert!(b.add(&t, page));
        assert_eq!(
            b.total_allocated(&t),
            t.node(users).allocated + t.node(page).allocated
        );
        assert!(b.remove(users));
        assert!(!b.remove(users));
        assert_eq!(b.items(), &[page]);
    }

    #[test]
    fn add_many_keeps_topmost_and_counts_new() {
        let t = sample_tree();
        let joschy = t.find_path("C:\\Users\\joschy").unwrap();
        let jpg = t.find_path("C:\\Users\\joschy\\Bild.JPG").unwrap();
        let txt = t.find_path("C:\\Users\\joschy\\notiz.txt").unwrap();
        let mut b = Basket::default();
        b.reset(1, true);
        assert_eq!(b.add_many(&t, &[jpg, txt, joschy, jpg, 9999]), 1);
        assert_eq!(b.items(), &[joschy]);
        b.reset(2, false);
        assert!(b.is_empty());
        assert_eq!(b.gen_(), 2);
        assert!(!b.live());
    }

    #[test]
    fn retain_after_removal() {
        let mut t = sample_tree();
        let joschy = t.find_path("C:\\Users\\joschy").unwrap();
        let page = t.find_path("C:\\pagefile.sys").unwrap();
        let mut b = Basket::default();
        b.reset(1, true);
        b.add_many(&t, &[joschy, page]);
        crate::app::unlink_node(&mut t, joschy);
        b.retain(|id| crate::app::is_attached(&t, id));
        assert_eq!(b.items(), &[page]);
        assert!(!b.contains(joschy));
    }
}
