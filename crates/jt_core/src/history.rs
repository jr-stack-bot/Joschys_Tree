// Joschys_Tree – Verlaufs-/Wachstumsanalyse über mehrere Snapshots – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::model::{FileTime, Tree};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HistoryPoint {
    pub time: FileTime,
    pub size: u64,
    pub allocated: u64,
    pub files: u64,
}

/// Nach Zeit sortiert.
pub fn series_for_path(snapshots: &[Tree], path: &str) -> Vec<HistoryPoint> {
    let _ = (snapshots, path);
    todo!("Auftrag C: series_for_path")
}

pub fn top_growers(snapshots: &[Tree], depth: u32, n: usize) -> Vec<(String, i64)> {
    let _ = (snapshots, depth, n);
    todo!("Auftrag C: top_growers")
}
