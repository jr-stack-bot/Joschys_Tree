// Joschys_Tree – Ansichten der Tabs und gemeinsame Helfer – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

use eframe::egui::{self, Align2, FontId, Sense, Ui, vec2};
use jt_core::{FileTime, Key, Lang, NO_NODE, Node, NodeId, Tree, tr};

use crate::app::{Action, AppState, Tab};
use crate::theme;

pub mod delta;
pub mod details;
pub mod duplicates;
pub mod file_types;
pub mod search;
pub mod top_files;
pub mod tree_view;
pub mod treemap;

const TABS: [(Tab, Key); 7] = [
    (Tab::Tree, Key::TabTree),
    (Tab::Treemap, Key::TabTreemap),
    (Tab::TopFiles, Key::TabTopFiles),
    (Tab::FileTypes, Key::TabFileTypes),
    (Tab::Search, Key::TabSearch),
    (Tab::Duplicates, Key::TabDuplicates),
    (Tab::Delta, Key::TabDelta),
];

/// Tab-Leiste und aktiver Tab.
pub fn show(ui: &mut Ui, state: &mut AppState) {
    let lang = state.lang();
    ui.horizontal(|ui| {
        for (tab, key) in TABS {
            ui.selectable_value(&mut state.tab, tab, tr(lang, key));
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let enabled = state.tree.is_some() && !state.file_dialog_busy();
            if ui
                .add_enabled(enabled, egui::Button::new(tr(lang, Key::MenuExport)))
                .clicked()
            {
                state.actions.push(Action::OpenExport);
            }
        });
    });
    ui.separator();
    match state.tab {
        Tab::Tree => tree_view::show(ui, state),
        Tab::Treemap => treemap::show(ui, state),
        Tab::TopFiles => top_files::show(ui, state),
        Tab::FileTypes => file_types::show(ui, state),
        Tab::Search => search::show(ui, state),
        Tab::Duplicates => duplicates::show(ui, state),
        Tab::Delta => delta::show(ui, state),
    }
}

/// Hinweis, solange kein Scan geladen ist.
pub fn no_tree(ui: &mut Ui, lang: Lang) {
    ui.add_space(24.0);
    ui.vertical_centered(|ui| {
        ui.heading(tr(lang, Key::NoScanYet));
    });
}

/// Anteil 0..1 -> "12,3 %" (de) bzw. "12.3 %" (en).
pub fn fmt_percent(frac: f32, lang: Lang) -> String {
    let s = format!("{:.1} %", frac * 100.0);
    match lang {
        Lang::De => s.replace('.', ","),
        Lang::En => s,
    }
}

/// Vorzeichenbehaftete Größe, z. B. "+1,5 KiB".
pub fn fmt_signed_bytes(delta: i64, lang: Lang) -> String {
    let sign = if delta < 0 { "-" } else { "+" };
    format!(
        "{sign}{}",
        jt_core::format::bytes(delta.unsigned_abs(), lang)
    )
}

/// Zeitpunkt oder Strich, wenn unbekannt.
pub fn fmt_time(t: FileTime, lang: Lang) -> String {
    if t.0 == 0 {
        "–".to_string()
    } else {
        jt_core::format::time(t, lang)
    }
}

/// Anteil eines Knotens an seinem Elternordner (0..1).
pub fn frac_of_parent(tree: &Tree, id: NodeId) -> f32 {
    let n = tree.node(id);
    if n.parent == NO_NODE {
        return 1.0;
    }
    let p = tree.node(n.parent);
    if p.size == 0 {
        0.0
    } else {
        (n.size as f64 / p.size as f64) as f32
    }
}

/// Text mit Platzhalter `{n}` füllen.
pub fn fill_n(template: &str, n: i64) -> String {
    template.replace("{n}", &n.to_string())
}

/// Alter in Tagen seit der letzten Änderung.
pub fn age_days(n: &Node, now: FileTime) -> Option<i64> {
    if n.modified.0 == 0 {
        return None;
    }
    Some(((now.to_unix_secs() - n.modified.to_unix_secs()) / 86_400).max(0))
}

/// Prozent-Balken mit Text in einer Tabellenzelle.
pub fn percent_bar(ui: &mut Ui, frac: f32, lang: Lang) {
    let frac = frac.clamp(0.0, 1.0);
    let h = ui.available_height().clamp(10.0, 16.0);
    let w = ui.available_width().max(20.0);
    let (rect, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
    let vis = ui.visuals();
    let painter = ui.painter();
    painter.rect_filled(rect, 2.0, vis.extreme_bg_color);
    let mut fill = rect;
    fill.set_width(rect.width() * frac);
    painter.rect_filled(fill, 2.0, theme::bar_color(vis.dark_mode));
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        fmt_percent(frac, lang),
        FontId::proportional(11.0),
        vis.text_color(),
    );
}

/// Rechtsklick-Menü für einen Knoten. Löschpunkte nur, wenn `allow_delete`
/// (= `AppState::deletion_allowed()`: Admin und Live-Scan, kein Snapshot).
pub fn node_menu(
    ui: &mut Ui,
    lang: Lang,
    tree: &Tree,
    id: NodeId,
    actions: &mut Vec<Action>,
    with_reveal: bool,
    allow_delete: bool,
) {
    if ui.button(tr(lang, Key::CtxOpenExplorer)).clicked() {
        actions.push(Action::OpenExplorer(id));
        ui.close();
    }
    if ui.button(tr(lang, Key::CtxCopyPath)).clicked() {
        actions.push(Action::CopyPath(id));
        ui.close();
    }
    if with_reveal && ui.button(tr(lang, Key::CtxShowInTree)).clicked() {
        actions.push(Action::Reveal(id));
        ui.close();
    }
    if allow_delete && id != tree.root() {
        ui.separator();
        if ui.button(tr(lang, Key::CtxRecycle)).clicked() {
            actions.push(Action::AskDelete {
                id,
                permanent: false,
            });
            ui.close();
        }
        if ui.button(tr(lang, Key::CtxDeletePermanently)).clicked() {
            actions.push(Action::AskDelete {
                id,
                permanent: true,
            });
            ui.close();
        }
    }
}

/// Gemeinsame Zeilenauswertung: Klick wählt aus, Doppelklick zeigt im Baum, Rechtsklick-Menü.
pub fn row_interaction(
    resp: &egui::Response,
    lang: Lang,
    tree: &Tree,
    id: NodeId,
    actions: &mut Vec<Action>,
    allow_delete: bool,
) {
    if resp.clicked() {
        actions.push(Action::Select(id));
    }
    if resp.double_clicked() {
        actions.push(Action::Reveal(id));
    }
    resp.context_menu(|ui| node_menu(ui, lang, tree, id, actions, true, allow_delete));
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    /// Liefert alle String-Literale aus Rust-Quelltext (ohne Kommentare und ohne Testmodul).
    fn string_literals(src: &str) -> Vec<String> {
        let code = match src.find("#[cfg(test)]") {
            Some(p) => &src[..p],
            None => src,
        };
        let mut out = Vec::new();
        let chars: Vec<char> = code.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            if c == '/' && chars.get(i + 1) == Some(&'/') {
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
            } else if c == '\'' && chars.get(i + 2) == Some(&'\'') {
                i += 3; // Zeichenliteral wie 'x'
            } else if c == '\'' && chars.get(i + 1) == Some(&'\\') {
                i += 2;
                while i < chars.len() && chars[i] != '\'' {
                    i += 1;
                }
                i += 1;
            } else if c == '"' {
                let mut s = String::new();
                i += 1;
                while i < chars.len() && chars[i] != '"' {
                    if chars[i] == '\\' {
                        i += 1;
                    }
                    if i < chars.len() {
                        s.push(chars[i]);
                    }
                    i += 1;
                }
                i += 1;
                out.push(s);
            } else {
                i += 1;
            }
        }
        out
    }

    /// Erlaubt: Formatstrings ohne Buchstaben außerhalb von {..} und interne IDs "jt_...".
    fn allowed(lit: &str) -> bool {
        if lit.starts_with("jt_")
            && lit
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        {
            return true;
        }
        let mut depth = 0;
        for c in lit.chars() {
            match c {
                '{' => depth += 1,
                '}' => depth -= 1,
                c if depth == 0 && c.is_alphabetic() => return false,
                _ => {}
            }
        }
        true
    }

    #[test]
    fn literal_scanner_works() {
        let lits = string_literals("let a = \"Hallo Welt\"; // \"Kommentar\"\nlet b = '\"';");
        assert_eq!(lits, vec!["Hallo Welt".to_string()]);
        assert!(!allowed("Hallo Welt"));
        assert!(!allowed("Size: {}"));
        assert!(allowed("{} ({})"));
        assert!(allowed("{n} – {:.1} %"));
        assert!(allowed("jt_tree_table"));
    }

    #[test]
    fn no_untranslated_literals_in_views() {
        let src_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files: Vec<_> = std::fs::read_dir(src_dir.join("views"))
            .unwrap()
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|e| e == "rs"))
            .collect();
        files.push(src_dir.join("footer.rs"));
        assert!(files.len() >= 10);
        let mut bad = Vec::new();
        for f in files {
            let src = std::fs::read_to_string(&f).unwrap();
            for lit in string_literals(&src) {
                if !allowed(&lit) {
                    bad.push(format!("{}: {lit:?}", f.display()));
                }
            }
        }
        assert!(bad.is_empty(), "Literale ohne tr(): {bad:#?}");
    }
}
