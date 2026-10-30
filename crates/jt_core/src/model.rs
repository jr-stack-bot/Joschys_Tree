// Joschys_Tree – Datenmodell: Node, Tree, NodeId, FileTime, ScanMeta, TreeBuilder – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashMap;

use bitflags::bitflags;
use serde::{Deserialize, Serialize};

pub const BRANDING: &str = "@ Joschy";

/// Index in `Tree::nodes`, 0 = Wurzel.
pub type NodeId = u32;
pub const NO_NODE: NodeId = u32::MAX;

/// Differenz zwischen 1601-01-01 und 1970-01-01 in Sekunden.
const FILETIME_UNIX_DIFF_SECS: i64 = 11_644_473_600;
const FILETIME_TICKS_PER_SEC: i64 = 10_000_000;

/// Windows FILETIME (100 ns seit 1601), 0 = unbekannt.
#[derive(Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub struct FileTime(pub i64);

impl FileTime {
    pub fn to_unix_secs(self) -> i64 {
        self.0.div_euclid(FILETIME_TICKS_PER_SEC) - FILETIME_UNIX_DIFF_SECS
    }

    pub fn from_unix_secs(s: i64) -> Self {
        FileTime((s + FILETIME_UNIX_DIFF_SECS) * FILETIME_TICKS_PER_SEC)
    }
}

bitflags! {
    #[derive(Clone, Copy, Default, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
    pub struct NodeFlags: u16 {
        const DIR = 1;
        const HIDDEN = 2;
        const SYSTEM = 4;
        const READONLY = 8;
        /// Junction/Symlink: wird NICHT verfolgt
        const REPARSE = 16;
        const COMPRESSED = 32;
        const SPARSE = 64;
        const HAS_ADS = 128;
        const ACCESS_DENIED = 256;
        const OFFLINE = 512;
    }
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct Node {
    /// nur der Name, kein Pfad (spart RAM)
    pub name: Box<str>,
    pub parent: NodeId,
    /// verkettete Kinderliste: first_child / next_sibling
    pub first_child: NodeId,
    pub next_sibling: NodeId,
    /// logische Größe (bei Ordnern: Summe, von aggregate gesetzt)
    pub size: u64,
    /// belegter Speicher auf Platte
    pub allocated: u64,
    /// bei Ordnern rekursiv; Datei = 0
    pub file_count: u32,
    pub dir_count: u32,
    pub created: FileTime,
    pub modified: FileTime,
    pub accessed: FileTime,
    pub flags: NodeFlags,
    /// Index in Tree::extensions, 0 = ohne Endung
    pub ext_id: u16,
    /// Summe alternativer Datenströme (0 wenn keine/unbekannt)
    pub ads_size: u64,
}

impl Node {
    pub fn is_dir(&self) -> bool {
        self.flags.contains(NodeFlags::DIR)
    }
}

#[derive(Clone, Copy, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum ScanMode {
    Mft,
    Walk,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct ScanMeta {
    /// z. B. "C:\\" oder "\\\\server\\share"
    pub root_path: String,
    pub mode: ScanMode,
    pub started: FileTime,
    pub duration_ms: u64,
    /// 0 = unbekannt (dann allocated = size)
    pub cluster_size: u32,
    pub volume_total: u64,
    pub volume_free: u64,
    /// Anzahl Zugriffsfehler
    pub errors: u32,
    pub tool_version: String,
}

impl ScanMeta {
    /// Bequemer Konstruktor mit Standardwerten (Walk, keine Volume-Infos).
    pub fn new(root_path: impl Into<String>, mode: ScanMode) -> Self {
        ScanMeta {
            root_path: root_path.into(),
            mode,
            started: FileTime::default(),
            duration_ms: 0,
            cluster_size: 0,
            volume_total: 0,
            volume_free: 0,
            errors: 0,
            tool_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct Tree {
    pub nodes: Vec<Node>,
    /// [0] = "" ; kleingeschrieben, ohne Punkt
    pub extensions: Vec<Box<str>>,
    pub meta: ScanMeta,
}

/// Iterator über die direkten Kinder eines Knotens.
pub struct ChildIter<'a> {
    tree: &'a Tree,
    cur: NodeId,
}

impl Iterator for ChildIter<'_> {
    type Item = NodeId;
    fn next(&mut self) -> Option<NodeId> {
        if self.cur == NO_NODE {
            return None;
        }
        let id = self.cur;
        self.cur = self.tree.nodes[id as usize].next_sibling;
        Some(id)
    }
}

impl Tree {
    /// immer 0
    pub fn root(&self) -> NodeId {
        0
    }

    pub fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id as usize]
    }

    pub fn children(&self, id: NodeId) -> ChildIter<'_> {
        ChildIter {
            tree: self,
            cur: self.nodes[id as usize].first_child,
        }
    }

    /// Voller Pfad mit Windows-Trenner '\'. Der Wurzelname ist der Scan-Pfad (z. B. "C:\").
    pub fn full_path(&self, id: NodeId) -> String {
        let mut parts: Vec<&str> = Vec::new();
        let mut cur = id;
        while cur != NO_NODE {
            let n = &self.nodes[cur as usize];
            parts.push(&n.name);
            cur = n.parent;
        }
        parts.reverse();
        let mut out = String::new();
        for (i, p) in parts.iter().enumerate() {
            if i > 0 && !out.ends_with('\\') {
                out.push('\\');
            }
            out.push_str(p);
        }
        out
    }

    /// Sucht einen Knoten über seinen vollen Pfad (case-insensitiv, '\' oder '/').
    pub fn find_path(&self, path: &str) -> Option<NodeId> {
        if self.nodes.is_empty() {
            return None;
        }
        let norm = |s: &str| s.replace('/', "\\").trim_end_matches('\\').to_lowercase();
        let root_name = norm(&self.nodes[0].name);
        let p = norm(path);
        let rest = if p == root_name {
            return Some(0);
        } else if let Some(r) = p.strip_prefix(&root_name) {
            r.strip_prefix('\\')
                .or(if root_name.is_empty() || self.nodes[0].name.ends_with('\\') {
                    Some(r)
                } else {
                    None
                })?
        } else {
            return None;
        };
        let mut cur = 0;
        for comp in rest.split('\\').filter(|c| !c.is_empty()) {
            cur = self
                .children(cur)
                .find(|&c| self.nodes[c as usize].name.to_lowercase() == comp)?;
        }
        Some(cur)
    }

    pub fn ext_of(&self, id: NodeId) -> &str {
        &self.extensions[self.nodes[id as usize].ext_id as usize]
    }
}

#[derive(Clone, Default, Debug)]
pub struct EntryInfo {
    pub size: u64,
    pub allocated: u64,
    pub created: FileTime,
    pub modified: FileTime,
    pub accessed: FileTime,
    pub flags: NodeFlags,
    pub ads_size: u64,
}

struct Orphan {
    key: u64,
    parent_key: u64,
    id: NodeId,
}

/// Wird von beiden Scannern benutzt; garantiert konsistente Trees.
pub struct TreeBuilder {
    nodes: Vec<Node>,
    /// Letztes Kind je Knoten (für O(1)-Anhängen, Reihenfolge bleibt erhalten).
    last_child: Vec<NodeId>,
    extensions: Vec<Box<str>>,
    ext_map: HashMap<Box<str>, u16>,
    orphans: Vec<Orphan>,
    meta: ScanMeta,
}

impl TreeBuilder {
    pub fn new(meta: ScanMeta, root_name: &str) -> Self {
        let mut b = TreeBuilder {
            nodes: Vec::new(),
            last_child: Vec::new(),
            extensions: vec!["".into()],
            ext_map: HashMap::new(),
            orphans: Vec::new(),
            meta,
        };
        b.push_node(
            root_name,
            EntryInfo {
                flags: NodeFlags::DIR,
                ..Default::default()
            },
            true,
        );
        b
    }

    fn ext_id(&mut self, name: &str) -> u16 {
        let ext = match name.rfind('.') {
            Some(pos) if pos > 0 && pos + 1 < name.len() => name[pos + 1..].to_lowercase(),
            _ => return 0,
        };
        if let Some(&id) = self.ext_map.get(ext.as_str()) {
            return id;
        }
        if self.extensions.len() > u16::MAX as usize {
            return 0; // Tabelle voll: als "ohne Endung" führen
        }
        let id = self.extensions.len() as u16;
        let boxed: Box<str> = ext.into();
        self.extensions.push(boxed.clone());
        self.ext_map.insert(boxed, id);
        id
    }

    fn push_node(&mut self, name: &str, info: EntryInfo, is_dir: bool) -> NodeId {
        let mut flags = info.flags;
        if is_dir {
            flags |= NodeFlags::DIR;
        } else {
            flags.remove(NodeFlags::DIR);
        }
        if info.ads_size > 0 {
            flags |= NodeFlags::HAS_ADS;
        }
        let ext_id = if is_dir { 0 } else { self.ext_id(name) };
        let id = self.nodes.len() as NodeId;
        self.nodes.push(Node {
            name: name.into(),
            parent: NO_NODE,
            first_child: NO_NODE,
            next_sibling: NO_NODE,
            size: if is_dir { 0 } else { info.size },
            allocated: if is_dir { 0 } else { info.allocated },
            file_count: 0,
            dir_count: 0,
            created: info.created,
            modified: info.modified,
            accessed: info.accessed,
            flags,
            ext_id,
            ads_size: info.ads_size,
        });
        self.last_child.push(NO_NODE);
        id
    }

    fn link(&mut self, parent: NodeId, child: NodeId) {
        self.nodes[child as usize].parent = parent;
        let last = self.last_child[parent as usize];
        if last == NO_NODE {
            self.nodes[parent as usize].first_child = child;
        } else {
            self.nodes[last as usize].next_sibling = child;
        }
        self.last_child[parent as usize] = child;
    }

    pub fn add_dir(&mut self, parent: NodeId, name: &str, info: EntryInfo) -> NodeId {
        let id = self.push_node(name, info, true);
        self.link(parent, id);
        id
    }

    pub fn add_file(&mut self, parent: NodeId, name: &str, info: EntryInfo) -> NodeId {
        let id = self.push_node(name, info, false);
        self.link(parent, id);
        id
    }

    /// MFT: Einträge kommen in beliebiger Reihenfolge -> Eltern später verknüpfen.
    /// `parent_key == key` oder ein unbekannter `parent_key` hängt den Eintrag an die Wurzel.
    pub fn add_orphan(
        &mut self,
        key: u64,
        parent_key: u64,
        name: &str,
        info: EntryInfo,
        is_dir: bool,
    ) {
        let id = self.push_node(name, info, is_dir);
        self.orphans.push(Orphan {
            key,
            parent_key,
            id,
        });
    }

    /// Verknüpft Waisen, ruft aggregate::recompute.
    pub fn finish(mut self) -> Tree {
        if !self.orphans.is_empty() {
            let orphans = std::mem::take(&mut self.orphans);
            let by_key: HashMap<u64, NodeId> = orphans
                .iter()
                .filter(|o| self.nodes[o.id as usize].is_dir())
                .map(|o| (o.key, o.id))
                .collect();
            for o in &orphans {
                let parent = match by_key.get(&o.parent_key) {
                    Some(&p) if o.parent_key != o.key => p,
                    _ => 0,
                };
                self.link(parent, o.id);
            }
            // Zyklen (a->b->a) sind danach unerreichbar: deren Köpfe an die Wurzel hängen.
            self.fix_unreachable();
        }
        let mut tree = Tree {
            nodes: self.nodes,
            extensions: self.extensions,
            meta: self.meta,
        };
        crate::aggregate::recompute(&mut tree);
        tree
    }

    fn fix_unreachable(&mut self) {
        let n = self.nodes.len();
        let mut reach = vec![false; n];
        let mut stack = vec![0 as NodeId];
        while let Some(id) = stack.pop() {
            if reach[id as usize] {
                continue;
            }
            reach[id as usize] = true;
            let mut c = self.nodes[id as usize].first_child;
            while c != NO_NODE {
                stack.push(c);
                c = self.nodes[c as usize].next_sibling;
            }
        }
        for id in 1..n {
            if !reach[id] {
                // Aus alter Liste lösen ist bei Zyklen nicht nötig: der Zyklus ist ohnehin unerreichbar.
                // Wir hängen nur den Kopf an die Wurzel; dessen alter Elternteil verliert ihn.
                let id = id as NodeId;
                let old_parent = self.nodes[id as usize].parent;
                if old_parent != NO_NODE && !reach[old_parent as usize] {
                    self.unlink(old_parent, id);
                }
                self.nodes[id as usize].next_sibling = NO_NODE;
                self.link(0, id);
                // Teilbaum als erreichbar markieren
                let mut st = vec![id];
                while let Some(x) = st.pop() {
                    if reach[x as usize] {
                        continue;
                    }
                    reach[x as usize] = true;
                    let mut c = self.nodes[x as usize].first_child;
                    while c != NO_NODE {
                        st.push(c);
                        c = self.nodes[c as usize].next_sibling;
                    }
                }
            }
        }
    }

    fn unlink(&mut self, parent: NodeId, child: NodeId) {
        let mut prev = NO_NODE;
        let mut c = self.nodes[parent as usize].first_child;
        while c != NO_NODE {
            let next = self.nodes[c as usize].next_sibling;
            if c == child {
                if prev == NO_NODE {
                    self.nodes[parent as usize].first_child = next;
                } else {
                    self.nodes[prev as usize].next_sibling = next;
                }
                if self.last_child[parent as usize] == child {
                    self.last_child[parent as usize] = prev;
                }
                return;
            }
            prev = c;
            c = next;
        }
    }
}

#[doc(hidden)]
pub mod testutil {
    //! Test-Helfer für alle Aufträge.
    use super::*;

    /// Kleiner 3-Ebenen-Baum (Wurzel "C:\"):
    ///
    /// ```text
    /// C:\                          (0)
    /// ├─ Users                     (1)  Ordner
    /// │  ├─ joschy                 (2)  Ordner
    /// │  │  ├─ Bild.JPG            (3)  3000 B, alloc 4096, modified 2026-01-01
    /// │  │  └─ notiz.txt           (4)   100 B, alloc 4096, modified 2020-01-01, HIDDEN
    /// │  └─ readme.md              (5)   500 B, alloc 4096, modified 2025-06-01
    /// ├─ Windows                   (6)  Ordner
    /// │  └─ system.dll             (7) 10000 B, alloc 12288, modified 2024-01-01, SYSTEM
    /// └─ pagefile.sys              (8) 50000 B, alloc 53248, modified 2026-10-01, ADS 10
    /// ```
    ///
    /// Summen Wurzel: size 63600, allocated 77824, file_count 5, dir_count 3.
    pub fn sample_tree() -> Tree {
        let mut meta = ScanMeta::new("C:\\", ScanMode::Walk);
        meta.cluster_size = 4096;
        meta.started = FileTime::from_unix_secs(1_793_318_400); // 2026-10-30
        let mut b = TreeBuilder::new(meta, "C:\\");
        let t = |y: i32, m: u32, d: u32| {
            let secs = chrono::NaiveDate::from_ymd_opt(y, m, d)
                .and_then(|d| d.and_hms_opt(12, 0, 0))
                .map(|dt| dt.and_utc().timestamp())
                .unwrap_or(0);
            FileTime::from_unix_secs(secs)
        };
        let file = |size: u64, alloc: u64, modified: FileTime, flags: NodeFlags| EntryInfo {
            size,
            allocated: alloc,
            created: modified,
            modified,
            accessed: modified,
            flags,
            ads_size: 0,
        };
        let dir = |modified: FileTime| EntryInfo {
            created: modified,
            modified,
            accessed: modified,
            ..Default::default()
        };
        let users = b.add_dir(0, "Users", dir(t(2020, 1, 1)));
        let joschy = b.add_dir(users, "joschy", dir(t(2020, 1, 1)));
        b.add_file(
            joschy,
            "Bild.JPG",
            file(3000, 4096, t(2026, 1, 1), NodeFlags::empty()),
        );
        b.add_file(
            joschy,
            "notiz.txt",
            file(100, 4096, t(2020, 1, 1), NodeFlags::HIDDEN),
        );
        b.add_file(
            users,
            "readme.md",
            file(500, 4096, t(2025, 6, 1), NodeFlags::empty()),
        );
        let windows = b.add_dir(0, "Windows", dir(t(2019, 1, 1)));
        b.add_file(
            windows,
            "system.dll",
            file(10000, 12288, t(2024, 1, 1), NodeFlags::SYSTEM),
        );
        let mut page = file(50000, 53248, t(2026, 10, 1), NodeFlags::HIDDEN | NodeFlags::SYSTEM);
        page.ads_size = 10;
        b.add_file(0, "pagefile.sys", page);
        b.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::testutil::sample_tree;
    use super::*;

    #[test]
    fn filetime_roundtrip() {
        let t = FileTime::from_unix_secs(1_700_000_000);
        assert_eq!(t.to_unix_secs(), 1_700_000_000);
        assert_eq!(FileTime::from_unix_secs(0).0, 116_444_736_000_000_000);
    }

    #[test]
    fn orphans_link_any_order() {
        let mut b = TreeBuilder::new(ScanMeta::new("C:\\", ScanMode::Mft), "C:\\");
        b.add_orphan(
            10,
            5,
            "f.txt",
            EntryInfo {
                size: 7,
                ..Default::default()
            },
            false,
        );
        b.add_orphan(5, 5, "dir", EntryInfo::default(), true);
        b.add_orphan(6, 6, "x", EntryInfo::default(), true);
        b.add_orphan(7, 8, "a", EntryInfo::default(), true);
        b.add_orphan(8, 7, "b", EntryInfo::default(), true); // Zyklus
        let t = b.finish();
        let f = t.find_path("C:\\dir\\f.txt").expect("f.txt");
        assert_eq!(t.node(f).size, 7);
        assert_eq!(t.node(0).size, 7);
        // alle Knoten erreichbar
        let mut count = 0;
        let mut st = vec![0];
        while let Some(id) = st.pop() {
            count += 1;
            st.extend(t.children(id));
        }
        assert_eq!(count, t.nodes.len());
    }

    #[test]
    fn sample_tree_consistent() {
        let t = sample_tree();
        assert_eq!(t.nodes.len(), 9);
        assert_eq!(t.root(), 0);
        assert_eq!(t.node(0).parent, NO_NODE);
        // Eltern-/Kind-Verweise stimmen überein
        for id in 0..t.nodes.len() as NodeId {
            for c in t.children(id) {
                assert_eq!(t.node(c).parent, id);
            }
            if id != 0 {
                let p = t.node(id).parent;
                assert!(t.children(p).any(|c| c == id));
            }
        }
        let r = t.node(0);
        assert_eq!(r.size, 63_600);
        assert_eq!(r.allocated, 77_824);
        assert_eq!(r.file_count, 5);
        assert_eq!(r.dir_count, 3);
        let users = t.find_path("c:\\users").unwrap();
        assert_eq!(t.node(users).size, 3600);
        assert_eq!(t.node(users).file_count, 3);
        assert_eq!(t.node(users).dir_count, 1);
        let jpg = t.find_path("C:\\USERS\\Joschy\\bild.jpg").unwrap();
        assert_eq!(t.full_path(jpg), "C:\\Users\\joschy\\Bild.JPG");
        assert_eq!(t.ext_of(jpg), "jpg");
        assert_eq!(t.ext_of(users), "");
        assert_eq!(t.find_path("C:\\"), Some(0));
        assert_eq!(t.find_path("C:\\nope"), None);
        let page = t.find_path("C:\\pagefile.sys").unwrap();
        assert!(t.node(page).flags.contains(NodeFlags::HAS_ADS));
        assert_eq!(t.extensions[0].as_ref(), "");
    }
}
