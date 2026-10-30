// Joschys_Tree – Tests Duplikat-Suche – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashMap;
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use jt_core::duplicates::{find_duplicates, DupGroup, FileReader, StdFileReader};
use jt_core::{EntryInfo, NodeId, ScanMeta, ScanMode, Tree, TreeBuilder};

fn no_progress(_: u64, _: u64) {}

/// Baum aus echten Dateien in einem tempdir (Wurzelname = tempdir-Pfad).
fn real_tree(dir: &std::path::Path, files: &[(&str, Vec<u8>)]) -> (Tree, HashMap<String, NodeId>) {
    let root = dir.to_string_lossy().to_string();
    let mut b = TreeBuilder::new(ScanMeta::new(root.clone(), ScanMode::Walk), &root);
    let mut ids = HashMap::new();
    for (name, data) in files {
        std::fs::write(dir.join(name), data).unwrap();
        let id = b.add_file(
            0,
            name,
            EntryInfo {
                size: data.len() as u64,
                allocated: data.len() as u64,
                ..Default::default()
            },
        );
        ids.insert(name.to_string(), id);
    }
    (b.finish(), ids)
}

#[test]
fn dup_three_equal_two_same_size_different() {
    let dir = tempfile::tempdir().unwrap();
    // 10 KiB: größer als der Teil-Hash, damit Stufe 3 läuft.
    let same: Vec<u8> = (0..10_240u32).map(|i| (i % 251) as u8).collect();
    let mut diff1 = same.clone();
    *diff1.last_mut().unwrap() ^= 0xFF; // gleicher Kopf, anderes Ende
    let mut diff2 = same.clone();
    diff2[0] ^= 0xFF; // anderer Kopf
    let (tree, ids) = real_tree(
        dir.path(),
        &[
            ("a.bin", same.clone()),
            ("b.bin", same.clone()),
            ("c.bin", same.clone()),
            ("d.bin", diff1),
            ("e.bin", diff2),
            ("klein.txt", b"hallo".to_vec()),
            ("klein2.txt", b"hallo".to_vec()),
        ],
    );
    let cancel = AtomicBool::new(false);
    let groups = find_duplicates(&tree, 0, 1024, &StdFileReader, &no_progress, &cancel);
    assert_eq!(groups.len(), 1, "{groups:?}");
    let g = &groups[0];
    assert_eq!(g.size, 10_240);
    let mut expect = vec![ids["a.bin"], ids["b.bin"], ids["c.bin"]];
    expect.sort();
    assert_eq!(g.files, expect);
    assert_eq!(g.savings(), 2 * 10_240);
}

#[test]
fn dup_min_size_ignores_small_files() {
    let dir = tempfile::tempdir().unwrap();
    let (tree, _) = real_tree(
        dir.path(),
        &[
            ("x1", vec![7u8; 100]),
            ("x2", vec![7u8; 100]),
            ("y1", vec![9u8; 5000]),
            ("y2", vec![9u8; 5000]),
        ],
    );
    let cancel = AtomicBool::new(false);
    let all = find_duplicates(&tree, 0, 0, &StdFileReader, &no_progress, &cancel);
    assert_eq!(all.len(), 2);
    // nach Einsparpotenzial sortiert
    assert_eq!(all[0].size, 5000);
    let big = find_duplicates(&tree, 0, 101, &StdFileReader, &no_progress, &cancel);
    assert_eq!(big.len(), 1);
    assert_eq!(big[0].size, 5000);
}

/// Mock: Inhalt je Pfad im Speicher, zählt Aufrufe je Pfad.
struct MockReader {
    content: HashMap<String, Vec<u8>>,
    calls: Mutex<HashMap<String, u32>>,
    cancel_after: Option<(&'static AtomicBool, u32)>,
}

impl MockReader {
    fn new(content: HashMap<String, Vec<u8>>) -> Self {
        MockReader {
            content,
            calls: Mutex::new(HashMap::new()),
            cancel_after: None,
        }
    }
    fn hit(&self, path: &str) -> io::Result<&Vec<u8>> {
        let total = {
            let mut c = self.calls.lock().unwrap();
            *c.entry(path.to_string()).or_default() += 1;
            c.values().sum::<u32>()
        };
        if let Some((flag, n)) = self.cancel_after {
            if total >= n {
                flag.store(true, Ordering::SeqCst);
            }
        }
        self.content
            .get(path)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "fehlt"))
    }
    fn calls_for(&self, path: &str) -> u32 {
        self.calls.lock().unwrap().get(path).copied().unwrap_or(0)
    }
}

impl FileReader for MockReader {
    fn read_head(&self, path: &str, n: usize) -> io::Result<Vec<u8>> {
        let d = self.hit(path)?;
        Ok(d[..n.min(d.len())].to_vec())
    }
    fn hash_full(&self, path: &str, _cancel: &AtomicBool) -> io::Result<u128> {
        Ok(xxhash_rust_shim(self.hit(path)?))
    }
}

/// Einfache, deterministische Prüfsumme für den Mock (kein xxh3 im Test nötig).
fn xxhash_rust_shim(d: &[u8]) -> u128 {
    d.iter().fold(0xcbf29ce484222325u128, |h, &b| {
        (h ^ b as u128).wrapping_mul(0x100000001b3)
    })
}

fn mock_tree(sizes: &[(&str, u64)]) -> Tree {
    let mut b = TreeBuilder::new(ScanMeta::new("C:\\", ScanMode::Walk), "C:\\");
    let d = b.add_dir(0, "daten", EntryInfo::default());
    for (name, size) in sizes {
        b.add_file(
            d,
            name,
            EntryInfo {
                size: *size,
                ..Default::default()
            },
        );
    }
    b.finish()
}

#[test]
fn dup_unique_sizes_never_read() {
    let tree = mock_tree(&[
        ("einzig1.dat", 111),
        ("einzig2.dat", 9_999),
        ("gleich1.dat", 8_000),
        ("gleich2.dat", 8_000),
        ("kaputt.dat", 8_000), // Lesefehler: wird übersprungen
    ]);
    let mut content = HashMap::new();
    for name in ["einzig1.dat", "einzig2.dat", "gleich1.dat", "gleich2.dat"] {
        content.insert(format!("C:\\daten\\{name}"), vec![1u8; 8_000]);
    }
    let reader = MockReader::new(content);
    let cancel = AtomicBool::new(false);
    let last = Mutex::new((0u64, 0u64));
    let groups = find_duplicates(
        &tree,
        0,
        0,
        &reader,
        &|d, t| *last.lock().unwrap() = (d, t),
        &cancel,
    );
    assert_eq!(reader.calls_for("C:\\daten\\einzig1.dat"), 0);
    assert_eq!(reader.calls_for("C:\\daten\\einzig2.dat"), 0);
    assert_eq!(reader.calls_for("C:\\daten\\gleich1.dat"), 2); // Kopf + voll
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].files.len(), 2);
    let (d, t) = *last.lock().unwrap();
    assert_eq!(t, 3 * 8_000);
    assert_eq!(d, t);
}

#[test]
fn dup_cancel_returns_partial_without_panic() {
    // 50 Größen-Gruppen à 3 gleiche Dateien -> mehrere Runden.
    let mut sizes = Vec::new();
    let mut content = HashMap::new();
    let mut names = Vec::new();
    for g in 0..300u64 {
        for k in 0..3 {
            names.push((format!("f{g}_{k}.bin"), 100 + g));
        }
    }
    for (name, size) in &names {
        sizes.push((name.as_str(), *size));
        content.insert(
            format!("C:\\daten\\{name}"),
            vec![(*size % 256) as u8; *size as usize],
        );
    }
    let tree = mock_tree(&sizes);

    // Sofort abgebrochen: leeres Ergebnis, kein Lesen.
    let cancel = AtomicBool::new(true);
    let reader = MockReader::new(content.clone());
    let none = find_duplicates(&tree, 0, 0, &reader, &no_progress, &cancel);
    assert!(none.is_empty());
    assert_eq!(reader.calls.lock().unwrap().len(), 0);

    // Abbruch mitten drin: Teilergebnis, jede Gruppe vollständig korrekt.
    static CANCEL: AtomicBool = AtomicBool::new(false);
    let mut reader = MockReader::new(content);
    reader.cancel_after = Some((&CANCEL, 300));
    let part = find_duplicates(&tree, 0, 0, &reader, &no_progress, &CANCEL);
    assert!(CANCEL.load(Ordering::SeqCst));
    assert!(!part.is_empty(), "Teilergebnis erwartet");
    assert!(part.len() < 300, "nicht alles fertig: {}", part.len());
    for g in &part {
        assert!(g.files.len() >= 2);
        for &f in &g.files {
            assert_eq!(tree.node(f).size, g.size);
        }
    }
    // sortiert nach Einsparpotenzial
    assert!(part.windows(2).all(|w| w[0].savings() >= w[1].savings()));
    let _: &[DupGroup] = &part;
}

#[test]
fn dup_scope_limits_search() {
    let tree = jt_core::model::testutil::sample_tree();
    let cancel = AtomicBool::new(false);
    let reader = MockReader::new(HashMap::new());
    // sample_tree hat nur eindeutige Größen -> nichts gelesen, keine Gruppen.
    let g = find_duplicates(&tree, 0, 0, &reader, &no_progress, &cancel);
    assert!(g.is_empty());
    assert!(reader.calls.lock().unwrap().is_empty());
}
