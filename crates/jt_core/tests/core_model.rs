// Joschys_Tree – Tests: Datenmodell und Aggregation – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

use jt_core::aggregate::{SortKey, percent_of_parent, recompute, sorted_children};
use jt_core::model::testutil::sample_tree;
use jt_core::{EntryInfo, ScanMeta, ScanMode, TreeBuilder};

#[test]
fn aggregation_three_levels() {
    let mut t = sample_tree();
    // Ordnerwerte verfälschen, recompute muss sie wiederherstellen.
    for n in t.nodes.iter_mut().filter(|n| n.is_dir()) {
        n.size = 999;
        n.allocated = 1;
        n.file_count = 77;
        n.dir_count = 77;
    }
    recompute(&mut t);
    let r = t.node(0);
    assert_eq!(
        (r.size, r.allocated, r.file_count, r.dir_count),
        (63_600, 77_824, 5, 3)
    );
    let users = t.find_path("C:\\Users").unwrap();
    let u = t.node(users);
    assert_eq!(
        (u.size, u.allocated, u.file_count, u.dir_count),
        (3_600, 12_288, 3, 1)
    );
    let joschy = t.find_path("C:\\Users\\joschy").unwrap();
    let j = t.node(joschy);
    assert_eq!(
        (j.size, j.allocated, j.file_count, j.dir_count),
        (3_100, 8_192, 2, 0)
    );
    let win = t.find_path("C:\\Windows").unwrap();
    assert_eq!(t.node(win).size, 10_000);
}

#[test]
fn full_path_and_find_path_case_insensitive() {
    let t = sample_tree();
    let id = t.find_path("c:\\users\\JOSCHY\\NOTIZ.TXT").unwrap();
    assert_eq!(t.full_path(id), "C:\\Users\\joschy\\notiz.txt");
    assert_eq!(t.find_path("C:/Users/joschy/notiz.txt"), Some(id));
    assert_eq!(t.full_path(0), "C:\\");
    assert_eq!(t.find_path("C:\\Users\\nix"), None);
    assert_eq!(t.find_path("D:\\Users"), None);
    // Roundtrip für alle Knoten
    for i in 0..t.nodes.len() as u32 {
        assert_eq!(t.find_path(&t.full_path(i)), Some(i));
    }
}

#[test]
fn percent_of_parent_values() {
    let t = sample_tree();
    assert_eq!(percent_of_parent(&t, 0), 100.0);
    let page = t.find_path("C:\\pagefile.sys").unwrap();
    let p = percent_of_parent(&t, page);
    assert!((p - 50_000.0 * 100.0 / 63_600.0).abs() < 1e-3, "{p}");
    let jpg = t.find_path("C:\\Users\\joschy\\Bild.JPG").unwrap();
    let p = percent_of_parent(&t, jpg);
    assert!((p - 3_000.0 * 100.0 / 3_100.0).abs() < 1e-3, "{p}");
    // Elternordner mit Größe 0
    let mut b = TreeBuilder::new(ScanMeta::new("X:\\", ScanMode::Walk), "X:\\");
    let e = b.add_file(0, "leer.txt", EntryInfo::default());
    let t2 = b.finish();
    assert_eq!(percent_of_parent(&t2, e), 0.0);
}

#[test]
fn sorted_children_keys() {
    let t = sample_tree();
    let names =
        |v: Vec<u32>| -> Vec<String> { v.iter().map(|&i| t.node(i).name.to_string()).collect() };
    assert_eq!(
        names(sorted_children(&t, 0, SortKey::Size, true)),
        ["pagefile.sys", "Windows", "Users"]
    );
    assert_eq!(
        names(sorted_children(&t, 0, SortKey::Size, false)),
        ["Users", "Windows", "pagefile.sys"]
    );
    assert_eq!(
        names(sorted_children(&t, 0, SortKey::Name, false)),
        ["pagefile.sys", "Users", "Windows"]
    );
    assert_eq!(
        names(sorted_children(&t, 0, SortKey::Files, true)),
        ["Users", "Windows", "pagefile.sys"]
    );
    let users = t.find_path("C:\\Users").unwrap();
    assert_eq!(
        names(sorted_children(&t, users, SortKey::Modified, true)),
        ["readme.md", "joschy"]
    );
    assert_eq!(
        names(sorted_children(&t, users, SortKey::Allocated, true)),
        ["joschy", "readme.md"]
    );
}
