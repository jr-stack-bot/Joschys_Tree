// Joschys_Tree – MFT-Scan unter Windows (Volume öffnen, $MFT lesen) – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::sync::atomic::AtomicBool;

use super::{ScanError, ScanProgress, ScanRequest};
use crate::model::Tree;

pub(crate) fn scan_mft(
    req: &ScanRequest,
    progress: &dyn Fn(&ScanProgress),
    cancel: &AtomicBool,
) -> Result<Tree, ScanError> {
    let _ = (req, progress, cancel);
    Err(ScanError::Unsupported(String::from("MFT-Scan noch nicht implementiert")))
}
