// Joschys_Tree – Nur Debug-Build: Start-Hook für Bildschirmfotos und Sichtprüfung – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later
//
// Existiert ausschließlich unter #[cfg(debug_assertions)] (siehe main.rs), nie im Release.
// JT_DEBUG_SCAN=<Pfad>   startet beim Programmstart einen Scan dieses Ziels
// JT_DEBUG_TAB=<Name>    tree | treemap | top | types | search | dups | delta
// JT_DEBUG_THEME=<Name>  dark | light
// JT_DEBUG_LANG=<Code>   de | en

use jt_core::aggregate::{SortKey, sorted_children};
use jt_core::{Lang, Tree};

use crate::app::{Action, AppState, Tab};

fn var(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.trim().is_empty())
}

/// Beim Start: Theme, Sprache, Tab und Scan-Ziel aus Umgebungsvariablen.
pub fn on_start(state: &mut AppState) {
    match var("JT_DEBUG_THEME").as_deref() {
        Some("light") => state.settings.dark = false,
        Some("dark") => state.settings.dark = true,
        _ => {}
    }
    match var("JT_DEBUG_LANG").as_deref() {
        Some("en") => state.settings.lang = Lang::En,
        Some("de") => state.settings.lang = Lang::De,
        _ => {}
    }
    if let Some(tab) = var("JT_DEBUG_TAB") {
        state.tab = match tab.as_str() {
            "treemap" => Tab::Treemap,
            "top" => Tab::TopFiles,
            "types" => Tab::FileTypes,
            "search" => Tab::Search,
            "dups" => Tab::Duplicates,
            "delta" => Tab::Delta,
            _ => Tab::Tree,
        };
    }
    if let Some(target) = var("JT_DEBUG_SCAN") {
        state.target_input = target;
        state.actions.push(Action::StartScan);
    }
}

/// Nach dem Scan: zwei Ebenen der größten Ordner aufklappen und einen Eintrag wählen.
pub fn after_scan(state: &mut AppState, tree: &Tree) {
    if var("JT_DEBUG_SCAN").is_none() {
        return;
    }
    let mut cur = tree.root();
    let mut pick = None;
    for _ in 0..2 {
        let kids = sorted_children(tree, cur, SortKey::Size, true);
        pick = kids.get(1).or(kids.first()).copied();
        let Some(dir) = kids.into_iter().find(|&k| tree.node(k).is_dir()) else {
            break;
        };
        state.tree_view.expanded.insert(dir);
        cur = dir;
    }
    if let Some(p) = pick {
        state.selected = Some(p);
        if state.tab == Tab::Treemap {
            state.selected = None;
        }
    }
    state.tree_view.invalidate();
}
