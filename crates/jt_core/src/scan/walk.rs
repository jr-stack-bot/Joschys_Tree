// Joschys_Tree – Paralleler Verzeichnis-Scan (std::fs + rayon) – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::sync::atomic::AtomicBool;

use super::{ScanError, ScanProgress, ScanRequest};
use crate::model::Tree;

pub(crate) fn scan_walk(
    req: &ScanRequest,
    progress: &dyn Fn(&ScanProgress),
    cancel: &AtomicBool,
) -> Result<Tree, ScanError> {
    let _ = (req, progress, cancel);
    todo!("Auftrag B: scan_walk")
}
