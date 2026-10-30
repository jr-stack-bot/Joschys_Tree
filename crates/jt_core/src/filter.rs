// Joschys_Tree – Filter-Ausdruck und Suche – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

use globset::{GlobBuilder, GlobMatcher};
use serde::{Deserialize, Serialize};

use crate::aggregate::for_each_descendant;
use crate::model::{FileTime, NodeFlags, NodeId, Tree};

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum AgeField {
    #[default]
    Modified,
    Accessed,
    Created,
}

/// Filter für Suche und Top-Listen. Alle gesetzten Bedingungen müssen zutreffen (UND).
///
/// - `name_glob`: Glob auf den Namen (`*`, `?`, `[..]`, `{a,b}`), ohne Groß-/Kleinschreibung.
///   Ohne Platzhalter wird als Teilstring gesucht ("bild" findet "Urlaubsbild.jpg").
///   Ein ungültiger Glob trifft nichts.
/// - `min_size`/`max_size`: inklusive Grenzen auf `size`.
/// - `older_than_days`: Alter >= Tage; `newer_than_days`: Alter < Tage (Feld laut `age_field`).
///   Unbekannte Zeit (0) erfüllt keine Altersbedingung.
/// - `extensions`: Endungen ohne Groß-/Kleinschreibung, "jpg", ".jpg" und "*.jpg" gleichwertig.
///   Ordner treffen nie, wenn die Liste nicht leer ist.
/// - `files_only`: Ordner ausschließen. `include_hidden == false`: versteckte Einträge ausschließen.
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct Filter {
    pub name_glob: Option<String>,
    pub min_size: Option<u64>,
    pub max_size: Option<u64>,
    pub older_than_days: Option<u32>,
    pub newer_than_days: Option<u32>,
    pub age_field: AgeField,
    pub extensions: Vec<String>,
    pub files_only: bool,
    pub include_hidden: bool,
}

impl Filter {
    /// Prüft einen einzelnen Knoten. Für viele Knoten `search` benutzen (kompiliert nur einmal).
    pub fn matches(&self, tree: &Tree, id: NodeId, now: FileTime) -> bool {
        CompiledFilter::new(self, tree, now).matches(tree, id)
    }
}

/// Aktuelle Zeit als FileTime.
pub fn now() -> FileTime {
    FileTime::from_unix_secs(chrono::Utc::now().timestamp())
}

const TICKS_PER_DAY: i64 = 86_400 * 10_000_000;

enum NameMatch {
    Any,
    None,
    Glob(GlobMatcher),
    /// kleingeschriebener Teilstring
    Contains(String),
}

/// Einmal vorbereiteter Filter: Glob kompiliert, Endungen als ext_id-Bitmap, Altersgrenzen absolut.
pub(crate) struct CompiledFilter<'f> {
    f: &'f Filter,
    name: NameMatch,
    /// Index = ext_id; leer = keine Endungsbedingung
    ext_ok: Vec<bool>,
    /// Zeit muss <= older_cut sein
    older_cut: Option<i64>,
    /// Zeit muss > newer_cut sein
    newer_cut: Option<i64>,
}

impl<'f> CompiledFilter<'f> {
    pub(crate) fn new(f: &'f Filter, tree: &Tree, now: FileTime) -> Self {
        let name = match f.name_glob.as_deref().map(str::trim) {
            None | Some("") => NameMatch::Any,
            Some(g) if g.contains(['*', '?', '[', '{']) => {
                match GlobBuilder::new(g)
                    .case_insensitive(true)
                    .literal_separator(false)
                    .build()
                {
                    Ok(gl) => NameMatch::Glob(gl.compile_matcher()),
                    Err(_) => NameMatch::None,
                }
            }
            Some(g) => NameMatch::Contains(g.to_lowercase()),
        };
        let ext_ok = if f.extensions.is_empty() {
            Vec::new()
        } else {
            let wanted: Vec<String> = f
                .extensions
                .iter()
                .map(|e| {
                    e.trim()
                        .trim_start_matches('*')
                        .trim_start_matches('.')
                        .to_lowercase()
                })
                .collect();
            tree.extensions
                .iter()
                .enumerate()
                .map(|(i, e)| i != 0 && wanted.iter().any(|w| w.as_str() == e.as_ref()))
                .collect()
        };
        CompiledFilter {
            f,
            name,
            ext_ok,
            older_cut: f
                .older_than_days
                .map(|d| now.0 - i64::from(d) * TICKS_PER_DAY),
            newer_cut: f
                .newer_than_days
                .map(|d| now.0 - i64::from(d) * TICKS_PER_DAY),
        }
    }

    pub(crate) fn matches(&self, tree: &Tree, id: NodeId) -> bool {
        let f = self.f;
        let n = &tree.nodes[id as usize];
        let is_dir = n.is_dir();
        if f.files_only && is_dir {
            return false;
        }
        if !f.include_hidden && n.flags.contains(NodeFlags::HIDDEN) {
            return false;
        }
        if f.min_size.is_some_and(|m| n.size < m) || f.max_size.is_some_and(|m| n.size > m) {
            return false;
        }
        if !f.extensions.is_empty()
            && (is_dir || !self.ext_ok.get(n.ext_id as usize).copied().unwrap_or(false))
        {
            return false;
        }
        if self.older_cut.is_some() || self.newer_cut.is_some() {
            let t = match f.age_field {
                AgeField::Modified => n.modified,
                AgeField::Accessed => n.accessed,
                AgeField::Created => n.created,
            }
            .0;
            if t == 0 {
                return false;
            }
            if self.older_cut.is_some_and(|c| t > c) || self.newer_cut.is_some_and(|c| t <= c) {
                return false;
            }
        }
        match &self.name {
            NameMatch::Any => true,
            NameMatch::None => false,
            NameMatch::Glob(g) => g.is_match(&*n.name),
            NameMatch::Contains(s) => {
                if n.name.is_ascii() {
                    contains_ascii_ci(n.name.as_bytes(), s.as_bytes())
                } else {
                    n.name.to_lowercase().contains(s.as_str())
                }
            }
        }
    }
}

/// Teilstring-Suche ohne Allocation; `needle` ist bereits kleingeschrieben.
fn contains_ascii_ci(hay: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() {
        return true;
    }
    if needle.len() > hay.len() {
        return false;
    }
    hay.windows(needle.len()).any(|w| {
        w.iter()
            .zip(needle)
            .all(|(a, b)| a.to_ascii_lowercase() == *b)
    })
}

/// Alle Nachfahren von `scope` (ohne `scope` selbst), die `f` erfüllen, in Baum-Reihenfolge
/// (Pre-Order). `limit == 0` bedeutet unbegrenzt.
pub fn search(tree: &Tree, scope: NodeId, f: &Filter, now: FileTime, limit: usize) -> Vec<NodeId> {
    let cf = CompiledFilter::new(f, tree, now);
    let mut out = Vec::new();
    for_each_descendant(tree, scope, |id| {
        if cf.matches(tree, id) {
            out.push(id);
            if limit != 0 && out.len() >= limit {
                return false;
            }
        }
        true
    });
    out
}
