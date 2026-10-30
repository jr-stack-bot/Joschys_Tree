// Joschys_Tree – Branding-Test: jede .rs-Datei enthält @ Joschy in den ersten 3 Zeilen – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fs;
use std::path::{Path, PathBuf};

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let name = e.file_name();
        if p.is_dir() {
            if name != "target" && name != ".git" {
                collect(&p, out);
            }
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

#[test]
fn every_rs_file_has_branding_header() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut files = Vec::new();
    collect(&root, &mut files);
    assert!(!files.is_empty());
    let mut bad = Vec::new();
    for f in &files {
        let text = fs::read_to_string(f).unwrap_or_default();
        let head: Vec<&str> = text.lines().take(3).collect();
        if !head.iter().any(|l| l.contains("@ Joschy"))
            || !head
                .iter()
                .any(|l| l.contains("SPDX-License-Identifier: AGPL-3.0-or-later"))
        {
            bad.push(f.display().to_string());
        }
    }
    assert!(bad.is_empty(), "Ohne Branding-Kopf: {bad:?}");
}
