// Joschys_Tree – Tests: Snapshot-Roundtrip und Formatierung – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

use jt_core::Lang;
use jt_core::format;
use jt_core::model::testutil::sample_tree;
use jt_core::snapshot::{self, SnapError};

#[test]
fn snapshot_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("a.jtscan");
    let t = sample_tree();
    snapshot::save(&t, &p).unwrap();
    let raw = std::fs::read(&p).unwrap();
    assert_eq!(&raw[..8], b"JTSCAN01");
    let back = snapshot::load(&p).unwrap();
    assert_eq!(back, t);
}

#[test]
fn snapshot_bad_magic_and_version() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("b.jtscan");
    std::fs::write(&p, b"NOTASCAN\x01\x00rest").unwrap();
    assert!(matches!(snapshot::load(&p), Err(SnapError::BadMagic)));
    std::fs::write(&p, b"JT").unwrap();
    assert!(matches!(snapshot::load(&p), Err(SnapError::BadMagic)));
    let mut good = snapshot::to_bytes(&sample_tree()).unwrap();
    good[8] = 9;
    assert!(matches!(
        snapshot::from_bytes(&good),
        Err(SnapError::UnsupportedVersion(9))
    ));
    let mut cut = snapshot::to_bytes(&sample_tree()).unwrap();
    cut.truncate(cut.len() / 2);
    assert!(matches!(
        snapshot::from_bytes(&cut),
        Err(SnapError::Decode(_))
    ));
    assert!(matches!(
        snapshot::load(&dir.path().join("fehlt.jtscan")),
        Err(SnapError::Io(_))
    ));
}

#[test]
fn format_bytes() {
    assert_eq!(format::bytes(1536, Lang::De), "1,5 KiB");
    assert_eq!(format::bytes(1536, Lang::En), "1.5 KiB");
    assert_eq!(format::bytes(0, Lang::De), "0 B");
    assert_eq!(format::bytes(1023, Lang::De), "1023 B");
    assert_eq!(format::bytes(1024, Lang::De), "1 KiB");
    assert_eq!(format::bytes(1_320_702_444, Lang::De), "1,23 GiB");
    assert_eq!(format::bytes(1024 * 1024 - 1, Lang::En), "1 MiB");
    assert_eq!(format::bytes(u64::MAX, Lang::En), "16 EiB");
}

#[test]
fn format_number_and_time() {
    assert_eq!(format::number(1_234_567, Lang::De), "1.234.567");
    assert_eq!(format::number(1_234_567, Lang::En), "1,234,567");
    assert_eq!(format::number(999, Lang::De), "999");
    assert_eq!(format::number(0, Lang::En), "0");
    assert_eq!(format::time(jt_core::FileTime(0), Lang::De), "");
    // Erwartung unabhängig von der lokalen Zeitzone der Testmaschine berechnen.
    let t = sample_tree().meta.started;
    let local = chrono::DateTime::from_timestamp(t.to_unix_secs(), 0)
        .unwrap()
        .with_timezone(&chrono::Local);
    assert_eq!(
        format::time(t, Lang::De),
        local.format("%d.%m.%Y %H:%M").to_string()
    );
    assert_eq!(
        format::time(t, Lang::En),
        local.format("%Y-%m-%d %H:%M").to_string()
    );
    assert_eq!(format::time(t, Lang::De).len(), 16);
}
