// Joschys_Tree – Tests: Filter und Suche – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

use jt_core::filter::{AgeField, Filter, search};
use jt_core::model::testutil::sample_tree;
use jt_core::{FileTime, NodeId, Tree};

fn id(t: &Tree, p: &str) -> NodeId {
    t.find_path(p).unwrap()
}

fn now(t: &Tree) -> FileTime {
    t.meta.started // 2026-10-30
}

#[test]
fn filter_size() {
    let t = sample_tree();
    let f = Filter {
        min_size: Some(5_000),
        max_size: Some(20_000),
        files_only: true,
        include_hidden: true,
        ..Default::default()
    };
    assert!(f.matches(&t, id(&t, "C:\\Windows\\system.dll"), now(&t)));
    assert!(!f.matches(&t, id(&t, "C:\\pagefile.sys"), now(&t)));
    assert!(!f.matches(&t, id(&t, "C:\\Windows"), now(&t))); // Ordner, files_only
}

#[test]
fn filter_age() {
    let t = sample_tree();
    let older = Filter {
        older_than_days: Some(365),
        include_hidden: true,
        ..Default::default()
    };
    assert!(older.matches(&t, id(&t, "C:\\Windows\\system.dll"), now(&t)));
    assert!(!older.matches(&t, id(&t, "C:\\pagefile.sys"), now(&t)));
    let newer = Filter {
        newer_than_days: Some(60),
        age_field: AgeField::Created,
        include_hidden: true,
        ..Default::default()
    };
    assert!(newer.matches(&t, id(&t, "C:\\pagefile.sys"), now(&t)));
    assert!(!newer.matches(&t, id(&t, "C:\\Users\\readme.md"), now(&t)));
}

#[test]
fn filter_extension() {
    let t = sample_tree();
    let f = Filter {
        extensions: vec![".JPG".into()],
        ..Default::default()
    };
    assert!(f.matches(&t, id(&t, "C:\\Users\\joschy\\Bild.JPG"), now(&t)));
    assert!(!f.matches(&t, id(&t, "C:\\Users\\readme.md"), now(&t)));
    assert!(!f.matches(&t, id(&t, "C:\\Users"), now(&t)));
}

#[test]
fn filter_glob() {
    let t = sample_tree();
    let f = Filter {
        name_glob: Some("*.d?l".into()),
        include_hidden: true,
        ..Default::default()
    };
    assert!(f.matches(&t, id(&t, "C:\\Windows\\system.dll"), now(&t)));
    assert!(!f.matches(&t, id(&t, "C:\\pagefile.sys"), now(&t)));
    // ohne Platzhalter: Teilstring, case-insensitiv
    let sub = Filter {
        name_glob: Some("BILD".into()),
        ..Default::default()
    };
    assert!(sub.matches(&t, id(&t, "C:\\Users\\joschy\\Bild.JPG"), now(&t)));
    assert!(!sub.matches(&t, id(&t, "C:\\Users\\readme.md"), now(&t)));
}

#[test]
fn filter_hidden() {
    let t = sample_tree();
    let f = Filter::default();
    assert!(!f.matches(&t, id(&t, "C:\\Users\\joschy\\notiz.txt"), now(&t)));
    assert!(f.matches(&t, id(&t, "C:\\Users\\readme.md"), now(&t)));
}

#[test]
fn search_scope_and_limit() {
    let t = sample_tree();
    let f = Filter {
        files_only: true,
        include_hidden: true,
        ..Default::default()
    };
    assert_eq!(search(&t, 0, &f, now(&t), 0).len(), 5);
    assert_eq!(search(&t, 0, &f, now(&t), 2).len(), 2);
    let users = id(&t, "C:\\Users");
    let r = search(&t, users, &f, now(&t), 0);
    assert_eq!(r.len(), 3);
    assert!(r.iter().all(|&i| t.full_path(i).starts_with("C:\\Users\\")));
    let all = search(
        &t,
        0,
        &Filter {
            include_hidden: true,
            ..Default::default()
        },
        now(&t),
        0,
    );
    assert_eq!(all.len(), t.nodes.len() - 1);
}
