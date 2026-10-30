// Joschys_Tree – Tests: Top-Dateien, Dateitypen, Alters-Histogramm – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

use jt_core::filter::Filter;
use jt_core::model::testutil::sample_tree;
use jt_core::stats::{age_histogram, file_types, top_files};
use jt_core::{EntryInfo, FileTime, ScanMeta, ScanMode, TreeBuilder};

#[test]
fn top_files_sorted_desc_files_only() {
    let t = sample_tree();
    let top = top_files(&t, 100, None);
    assert_eq!(top.len(), 5);
    assert!(top.iter().all(|&id| !t.node(id).is_dir()));
    let sizes: Vec<u64> = top.iter().map(|&id| t.node(id).size).collect();
    assert_eq!(sizes, [50_000, 10_000, 3_000, 500, 100]);
    assert_eq!(top_files(&t, 2, None).len(), 2);
    assert_eq!(top_files(&t, 2, None), top[..2]);
    assert!(top_files(&t, 0, None).is_empty());
}

#[test]
fn top_files_bounded_many() {
    let mut b = TreeBuilder::new(ScanMeta::new("X:\\", ScanMode::Walk), "X:\\");
    let d = b.add_dir(0, "d", EntryInfo::default());
    for i in 0..10_000u64 {
        let size = (i * 7919) % 10_007;
        b.add_file(
            d,
            &format!("f{i}.bin"),
            EntryInfo {
                size,
                ..Default::default()
            },
        );
    }
    let t = b.finish();
    let top = top_files(&t, 100, None);
    assert_eq!(top.len(), 100);
    let mut all: Vec<u64> = t
        .nodes
        .iter()
        .filter(|n| !n.is_dir())
        .map(|n| n.size)
        .collect();
    all.sort_unstable_by(|a, b| b.cmp(a));
    let got: Vec<u64> = top.iter().map(|&id| t.node(id).size).collect();
    assert_eq!(got, all[..100]);
}

#[test]
fn top_files_with_filter() {
    let t = sample_tree();
    let f = Filter {
        extensions: vec!["txt".into(), "MD".into()],
        include_hidden: true,
        ..Default::default()
    };
    let top = top_files(&t, 100, Some(&f));
    let names: Vec<&str> = top.iter().map(|&id| &*t.node(id).name).collect();
    assert_eq!(names, ["readme.md", "notiz.txt"]);
}

#[test]
fn file_types_by_size() {
    let t = sample_tree();
    let ft = file_types(&t, 0);
    let exts: Vec<&str> = ft.iter().map(|s| s.ext.as_str()).collect();
    assert_eq!(exts, ["sys", "dll", "jpg", "md", "txt"]);
    assert_eq!(ft[0].files, 1);
    assert_eq!(ft[0].size, 50_000);
    assert_eq!(ft[0].allocated, 53_248);
    let users = t.find_path("C:\\Users").unwrap();
    let ft = file_types(&t, users);
    assert_eq!(ft.len(), 3);
    assert_eq!(ft.iter().map(|s| s.size).sum::<u64>(), 3_600);
}

#[test]
fn age_histogram_buckets() {
    let t = sample_tree();
    let now = t.meta.started; // 2026-10-30
    // pagefile 29 Tage -> <1M; Bild 302 Tage -> <1J; readme 516 und system.dll 1033 Tage -> <3J;
    // notiz 2020 -> >=3J
    let h = age_histogram(&t, 0, now);
    assert_eq!(h, [50_000, 0, 0, 3_000, 10_500, 100]);
    assert_eq!(h.iter().sum::<u64>(), t.node(0).size);
    let users = t.find_path("C:\\Users").unwrap();
    assert_eq!(age_histogram(&t, users, now), [0, 0, 0, 3_000, 500, 100]);
    // Unbekannte Zeit zählt als >=3J
    let mut b = TreeBuilder::new(ScanMeta::new("X:\\", ScanMode::Walk), "X:\\");
    b.add_file(
        0,
        "a",
        EntryInfo {
            size: 5,
            ..Default::default()
        },
    );
    let t2 = b.finish();
    assert_eq!(
        age_histogram(&t2, 0, FileTime::from_unix_secs(1_800_000_000)),
        [0, 0, 0, 0, 0, 5]
    );
}
