// Joschys_Tree – Delta-Vergleich zweier Trees – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::model::Tree;

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

/// Nach |Δ| absteigend.
pub fn compare(old: &Tree, new: &Tree, min_change: u64) -> Vec<DeltaEntry> {
    let _ = (old, new, min_change);
    todo!("Auftrag C: compare")
}
