// Joschys_Tree – Ansichten der Tabs und gemeinsame Helfer – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

use eframe::egui::{self, Ui};
use jt_core::protect::Protection;
use jt_core::{FileTime, Key, Lang, NO_NODE, Node, NodeId, Tree, tr};

use crate::app::{Action, AppState, Tab};
use crate::basket::Basket;
use crate::icons;
use crate::safety::{Decision, ProtCache};
use crate::theme::Theme;
use crate::widgets;

pub mod basket;
pub mod cleanup;
pub mod delta;
pub mod details;
pub mod duplicates;
pub mod file_types;
pub mod search;
pub mod top_files;
pub mod tree_view;
pub mod treemap;

const TABS: [(Tab, Key); 8] = [
    (Tab::Tree, Key::TabTree),
    (Tab::Treemap, Key::TabTreemap),
    (Tab::Cleanup, Key::TabCleanup),
    (Tab::TopFiles, Key::TabTopFiles),
    (Tab::FileTypes, Key::TabFileTypes),
    (Tab::Search, Key::TabSearch),
    (Tab::Duplicates, Key::TabDuplicates),
    (Tab::Delta, Key::TabDelta),
];

/// Tab-Leiste und aktiver Tab.
pub fn show(ui: &mut Ui, state: &mut AppState) {
    let lang = state.lang();
    ui.add_space(2.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 3.0;
        for (tab, key) in TABS {
            if widgets::tab_button(ui, state.tab == tab, tr(lang, key)).clicked() {
                state.tab = tab;
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let enabled = state.tree.is_some() && !state.file_dialog_busy();
            if widgets::tool_button(ui, tr(lang, Key::MenuExport), enabled).clicked() {
                state.actions.push(Action::OpenExport);
            }
            let label = fill_n(tr(lang, Key::BasketToggle), state.basket.len() as i64);
            let r =
                widgets::tool_button(ui, &label, true).on_hover_text(tr(lang, Key::BasketShortcut));
            let t = Theme::of(ui);
            let ir = egui::Rect::from_center_size(
                r.rect.left_center() - egui::vec2(9.0, 0.0),
                egui::vec2(13.0, 13.0),
            );
            let c = if state.settings.basket_open {
                t.accent
            } else {
                t.dim
            };
            icons::paint_basket(ui.painter(), ir, c);
            ui.add_space(14.0);
            if r.clicked() {
                state.actions.push(Action::ToggleBasketPanel);
            }
        });
    });
    let t = Theme::of(ui);
    let (r, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 7.0), egui::Sense::hover());
    ui.painter().hline(
        r.x_range(),
        r.center().y.round() + 0.5,
        egui::Stroke::new(1.0, t.line),
    );
    match state.tab {
        Tab::Tree => tree_view::show(ui, state),
        Tab::Treemap => treemap::show(ui, state),
        Tab::Cleanup => cleanup::show(ui, state),
        Tab::TopFiles => top_files::show(ui, state),
        Tab::FileTypes => file_types::show(ui, state),
        Tab::Search => search::show(ui, state),
        Tab::Duplicates => duplicates::show(ui, state),
        Tab::Delta => delta::show(ui, state),
    }
}

/// Hinweis, solange kein Scan geladen ist.
pub fn no_tree(ui: &mut Ui, lang: Lang) {
    ui.add_space(40.0);
    ui.vertical_centered(|ui| {
        let t = Theme::of(ui);
        ui.label(
            egui::RichText::new(tr(lang, Key::NoScanYet))
                .font(widgets::title_font(18.0))
                .color(t.text2),
        );
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

/// Tabellenzelle mit fester Textfarbe.
pub fn cell(ui: &mut Ui, text: impl Into<String>, color: egui::Color32) -> egui::Response {
    ui.add(egui::Label::new(egui::RichText::new(text).color(color)).selectable(false))
}

/// Gemeinsamer Kontext für Kontextmenü und Zeilenaktionen aller Ansichten.
pub struct MenuCx<'a> {
    pub lang: Lang,
    pub tree: &'a Tree,
    pub actions: &'a mut Vec<Action>,
    pub prot: &'a mut ProtCache,
    pub basket: &'a Basket,
    /// `app::can_delete()`
    pub admin: bool,
    /// `AppState::tree_is_live`
    pub live: bool,
}

/// Baut einen [`MenuCx`] aus dem `AppState` (getrennte Feld-Borrows).
#[macro_export]
macro_rules! menu_cx {
    ($state:expr, $tree:expr) => {
        $crate::views::MenuCx {
            lang: $state.settings.lang,
            tree: $tree,
            actions: &mut $state.actions,
            prot: &mut $state.prot,
            basket: &$state.basket,
            admin: $crate::app::can_delete(),
            live: $state.tree_is_live,
        }
    };
}

/// Aus welcher Ansicht das Menü kommt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuFrom {
    Tree,
    Treemap,
    List,
}

/// Rechtsklick-Menü für einen Knoten – EIN gemeinsamer Code für Baum, Treemap,
/// Top 100, Suche, Duplikate und Korb. Löschpunkte erscheinen nur als Admin im
/// Live-Scan (nie bei Snapshots) und sind nur aktiv, wenn
/// [`crate::safety::delete_decision`] zustimmt; sonst ausgegraut mit Begründung.
pub fn node_menu(ui: &mut Ui, cx: &mut MenuCx<'_>, id: NodeId, from: MenuFrom) {
    let lang = cx.lang;
    let tree = cx.tree;
    if (id as usize) >= tree.nodes.len() {
        return;
    }
    if tree.node(id).is_dir() && ui.button(tr(lang, Key::CtxShowInTreemap)).clicked() {
        cx.actions.push(Action::ShowInTreemap(id));
        ui.close();
    }
    if ui.button(tr(lang, Key::CtxOpenExplorer)).clicked() {
        cx.actions.push(Action::OpenExplorer(id));
        ui.close();
    }
    if ui.button(tr(lang, Key::CtxCopyPath)).clicked() {
        cx.actions.push(Action::CopyPath(id));
        ui.close();
    }
    if from != MenuFrom::Tree && ui.button(tr(lang, Key::CtxShowInTree)).clicked() {
        cx.actions.push(Action::Reveal(id));
        ui.close();
    }
    ui.separator();
    if cx.basket.contains(id) {
        if ui.button(tr(lang, Key::CtxRemoveBasket)).clicked() {
            cx.actions.push(Action::RemoveBasket(id));
            ui.close();
        }
    } else if cx.basket.covered_by_ancestor(tree, id) {
        ui.add_enabled(false, egui::Button::new(tr(lang, Key::CtxAddBasket)))
            .on_disabled_hover_text(tr(lang, Key::BasketCovered));
    } else {
        let d = cx.prot.basket_decision(tree, id);
        let r = ui.add_enabled(
            d.is_allowed(),
            egui::Button::new(tr(lang, Key::CtxAddBasket)),
        );
        let r = hint_for(r, d, lang);
        if r.clicked() {
            cx.actions.push(Action::AddBasket(vec![id]));
            ui.close();
        }
    }
    if cx.admin && cx.live && id != tree.root() {
        ui.separator();
        let d = cx.prot.decision(tree, id, cx.admin, cx.live);
        for (key, permanent) in [(Key::CtxRecycle, false), (Key::CtxDeletePermanently, true)] {
            let r = ui.add_enabled(d.is_allowed(), egui::Button::new(tr(lang, key)));
            if hint_for(r, d, lang).clicked() {
                cx.actions.push(Action::AskDelete { id, permanent });
                ui.close();
            }
        }
    }
}

/// Hover-Hinweis zu einer Entscheidung (gesperrt: Begründung, Vorsicht: Warnung).
fn hint_for(r: egui::Response, d: Decision, lang: Lang) -> egui::Response {
    match d {
        Decision::Denied(x) => r.on_disabled_hover_text(x.text(lang)),
        Decision::NeedsAck(reason) => r.on_hover_text(format!(
            "{}: {}",
            tr(lang, Key::ProtLevelCaution),
            reason.text(lang)
        )),
        Decision::Allowed => r,
    }
}

/// Gemeinsame Zeilenauswertung: Klick wählt aus, Strg+Klick legt in den Korb bzw.
/// nimmt heraus, Doppelklick zeigt im Baum, Rechtsklick öffnet das Menü.
pub fn row_interaction(resp: &egui::Response, cx: &mut MenuCx<'_>, id: NodeId) {
    if resp.clicked() {
        if resp.ctx.input(|i| i.modifiers.command) {
            cx.actions.push(Action::ToggleBasket(id));
        } else {
            cx.actions.push(Action::Select(id));
        }
    }
    if resp.double_clicked() {
        cx.actions.push(Action::Reveal(id));
    }
    resp.context_menu(|ui| node_menu(ui, cx, id, MenuFrom::List));
}

/// Kleines Schutz-Symbol (Schloss bzw. Warndreieck) mit Begründung als Tooltip;
/// liegt das Element im Korb, ein Korb-Symbol. Belegt immer gleich viel Platz.
pub fn prot_badge(ui: &mut Ui, prot: Protection, in_basket: bool, lang: Lang) {
    let t = Theme::of(ui);
    let (r, resp) = ui.allocate_exact_size(egui::vec2(13.0, 16.0), egui::Sense::hover());
    let ir = egui::Rect::from_center_size(r.center(), egui::vec2(11.0, 11.0));
    match prot {
        Protection::Blocked(reason) => {
            icons::paint_padlock(ui.painter(), ir, lock_color(t));
            resp.on_hover_text(prot_text(prot, reason, lang));
        }
        Protection::Caution(reason) => {
            icons::paint_warning(ui.painter(), ir, t.warn);
            resp.on_hover_text(prot_text(prot, reason, lang));
        }
        Protection::Free if in_basket => {
            icons::paint_basket(ui.painter(), ir, t.accent);
        }
        Protection::Free => {}
    }
}

/// Farbe des Schloss-Symbols (rot-grau).
pub fn lock_color(t: &Theme) -> egui::Color32 {
    crate::theme::lerp_color(t.text2, t.red, 0.45)
}

/// "Geschützt: <Begründung>"
pub fn prot_text(prot: Protection, reason: jt_core::protect::Reason, lang: Lang) -> String {
    format!("{}: {}", tr(lang, prot.level_key()), reason.text(lang))
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
