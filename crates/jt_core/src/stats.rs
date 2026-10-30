// Joschys_Tree – Top-Dateien, Dateitypen-Statistik, Alters-Histogramm – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::filter::Filter;
use crate::model::{FileTime, NodeId, Tree};

pub fn top_files(tree: &Tree, n: usize, filter: Option<&Filter>) -> Vec<NodeId> {
    let _ = (tree, n, filter);
    todo!("Auftrag A: top_files")
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypeStat {
    pub ext: String,
    pub files: u64,
    pub size: u64,
    pub allocated: u64,
}

/// Nach size absteigend.
pub fn file_types(tree: &Tree, scope: NodeId) -> Vec<TypeStat> {
    let _ = (tree, scope);
    todo!("Auftrag A: file_types")
}

/// Bytes je Altersklasse: <1M, <3M, <6M, <1J, <3J, >=3J.
pub fn age_histogram(tree: &Tree, scope: NodeId, now: FileTime) -> [u64; 6] {
    let _ = (tree, scope, now);
    todo!("Auftrag A: age_histogram")
}
