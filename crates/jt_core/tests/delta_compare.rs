// Joschys_Tree – Tests Delta-Vergleich und Verlauf – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

use jt_core::delta::{ChangeKind, compare};
use jt_core::history::{series_for_path, top_growers};
use jt_core::model::testutil::sample_tree;
use jt_core::{EntryInfo, FileTime, ScanMeta, ScanMode, Tree, TreeBuilder};

fn f(size: u64) -> EntryInfo {
    EntryInfo {
        size,
        allocated: size,
        ..Default::default()
    }
}

/// Alt:  C:\ { Users\joschy\{Bild.JPG 3000, notiz.txt 100}, Users\readme.md 500,
///            Windows\system.dll 10000, pagefile.sys 50000 }  (= sample_tree)
/// Neu:  Bild.jpg (andere Schreibweise) 3000 -> 8000, notiz.txt entfernt,
///       system.dll 10000 -> 4000, neu: Users\joschy\video.mp4 20000.
fn new_tree() -> Tree {
    let mut b = TreeBuilder::new(ScanMeta::new("C:\\", ScanMode::Walk), "C:\\");
    let users = b.add_dir(0, "USERS", EntryInfo::default());
    let joschy = b.add_dir(users, "joschy", EntryInfo::default());
    b.add_file(joschy, "Bild.jpg", f(8000));
    b.add_file(joschy, "video.mp4", f(20000));
    b.add_file(users, "readme.md", f(500));
    let win = b.add_dir(0, "Windows", EntryInfo::default());
    b.add_file(win, "system.dll", f(4000));
    b.add_file(0, "pagefile.sys", f(50000));
    b.finish()
}

#[test]
fn delta_compare_kinds_and_order() {
    let old = sample_tree();
    let new = new_tree();
    let d = compare(&old, &new, 0);
    let find = |p: &str| {
        d.iter()
            .find(|e| e.path.eq_ignore_ascii_case(p))
            .unwrap_or_else(|| panic!("{p} fehlt in {d:#?}"))
    };
    let v = find("C:\\USERS\\joschy\\video.mp4");
    assert_eq!(
        (v.kind, v.old_size, v.new_size, v.is_dir),
        (ChangeKind::Added, 0, 20000, false)
    );
    let n = find("C:\\Users\\joschy\\notiz.txt");
    assert_eq!(
        (n.kind, n.old_size, n.new_size),
        (ChangeKind::Removed, 100, 0)
    );
    let b = find("C:\\Users\\joschy\\Bild.jpg");
    assert_eq!(
        (b.kind, b.old_size, b.new_size),
        (ChangeKind::Grown, 3000, 8000)
    );
    let s = find("C:\\Windows\\system.dll");
    assert_eq!(
        (s.kind, s.old_size, s.new_size),
        (ChangeKind::Shrunk, 10000, 4000)
    );
    // Ordner werden auch gemeldet
    let j = find("C:\\Users\\joschy");
    assert!(j.is_dir);
    assert_eq!(j.kind, ChangeKind::Grown);
    assert_eq!(j.new_size - j.old_size, 5000 + 20000 - 100);
    let w = find("C:\\Windows");
    assert!(w.is_dir && w.kind == ChangeKind::Shrunk);
    // unverändert -> nicht gemeldet
    assert!(d.iter().all(|e| !e.path.ends_with("pagefile.sys")));
    assert!(d.iter().all(|e| !e.path.ends_with("readme.md")));
    // sortiert nach |Δ| absteigend
    assert!(
        d.windows(2).all(|w| w[0].abs_delta() >= w[1].abs_delta()),
        "{d:#?}"
    );
    // Users und joschy wachsen am stärksten (je +24900), dann video.mp4 (+20000).
    let top: Vec<&str> = d.iter().take(3).map(|e| e.path.as_str()).collect();
    assert_eq!(
        top,
        vec![
            "C:\\USERS",
            "C:\\USERS\\joschy",
            "C:\\USERS\\joschy\\video.mp4"
        ]
    );
}

#[test]
fn delta_min_change_threshold() {
    let old = sample_tree();
    let new = new_tree();
    let d = compare(&old, &new, 5000);
    assert!(d.iter().all(|e| e.abs_delta() >= 5000));
    assert!(d.iter().any(|e| e.path.ends_with("Bild.jpg"))); // genau 5000
    assert!(d.iter().all(|e| !e.path.ends_with("notiz.txt"))); // 100
    assert!(compare(&old, &old, 0).is_empty());
}

fn snap(unix: i64, bild: u64) -> Tree {
    let mut b = TreeBuilder::new(
        ScanMeta {
            started: FileTime::from_unix_secs(unix),
            ..ScanMeta::new("C:\\", ScanMode::Walk)
        },
        "C:\\",
    );
    let users = b.add_dir(0, "Users", EntryInfo::default());
    if bild > 0 {
        b.add_file(users, "Bild.jpg", f(bild));
    }
    b.add_file(users, "fest.txt", f(10));
    let tmp = b.add_dir(0, "Temp", EntryInfo::default());
    b.add_file(tmp, "x.tmp", f(unix as u64 / 1000));
    b.finish()
}

#[test]
fn history_series_three_snapshots_sorted() {
    // absichtlich unsortiert übergeben
    let snaps = vec![
        snap(3_000_000, 300),
        snap(1_000_000, 100),
        snap(2_000_000, 0),
    ];
    let s = series_for_path(&snaps, "c:\\users\\BILD.JPG");
    assert_eq!(s.len(), 3);
    assert!(s.windows(2).all(|w| w[0].time < w[1].time));
    assert_eq!(
        s.iter().map(|p| p.size).collect::<Vec<_>>(),
        vec![100, 0, 300]
    );
    assert_eq!(s[1].files, 0);
    assert_eq!(s[0].files, 1);
    let u = series_for_path(&snaps, "C:\\Users");
    assert_eq!(u.iter().map(|p| p.files).collect::<Vec<_>>(), vec![2, 1, 2]);
    let missing = series_for_path(&snaps, "C:\\gibtsnicht");
    assert_eq!(missing.len(), 3);
    assert!(missing.iter().all(|p| p.size == 0 && p.allocated == 0));
}

#[test]
fn history_top_growers() {
    let snaps = vec![
        snap(3_000_000, 300),
        snap(1_000_000, 100),
        snap(2_000_000, 0),
    ];
    let g = top_growers(&snaps, 1, 10);
    // Temp: 1000 -> 3000 (+2000), Users: 110 -> 310 (+200)
    assert_eq!(
        g,
        vec![
            ("C:\\Temp".to_string(), 2000),
            ("C:\\Users".to_string(), 200)
        ]
    );
    assert_eq!(top_growers(&snaps, 1, 1).len(), 1);
    assert!(top_growers(&snaps[..1], 1, 10).is_empty());
}
