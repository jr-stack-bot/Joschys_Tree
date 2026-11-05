// Joschys_Tree – Lösch-Entscheidung (Admin, Live-Scan, Löschschutz) und Schutz-Cache – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later
//
// Eine einzige, reine Entscheidungsfunktion für alle Lösch- und Korb-Wege. Die GUI
// zeigt Menüpunkte nur an bzw. aktiviert sie nur, wenn diese Funktion zustimmt; der
// Löschweg in jt_core prüft zusätzlich unabhängig (guard_paths).
//
// Zusätzlich zum Pfadtext wird der "echte" Pfad bewertet: die beim Scan-Start
// aufgelöste Scan-Wurzel (`RealRoot`, z. B. X: -> C:\ bei subst) plus der relative
// Pfad des Knotens im Baum. Die strengere Stufe gilt.

use std::collections::HashMap;
use std::io;

use jt_core::protect::{
    ProtectEnv, Protection, Reason, classify, classify_real, contains_blocked_real, stricter,
};
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

/// Beim Scan-Start aufgelöste Scan-Wurzel (neben `meta.root_path`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RealRoot {
    /// Aufgelöster Pfad, bei Fehler der Roh-Pfad.
    pub path: String,
    /// `false`: Auflösung fehlgeschlagen, `path` ist der Roh-Pfad.
    pub resolved: bool,
}

impl RealRoot {
    /// Löst `root` mit `canon` auf; bei Fehler gilt der Roh-Pfad (und das wird gemerkt).
    pub fn resolve(root: &str, canon: impl FnOnce(&str) -> io::Result<String>) -> Self {
        match canon(root) {
            Ok(p) => RealRoot {
                path: jt_core::platform::strip_long_prefix(&p),
                resolved: true,
            },
            Err(_) => RealRoot {
                path: root.to_string(),
                resolved: false,
            },
        }
    }

    /// Echte Auflösung mit `std::fs::canonicalize`.
    pub fn from_system(root: &str) -> Self {
        Self::resolve(root, |r| {
            std::fs::canonicalize(jt_core::platform::long_path(r))
                .map(|p| p.to_string_lossy().into_owned())
        })
    }

    /// Pfad für die zusätzliche Bewertung (nur wenn aufgelöst; sonst reicht der Roh-Pfad).
    pub fn effective(&self) -> Option<&str> {
        self.resolved.then_some(self.path.as_str())
    }
}

fn effective(real: Option<&RealRoot>) -> Option<&str> {
    real.and_then(RealRoot::effective)
}

/// Schutzstufe eines Knotens mit echter Scan-Wurzel: Roh- und echter Pfad, die
/// strengere Stufe gilt. Ist die Wurzel nicht auflösbar, ist alles gesperrt
/// (`Unresolvable`), damit die GUI es schon vor dem Dialog so anzeigt wie der Löschweg.
pub fn protection_with_root(
    tree: &Tree,
    id: NodeId,
    real: Option<&RealRoot>,
    env: &ProtectEnv,
) -> Protection {
    match real {
        Some(r) if !r.resolved => stricter(
            classify(tree, id, env),
            Protection::Blocked(Reason::Unresolvable),
        ),
        _ => classify_real(tree, id, effective(real), env),
    }
}

/// Darf `id` gelöscht werden? Nur als Admin, nur im Live-Scan, nur wenn der
/// Löschschutz es erlaubt und (bei Ordnern) nichts Geschütztes darin liegt.
/// Bewertet Roh-Pfad und echten Pfad unter `real` (die strengere Stufe gilt).
pub fn delete_decision(
    env: &ProtectEnv,
    tree: &Tree,
    id: NodeId,
    admin: bool,
    live: bool,
    real: Option<&RealRoot>,
) -> Decision {
    if !admin {
        return Decision::Denied(Deny::NotAdmin);
    }
    if !live {
        return Decision::Denied(Deny::NotLive);
    }
    let n = tree.node(id);
    let rr = effective(real);
    decide(
        protection_with_root(tree, id, real, env),
        id == tree.root(),
        n.is_dir(),
        || contains_blocked_real(tree, id, rr, env),
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
    /// Aufgelöste Scan-Wurzel des aktuellen Baums (nur Live-Scan).
    real: Option<RealRoot>,
}

impl ProtCache {
    /// Bei neuer Baum-Generation alles verwerfen (auch die echte Wurzel).
    pub fn sync(&mut self, gen_: u64) {
        if self.gen_ != gen_ {
            self.prot.clear();
            self.inner.clear();
            self.real = None;
            self.gen_ = gen_;
        }
    }

    /// Echte Scan-Wurzel des aktuellen Baums setzen (verwirft gecachte Stufen).
    pub fn set_real_root(&mut self, real: Option<RealRoot>) {
        self.prot.clear();
        self.inner.clear();
        self.real = real;
    }

    pub fn real_root(&self) -> Option<&RealRoot> {
        self.real.as_ref()
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
            _ => protection_with_root(tree, id, self.real.as_ref(), env()),
        };
        self.prot.insert(id, p);
        p
    }

    /// Enthält der Teilbaum ein gesperrtes Element? (gecacht, Tiefensuche nur einmal)
    pub fn contains_blocked(&mut self, tree: &Tree, id: NodeId) -> bool {
        let rr = effective(self.real.as_ref());
        *self
            .inner
            .entry(id)
            .or_insert_with(|| contains_blocked_real(tree, id, rr, env()))
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
        let d = |p: &str| delete_decision(&env, &t, id(&t, p), true, true, None);
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
            delete_decision(&env, &t, 0, true, true, None),
            Decision::Denied(Deny::ScanRoot)
        );
    }

    #[test]
    fn decision_needs_admin_and_live_scan() {
        let t = win_tree();
        let env = ProtectEnv::windows_default();
        let iso = id(&t, "C:\\Users\\demo\\Downloads\\old.iso");
        assert_eq!(
            delete_decision(&env, &t, iso, false, true, None),
            Decision::Denied(Deny::NotAdmin)
        );
        assert_eq!(
            delete_decision(&env, &t, iso, true, false, None),
            Decision::Denied(Deny::NotLive)
        );
        assert_eq!(
            delete_decision(&env, &t, iso, false, false, None),
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
                delete_decision(env, &t, i, true, true, None),
                "{}",
                t.full_path(i)
            );
            assert_eq!(c.protection(&t, i), jt_core::protect::classify(&t, i, env));
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

    /// Baum mit Scan-Wurzel X:\ (subst-/Netzlaufwerk).
    fn x_tree() -> Tree {
        let mut b = TreeBuilder::new(ScanMeta::new("X:\\", ScanMode::Walk), "X:\\");
        let d = EntryInfo::default();
        let win = b.add_dir(0, "Windows", d.clone());
        let sys = b.add_dir(win, "System32", d.clone());
        b.add_file(sys, "ntoskrnl.exe", file(10, NodeFlags::empty()));
        let data = b.add_dir(0, "Daten", d);
        b.add_file(data, "a.txt", file(3, NodeFlags::empty()));
        b.finish()
    }

    #[test]
    fn real_root_resolution() {
        let ok = RealRoot::resolve("X:\\", |_| Ok(r"\\?\C:\".to_string()));
        assert_eq!(ok.path, r"C:\");
        assert!(ok.resolved);
        assert_eq!(ok.effective(), Some(r"C:\"));
        let unc = RealRoot::resolve("Z:\\", |_| Ok(r"\\?\UNC\server\c$\".to_string()));
        assert_eq!(unc.path, r"\\server\c$\");
        let bad = RealRoot::resolve("X:\\", |_| Err(io::Error::other("kaputt")));
        assert_eq!(bad.path, "X:\\");
        assert!(!bad.resolved);
        assert_eq!(bad.effective(), None);
        // Echte Auflösung: nicht existierender Pfad -> Roh-Pfad, gemerkt.
        let none = RealRoot::from_system("/gibt/es/nicht/joschy");
        assert!(!none.resolved);
    }

    #[test]
    fn decision_uses_real_root() {
        let t = x_tree();
        let env = ProtectEnv::windows_default();
        let sys = id(&t, "X:\\Windows\\System32");
        let win = id(&t, "X:\\Windows");
        let txt = id(&t, "X:\\Daten\\a.txt");
        let d = |n: NodeId, r: Option<&RealRoot>| delete_decision(&env, &t, n, true, true, r);
        // Ohne echte Wurzel: nur Vorsicht (fremdes Laufwerk).
        assert_eq!(d(sys, None), Decision::NeedsAck(Reason::ForeignSystem));
        // subst X: = C:\ -> gesperrt
        let subst = RealRoot::resolve("X:\\", |_| Ok(r"\\?\C:\".to_string()));
        assert!(matches!(
            d(sys, Some(&subst)),
            Decision::Denied(Deny::Protected(Reason::WindowsDir))
        ));
        assert!(matches!(d(win, Some(&subst)), Decision::Denied(_)));
        assert_eq!(d(txt, Some(&subst)), Decision::Allowed);
        // Netzlaufwerk Z: = \\server\c$ -> gesperrt
        let net = RealRoot::resolve("X:\\", |_| Ok(r"\\?\UNC\server\c$".to_string()));
        assert!(matches!(d(sys, Some(&net)), Decision::Denied(_)));
        // Netzlaufwerk auf normale Freigabe -> Systemordner dort gesperrt
        let nas = RealRoot::resolve("X:\\", |_| Ok(r"\\nas\daten".to_string()));
        assert!(matches!(
            d(win, Some(&nas)),
            Decision::Denied(Deny::Protected(Reason::AliasedSystem))
        ));
        // subst auf echtes anderes Laufwerk -> bleibt Vorsicht
        let other = RealRoot::resolve("X:\\", |_| Ok(r"D:\".to_string()));
        assert_eq!(
            d(sys, Some(&other)),
            Decision::NeedsAck(Reason::ForeignSystem)
        );
        // Auflösung fehlgeschlagen -> schon vor dem Dialog gesperrt (wie im Löschweg)
        let bad = RealRoot::resolve("X:\\", |_| Err(io::Error::other("x")));
        assert_eq!(
            d(sys, Some(&bad)),
            Decision::Denied(Deny::Protected(Reason::Unresolvable))
        );
        assert_eq!(
            d(txt, Some(&bad)),
            Decision::Denied(Deny::Protected(Reason::Unresolvable))
        );
        assert_eq!(
            protection_with_root(&t, txt, Some(&bad), &env),
            Protection::Blocked(Reason::Unresolvable)
        );
        assert_eq!(protection_with_root(&t, txt, None, &env), Protection::Free);
        let mut cb = ProtCache::default();
        cb.sync(9);
        cb.set_real_root(Some(bad.clone()));
        assert_eq!(
            cb.basket_decision(&t, txt),
            Decision::Denied(Deny::Protected(Reason::Unresolvable))
        );
        // Cache: gleiche Entscheidungen, set_real_root verwirft gecachte Stufen.
        let mut c = ProtCache::default();
        c.sync(5);
        assert_eq!(c.decision(&t, sys, true, true), d(sys, None));
        c.set_real_root(Some(subst.clone()));
        assert_eq!(c.real_root(), Some(&subst));
        for n in 0..t.nodes.len() as NodeId {
            assert_eq!(c.decision(&t, n, true, true), d(n, Some(&subst)), "{n}");
        }
        assert!(c.contains_blocked(&t, 0));
        // Neue Generation: echte Wurzel weg.
        c.sync(6);
        assert_eq!(c.real_root(), None);
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
