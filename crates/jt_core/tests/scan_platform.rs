// Joschys_Tree – Tests Plattform-Logik: lange Pfade, Fallback-Entscheidung, Hilfen – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

use jt_core::model::{NodeFlags, ScanMode};
use jt_core::platform::{
    drives_from_mask, format_access_mask, stream_display_name, strip_long_prefix, to_long_path,
};
use jt_core::scan::mft_parse::flags_from_win_attributes;
use jt_core::scan::{choose_mode, drive_root_letter};

#[test]
fn scan_long_path_conversion() {
    assert_eq!(to_long_path(r"C:\Users\joschy"), r"\\?\C:\Users\joschy");
    assert_eq!(to_long_path("c:/Users/joschy/"), r"\\?\C:\Users\joschy");
    assert_eq!(to_long_path("C:"), r"\\?\C:\");
    assert_eq!(to_long_path(r"C:\"), r"\\?\C:\");
    assert_eq!(to_long_path(r"C:\a\.\b\..\c"), r"\\?\C:\a\c");
    assert_eq!(
        to_long_path(r"\\server\share\ordner\x.txt"),
        r"\\?\UNC\server\share\ordner\x.txt"
    );
    assert_eq!(to_long_path(r"\\pc\c$"), r"\\?\UNC\pc\c$");
    assert_eq!(to_long_path(r"\\?\C:\schon"), r"\\?\C:\schon");
    assert_eq!(to_long_path(r"\\.\C:"), r"\\.\C:");
    assert_eq!(to_long_path(r"relativ\pfad"), r"relativ\pfad");
    assert_eq!(to_long_path("C:relativ"), "C:relativ");
    assert_eq!(to_long_path(r"\\nurserver"), r"\\nurserver");
    let long = format!(r"C:\{}", "x".repeat(300));
    assert!(to_long_path(&long).starts_with(r"\\?\C:\xxx"));
}

#[test]
fn scan_strip_prefix_roundtrip() {
    for p in [r"C:\a\b", r"\\server\share\x"] {
        assert_eq!(strip_long_prefix(&to_long_path(p)), p);
    }
}

#[test]
fn scan_fallback_decision() {
    assert_eq!(drive_root_letter(r"C:\"), Some('C'));
    assert_eq!(drive_root_letter("d:"), Some('D'));
    assert_eq!(drive_root_letter(r"\\?\E:\"), Some('E'));
    assert_eq!(drive_root_letter(r"C:\Users"), None);
    assert_eq!(drive_root_letter(r"\\server\share"), None);

    assert_eq!(choose_mode(true, true, r"C:\", Some("NTFS")), ScanMode::Mft);
    assert_eq!(
        choose_mode(false, true, r"C:\", Some("NTFS")),
        ScanMode::Walk
    );
    assert_eq!(
        choose_mode(true, false, r"C:\", Some("NTFS")),
        ScanMode::Walk
    );
    assert_eq!(
        choose_mode(true, true, r"C:\", Some("FAT32")),
        ScanMode::Walk
    );
    assert_eq!(
        choose_mode(true, true, r"E:\", Some("exFAT")),
        ScanMode::Walk
    );
    assert_eq!(choose_mode(true, true, r"C:\", None), ScanMode::Walk);
    assert_eq!(
        choose_mode(true, true, r"\\server\share", Some("NTFS")),
        ScanMode::Walk
    );
    assert_eq!(
        choose_mode(true, true, r"C:\Users", Some("NTFS")),
        ScanMode::Walk
    );
}

#[test]
fn scan_small_helpers() {
    assert_eq!(drives_from_mask(0b1101), vec!["A:\\", "C:\\", "D:\\"]);
    assert_eq!(stream_display_name("::$DATA"), None);
    assert_eq!(
        stream_display_name(":geheim:$DATA").as_deref(),
        Some("geheim")
    );
    assert_eq!(format_access_mask(0x001F_01FF), "FullControl");
    assert_eq!(format_access_mask(0x0013_01BF), "Modify");
    assert_eq!(format_access_mask(0x0012_00A9), "ReadAndExecute");
    assert_eq!(format_access_mask(0x0000_0001), "0x00000001");
    let f = flags_from_win_attributes(0x2 | 0x4 | 0x400 | 0x800);
    assert!(f.contains(NodeFlags::HIDDEN | NodeFlags::SYSTEM | NodeFlags::REPARSE));
    assert!(f.contains(NodeFlags::COMPRESSED));
}

#[cfg(not(windows))]
#[test]
fn scan_non_windows_stubs() {
    use jt_core::platform::*;
    assert!(!is_admin());
    assert!(list_drives().is_empty());
    assert_eq!(long_path("/tmp/x"), "/tmp/x");
    assert!(volume_info("/").is_err());
    assert!(delete_permanently(&["x".into()]).is_err());
}

#[test]
fn recycle_bin_only_on_fixed_drives() {
    use jt_core::platform::{drive_type_has_recycle_bin, no_recycle_bin_error};
    // 3 = DRIVE_FIXED
    assert!(drive_type_has_recycle_bin(3));
    // 0 unbekannt, 1 kein Root, 2 Wechseldatenträger (USB), 4 Netz, 5 CD, 6 RAM-Disk
    for t in [0, 1, 2, 4, 5, 6] {
        assert!(!drive_type_has_recycle_bin(t), "Typ {t}");
    }
    let e = no_recycle_bin_error(r"\\server\share\a.txt");
    assert_eq!(e.kind(), std::io::ErrorKind::Unsupported);
}
