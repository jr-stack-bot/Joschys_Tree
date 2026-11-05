// Joschys_Tree – Test: Löschschutz-Fuzz und Umgehung über andere Pfade – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

use jt_core::NodeFlags;
use jt_core::platform::{Resolved, guard_paths_with};
use jt_core::protect::{ProtectEnv, ProtectError, Reason, classify_path};

/// Zusammengesetzte Schreibweisen geschützter Pfade müssen immer Blocked sein
/// (verkleinerte Variante des Prüfer-Fuzz mit 224k Kombinationen, 30.000 Pfade;
/// Freigaben namens C wie `\\pc\C` gelten wie das Systemlaufwerk).
#[test]
fn fuzz_protected_paths_always_blocked() {
    let env = ProtectEnv::windows_default();
    let roots = [
        "C:",
        "c:",
        r"\\?\C:",
        r"\\.\C:",
        r"\??\C:",
        r"\\pc\c$",
        r"\\localhost\C$",
        "C:/",
        r"\\?\UNC\pc\c$",
        r"\\pc\C",
        r"\\localhost\C",
        r"\\?\UNC\pc\C",
    ];
    let prot: [&[&str]; 17] = [
        &["Windows"],
        &["Windows", "System32"],
        &["Program Files"],
        &["Program Files (x86)"],
        &["ProgramData", "Microsoft"],
        &[
            "Users",
            "joschy",
            "AppData",
            "Local",
            "Microsoft",
            "Windows",
        ],
        &["System Volume Information"],
        &["$Recycle.Bin"],
        &["Recovery"],
        &["Boot"],
        &["EFI"],
        &["Users", "joschy", "NTUSER.DAT"],
        &["pagefile.sys"],
        &["hiberfil.sys"],
        &["swapfile.sys"],
        &["Program Files", "WindowsApps"],
        &[
            "Users",
            "joschy",
            "AppData",
            "Local",
            "Microsoft",
            "Windows",
            "UsrClass.dat",
        ],
    ];
    let mutate = |s: &str, k: u64| -> String {
        match k {
            0 => s.to_string(),
            1 => s.to_uppercase(),
            2 => s.to_lowercase(),
            3 => format!("{s}."),
            4 => format!("{s} "),
            5 => format!("{s}. ."),
            6 => format!("{s}::$DATA"),
            7 => format!("{s}\\."),
            8 => format!("{s}\\\\"),
            _ => s.to_string(),
        }
    };
    let tails = [
        "",
        r"\x",
        r"\a\b\c.dll",
        r"\.",
        r"\..\x",
        "\\x\\",
        r"\deep\deeper\deepest\file.sys",
    ];
    let mut seed: u64 = 0x1234_5678;
    let mut rnd = |n: u64| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed % n
    };
    let mut bad = Vec::new();
    let mut n = 0;
    while n < 30_000 {
        let r = roots[rnd(roots.len() as u64) as usize];
        let pv = prot[rnd(prot.len() as u64) as usize];
        let mut s = r.to_string();
        for comp in pv {
            s.push(if rnd(4) == 0 { '/' } else { '\\' });
            if rnd(6) == 0 {
                s.push('\\');
            }
            s.push_str(&mutate(comp, rnd(10)));
        }
        let tl = tails[rnd(tails.len() as u64) as usize];
        let last = pv[pv.len() - 1].to_lowercase();
        if (last.ends_with(".sys") || last.ends_with(".dat")) && !tl.is_empty() {
            continue;
        }
        s.push_str(tl);
        n += 1;
        for flags in [NodeFlags::empty(), NodeFlags::HIDDEN, NodeFlags::REPARSE] {
            for is_dir in [true, false] {
                let p = classify_path(&s, flags, is_dir, &env);
                if !p.is_blocked() {
                    bad.push(format!("{s:?} {flags:?} dir={is_dir} => {p:?}"));
                }
            }
        }
    }
    assert!(
        bad.is_empty(),
        "{} nicht blockiert: {:?}",
        bad.len(),
        &bad[..bad.len().min(10)]
    );
}

fn blocked_reason(path: &str, resolve: &dyn Fn(&str) -> Resolved) -> Option<Reason> {
    let env = ProtectEnv::windows_default();
    guard_paths_with(&[path.to_string()], &env, resolve)
        .err()
        .and_then(|e| {
            e.get_ref()
                .and_then(|e| e.downcast_ref::<ProtectError>())
                .map(|p| p.reason)
        })
}

/// Die vier Umgehungen aus der Sicherheitsabnahme sind jetzt gesperrt.
#[test]
fn bypass_examples_are_blocked() {
    let same = |p: &str| Resolved::Real(p.to_string());
    // 1) subst X: = C:\
    let subst = |p: &str| Resolved::Real(p.replacen("X:", r"\\?\C:", 1));
    assert_eq!(
        blocked_reason(r"X:\Windows\System32", &subst),
        Some(Reason::WindowsDir)
    );
    // 2) Netzlaufwerk Z: = \\server\c$
    let net = |p: &str| Resolved::Real(p.replacen("Z:", r"\\?\UNC\server\c$", 1));
    assert_eq!(
        blocked_reason(r"Z:\Windows\System32", &net),
        Some(Reason::WindowsDir)
    );
    // 3) Freigabe namens C
    assert_eq!(
        blocked_reason(r"\\pc\C\Windows\System32", &same),
        Some(Reason::AliasedSystem)
    );
    // 4) 8.3-Kurzname ab der 4. Ebene
    assert_eq!(
        blocked_reason(r"C:\Users\joschy\AppData\Local\Microsoft\WINDOW~1", &same),
        Some(Reason::ShortName)
    );
    // Gegenprobe: ohne Auflösung wären 1) und 2) nur Vorsicht (erlaubt).
    assert_eq!(blocked_reason(r"X:\Windows\System32", &same), None);
    assert_eq!(blocked_reason(r"Z:\Windows\System32", &same), None);
}
