// Joschys_Tree – Lösch-Entscheidung (Admin, Live-Scan, Löschschutz) und Schutz-Cache – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later
//
// Eine einzige, reine Entscheidungsfunktion für alle Lösch- und Korb-Wege. Die GUI
// zeigt Menüpunkte nur an bzw. aktiviert sie nur, wenn diese Funktion zustimmt; der
// Löschweg in jt_core prüft zusätzlich unabhängig (guard_paths).

use std::collections::HashMap;

use jt_core::protect::{ProtectEnv, Protection, Reason, classify, contains_blocked};
use jt_core::{Key, Lang, NO_NODE, NodeId, Tree, tr};

/// Warum ein Element nicht gelöscht werden darf.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Deny {
    /// Das Element selbst ist geschützt.
    Protected(Reason),
    NotAdmin,
    /// Snapshot statt Live-Scan.
    NotLive,
    /// Ordner mit geschützten Nachkommen.
    ContainsBlocked,
    /// Wurzel des Scans.
    ScanRoot,
}

impl Deny {
    pub fn text(self, lang: Lang) -> &'static str {
        match self {
            Deny::Protected(r) => r.text(lang),
            Deny::NotAdmin => tr(lang, Key::DelAdminOnly),
            Deny::NotLive => tr(lang, Key::DenyNotLive),
            Deny::ContainsBlocked => tr(lang, Key::DenyContainsBlocked),
            Deny::ScanRoot => tr(lang, Key::DenyScanRoot),
        }
    }
}

/// Ergebnis der Lösch-Entscheidung.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    Allowed,
    /// Erlaubt, aber nur mit Warnung und Häkchen "Ich weiß, was ich tue".
    NeedsAck(Reason),
    Denied(Deny),
}

impl Decision {
    pub fn is_allowed(self) -> bool {
        !matches!(self, Decision::Denied(_))
    }
}

/// Kern der Entscheidung ohne Rechte-Prüfung (gilt auch für den Auswahlkorb).
/// `inner_blocked` wird nur für Ordner aufgerufen, deren eigene Stufe nicht gesperrt ist.
pub fn decide(
    prot: Protection,
    is_root: bool,
    is_dir: bool,
    inner_blocked: impl FnOnce() -> bool,
) -> Decision {
    if is_root {
        return Decision::Denied(Deny::ScanRoot);
    }
    if let Protection::Blocked(r) = prot {
        return Decision::Denied(Deny::Protected(r));
    }
    if is_dir && inner_blocked() {
        return Decision::Denied(Deny::ContainsBlocked);
    }
    match prot {
        Protection::Caution(r) => Decision::NeedsAck(r),
        _ => Decision::Allowed,
    }
}

/// Darf `id` gelöscht werden? Nur als Admin, nur im Live-Scan, nur wenn der
/// Löschschutz es erlaubt und (bei Ordnern) nichts Geschütztes darin liegt.
pub fn delete_decision(
    env: &ProtectEnv,
    tree: &Tree,
    id: NodeId,
    admin: bool,
    live: bool,
) -> Decision {
    if !admin {
        return Decision::Denied(Deny::NotAdmin);
    }
    if !live {
        return Decision::Denied(Deny::NotLive);
    }
    let n = tree.node(id);
    decide(
        classify(tree, id, env),
        id == tree.root(),
        n.is_dir(),
        || contains_blocked(tree, id, env),
    )
}

/// Schutz-Umgebung des laufenden Systems (auf Nicht-Windows: Windows-Standard).
pub fn env() -> &'static ProtectEnv {
    jt_core::platform::protect_env()
}

/// Zwischenspeicher der Schutzstufen je Knoten. Nur angefragte (sichtbare) Knoten
/// werden bewertet; geleert, sobald sich `tree_gen` ändert.
#[derive(Default)]
pub struct ProtCache {
    gen_: u64,
    prot: HashMap<NodeId, Protection>,
    inner: HashMap<NodeId, bool>,
}

impl ProtCache {
    /// Bei neuer Baum-Generation alles verwerfen.
    pub fn sync(&mut self, gen_: u64) {
        if self.gen_ != gen_ {
            self.prot.clear();
            self.inner.clear();
            self.gen_ = gen_;
        }
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.prot.len()
    }

    /// Schutzstufe eines Knotens (gecacht). Unter einem vererbend gesperrten,
    /// bereits bewerteten Elternordner wird nicht neu bewertet.
    pub fn protection(&mut self, tree: &Tree, id: NodeId) -> Protection {
        if let Some(&p) = self.prot.get(&id) {
            return p;
        }
        let parent = tree.node(id).parent;
        let p = match (parent != NO_NODE)
            .then(|| self.prot.get(&parent))
            .flatten()
        {
            Some(&Protection::Blocked(r)) if r.is_inherited() => Protection::Blocked(r),
            _ => classify(tree, id, env()),
        };
        self.prot.insert(id, p);
        p
    }

    /// Enthält der Teilbaum ein gesperrtes Element? (gecacht, Tiefensuche nur einmal)
    pub fn contains_blocked(&mut self, tree: &Tree, id: NodeId) -> bool {
        *self
            .inner
            .entry(id)
            .or_insert_with(|| contains_blocked(tree, id, env()))
    }

    /// Darf das Element in den Auswahlkorb? (Schutz ohne Rechte-Prüfung)
    pub fn basket_decision(&mut self, tree: &Tree, id: NodeId) -> Decision {
        let prot = self.protection(tree, id);
        let is_dir = tree.node(id).is_dir();
        decide(prot, id == tree.root(), is_dir, || {
            self.contains_blocked(tree, id)
        })
    }

    /// Wie [`delete_decision`], mit Cache.
    pub fn decision(&mut self, tree: &Tree, id: NodeId, admin: bool, live: bool) -> Decision {
        if !admin {
            return Decision::Denied(Deny::NotAdmin);
        }
        if !live {
            return Decision::Denied(Deny::NotLive);
        }
        self.basket_decision(tree, id)
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use jt_core::model::{EntryInfo, ScanMeta, ScanMode, TreeBuilder};
    use jt_core::{FileTime, NodeFlags};

    fn file(size: u64, flags: NodeFlags) -> EntryInfo {
        EntryInfo {
            size,
            allocated: size,
            modified: FileTime::from_unix_secs(1_700_000_000),
            flags,
            ..Default::default()
        }
    }

    /// Kleiner Windows-Baum für die Entscheidungstests.
    pub fn win_tree() -> Tree {
        let mut b = TreeBuilder::new(ScanMeta::new("C:\\", ScanMode::Walk), "C:\\");
        let d = EntryInfo::default();
        let win = b.add_dir(0, "Windows", d.clone());
        b.add_file(win, "notepad.exe", file(100, NodeFlags::empty()));
        let users = b.add_dir(0, "Users", d.clone());
        let demo = b.add_dir(users, "demo", d.clone());
        let dl = b.add_dir(demo, "Downloads", d.clone());
        b.add_file(dl, "old.iso", file(5000, NodeFlags::empty()));
        let app = b.add_dir(demo, "AppData", d.clone());
        let roam = b.add_dir(app, "Roaming", d.clone());
        b.add_file(roam, "cfg.json", file(10, NodeFlags::empty()));
        let backup = b.add_dir(0, "Backup", d.clone());
        b.add_file(backup, "a.zip", file(700, NodeFlags::empty()));
        b.add_file(backup, "sys.dat", file(7, NodeFlags::SYSTEM));
        let clean = b.add_dir(0, "Data", d);
        b.add_file(clean, "x.txt", file(3, NodeFlags::empty()));
        b.add_file(0, "pagefile.sys", file(9000, NodeFlags::HIDDEN));
        b.finish()
    }

    fn id(t: &Tree, p: &str) -> NodeId {
        t.find_path(p).unwrap()
    }

    #[test]
    fn decision_rules() {
        let t = win_tree();
        let env = ProtectEnv::windows_default();
        let d = |p: &str| delete_decision(&env, &t, id(&t, p), true, true);
        assert_eq!(d("C:\\Users\\demo\\Downloads\\old.iso"), Decision::Allowed);
        assert_eq!(d("C:\\Data"), Decision::Allowed);
        assert!(matches!(
            d("C:\\Windows"),
            Decision::Denied(Deny::Protected(Reason::WindowsDir))
        ));
        assert!(matches!(
            d("C:\\Windows\\notepad.exe"),
            Decision::Denied(Deny::Protected(_))
        ));
        assert!(matches!(
            d("C:\\pagefile.sys"),
            Decision::Denied(Deny::Protected(_))
        ));
        assert!(matches!(
            d("C:\\Users\\demo\\AppData\\Roaming\\cfg.json"),
            Decision::NeedsAck(Reason::AppData)
        ));
        // Ordner mit gesperrter Datei (SYSTEM-Attribut) ist als Ganzes nicht löschbar,
        // die freie Datei daneben schon.
        assert_eq!(d("C:\\Backup"), Decision::Denied(Deny::ContainsBlocked));
        assert_eq!(d("C:\\Backup\\a.zip"), Decision::Allowed);
        // Wurzel des Scans nie.
        assert_eq!(
            delete_decision(&env, &t, 0, true, true),
            Decision::Denied(Deny::ScanRoot)
        );
    }

    #[test]
    fn decision_needs_admin_and_live_scan() {
        let t = win_tree();
        let env = ProtectEnv::windows_default();
        let iso = id(&t, "C:\\Users\\demo\\Downloads\\old.iso");
        assert_eq!(
            delete_decision(&env, &t, iso, false, true),
            Decision::Denied(Deny::NotAdmin)
        );
        assert_eq!(
            delete_decision(&env, &t, iso, true, false),
            Decision::Denied(Deny::NotLive)
        );
        assert_eq!(
            delete_decision(&env, &t, iso, false, false),
            Decision::Denied(Deny::NotAdmin)
        );
    }

    #[test]
    fn decide_skips_subtree_walk_when_not_needed() {
        let called = std::cell::Cell::new(false);
        let r = decide(Protection::Blocked(Reason::WindowsDir), false, true, || {
            called.set(true);
            false
        });
        assert!(matches!(r, Decision::Denied(Deny::Protected(_))));
        assert!(!called.get());
        let r = decide(Protection::Free, false, false, || {
            called.set(true);
            true
        });
        assert_eq!(r, Decision::Allowed);
        assert!(!called.get(), "Dateien brauchen keine Teilbaum-Suche");
    }

    #[test]
    fn cache_matches_pure_function_and_resets() {
        let t = win_tree();
        #[cfg(not(windows))]
        assert_eq!(
            ProtectEnv::windows_default(),
            *super::env(),
            "Linux-Tests nutzen den Windows-Standard"
        );
        let env = super::env();
        let mut c = ProtCache::default();
        c.sync(1);
        for i in 0..t.nodes.len() as NodeId {
            assert_eq!(
                c.decision(&t, i, true, true),
                delete_decision(env, &t, i, true, true),
                "{}",
                t.full_path(i)
            );
            assert_eq!(c.protection(&t, i), classify(&t, i, env));
        }
        assert_eq!(
            c.decision(&t, 3, false, true),
            Decision::Denied(Deny::NotAdmin)
        );
        assert!(c.len() > 0);
        c.sync(1);
        assert!(c.len() > 0);
        c.sync(2);
        assert_eq!(c.len(), 0);
    }

    #[test]
    fn deny_texts_are_translated() {
        for d in [
            Deny::Protected(Reason::WindowsDir),
            Deny::NotAdmin,
            Deny::NotLive,
            Deny::ContainsBlocked,
            Deny::ScanRoot,
        ] {
            assert!(!d.text(Lang::De).is_empty());
            assert_ne!(d.text(Lang::De), d.text(Lang::En));
        }
    }
}
