// Joschys_Tree – Tests paralleler Walk: Größen, Zähler, Symlinks, Abbruch – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use jt_core::model::{NodeFlags, ScanMode};
use jt_core::scan::{ScanError, ScanRequest, scan};

fn req(root: &Path) -> ScanRequest {
    ScanRequest {
        root: root.to_string_lossy().into_owned(),
        prefer_mft: true, // auf Linux nie MFT -> Walk
        follow_reparse: false,
    }
}

fn write(p: &Path, n: usize) {
    fs::write(p, vec![b'z'; n]).unwrap();
}

/// 20 Dateien in 3 Ebenen (a, a/b, a/b/c) plus Wurzel, ein Symlink auf `a`.
#[test]
fn scan_walk_sizes_counts_and_symlink_not_followed() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let a = root.join("a");
    let b = a.join("b");
    let c = b.join("c");
    fs::create_dir_all(&c).unwrap();
    let mut expected = 0u64;
    let mut size_a = 0u64;
    for i in 0..20usize {
        let dir = match i % 4 {
            0 => root.to_path_buf(),
            1 => a.clone(),
            2 => b.clone(),
            _ => c.clone(),
        };
        let n = 100 + i * 37;
        write(&dir.join(format!("datei{i:02}.bin")), n);
        expected += n as u64;
        if i % 4 != 0 {
            size_a += n as u64;
        }
    }
    #[cfg(unix)]
    std::os::unix::fs::symlink(&a, root.join("link_auf_a")).unwrap();

    let cancel = AtomicBool::new(false);
    let calls = std::sync::atomic::AtomicU32::new(0);
    let t = scan(
        &req(root),
        &|_p| {
            calls.fetch_add(1, Ordering::Relaxed);
        },
        &cancel,
    )
    .expect("Walk");

    assert!(calls.load(Ordering::Relaxed) >= 2, "Fortschritt gemeldet");
    assert_eq!(t.meta.mode, ScanMode::Walk);
    assert_eq!(t.meta.errors, 0);
    let r = t.node(0);
    assert_eq!(r.size, expected, "Symlink-Ziel darf nicht doppelt zählen");
    assert_eq!(r.file_count, 20);
    let root_s = root.to_string_lossy().into_owned();
    let na = t.find_path(&format!("{root_s}/a")).expect("a");
    assert_eq!(t.node(na).size, size_a);
    assert_eq!(t.node(na).file_count, 15);
    assert_eq!(t.node(na).dir_count, 2);
    let nc = t.find_path(&format!("{root_s}/a/b/c")).expect("c");
    assert_eq!(t.node(nc).file_count, 5);
    let f = t
        .find_path(&format!("{root_s}/a/b/c/datei03.bin"))
        .expect("datei03");
    assert_eq!(t.node(f).size, 100 + 3 * 37);
    assert_eq!(t.ext_of(f), "bin");

    #[cfg(unix)]
    {
        let l = t
            .find_path(&format!("{root_s}/link_auf_a"))
            .expect("Symlink als Eintrag vorhanden");
        let ln = t.node(l);
        assert!(ln.flags.contains(NodeFlags::REPARSE));
        assert_eq!(t.children(l).count(), 0, "Symlink wird nicht verfolgt");
        assert_eq!(ln.size, 0);
        // a, b, c + Link (als Ordner-Link angezeigt)
        assert_eq!(r.dir_count, 4);
    }
}

#[test]
fn scan_walk_unreadable_dir_counts_error_and_marks_access_denied() {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let zu = root.join("gesperrt");
        fs::create_dir(&zu).unwrap();
        write(&zu.join("x.txt"), 10);
        write(&root.join("ok.txt"), 5);
        fs::set_permissions(&zu, fs::Permissions::from_mode(0o000)).unwrap();
        let readable_anyway = fs::read_dir(&zu).is_ok(); // root ignoriert Rechte
        let cancel = AtomicBool::new(false);
        let t = scan(&req(root), &|_| {}, &cancel).expect("Walk bricht nie ab");
        fs::set_permissions(&zu, fs::Permissions::from_mode(0o755)).unwrap();
        let root_s = root.to_string_lossy().into_owned();
        let id = t.find_path(&format!("{root_s}/gesperrt")).unwrap();
        if readable_anyway {
            assert_eq!(t.meta.errors, 0);
        } else {
            assert!(t.meta.errors >= 1);
            assert!(t.node(id).flags.contains(NodeFlags::ACCESS_DENIED));
            assert_eq!(t.node(0).size, 5);
        }
    }
}

#[test]
fn scan_walk_missing_root_is_not_found() {
    let cancel = AtomicBool::new(false);
    let r = scan(&req(Path::new("/gibt/es/nicht/joschy")), &|_| {}, &cancel);
    assert!(matches!(r, Err(ScanError::NotFound(_))));
}

fn big_tree(root: &Path, dirs: usize, files: usize) {
    for d in 0..dirs {
        let p = root.join(format!("d{d:03}")).join("x").join("y");
        fs::create_dir_all(&p).unwrap();
        for f in 0..files {
            write(&p.join(format!("f{f}")), 1);
        }
    }
}

/// (5) Abbruch aus dem Fortschritts-Callback: sofortiges Ende mit `Cancelled`.
#[test]
fn scan_walk_cancel_from_progress_under_one_second() {
    let tmp = tempfile::tempdir().unwrap();
    big_tree(tmp.path(), 200, 10);
    let cancel = AtomicBool::new(false);
    let t0 = Instant::now();
    let r = scan(
        &req(tmp.path()),
        &|_| cancel.store(true, Ordering::Relaxed),
        &cancel,
    );
    assert!(matches!(r, Err(ScanError::Cancelled)));
    assert!(t0.elapsed() < Duration::from_secs(1), "{:?}", t0.elapsed());
}

/// (5) Abbruch von einem anderen Thread während des Walks: Ende in unter 1 s.
#[test]
fn scan_walk_cancel_from_other_thread_under_one_second() {
    let tmp = tempfile::tempdir().unwrap();
    big_tree(tmp.path(), 300, 10);
    let cancel = AtomicBool::new(false);
    let set_at = std::sync::Mutex::new(None::<Instant>);
    let r = std::thread::scope(|s| {
        s.spawn(|| {
            std::thread::sleep(Duration::from_millis(5));
            *set_at.lock().unwrap() = Some(Instant::now());
            cancel.store(true, Ordering::SeqCst);
        });
        scan(&req(tmp.path()), &|_| {}, &cancel)
    });
    let done = Instant::now();
    match r {
        Err(ScanError::Cancelled) => {
            let set = set_at.lock().unwrap().expect("gesetzt");
            assert!(done - set < Duration::from_secs(1));
        }
        // Sehr schnelle Maschine: Walk war schon vor dem Abbruch fertig.
        Ok(t) => assert_eq!(t.node(0).file_count, 3000),
        Err(e) => panic!("unerwartet: {e}"),
    }
}
