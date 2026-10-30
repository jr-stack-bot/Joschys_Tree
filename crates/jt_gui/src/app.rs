// Joschys_Tree – Anwendungszustand, Tabs, Hintergrund-Jobs – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

use eframe::egui;
use jt_core::Tree;

use crate::{footer, settings::Settings, theme, views};

/// Tabs der Hauptansicht.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Tab {
    #[default]
    Tree,
    Treemap,
    TopFiles,
    FileTypes,
    Search,
    Duplicates,
    Delta,
}

/// Lösch-Regel an genau einer Stelle: nur als Admin.
pub fn can_delete() -> bool {
    jt_core::platform::is_admin()
}

#[derive(Default)]
pub struct AppState {
    pub tree: Option<Tree>,
    pub tab: Tab,
    pub status: String,
}

pub struct JtApp {
    pub state: AppState,
    pub settings: Settings,
}

impl JtApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let settings = Settings::load();
        theme::apply(&cc.egui_ctx, settings.dark);
        JtApp {
            state: AppState::default(),
            settings,
        }
    }
}

impl eframe::App for JtApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::bottom("jt_footer").show(ui, |ui| {
            footer::show(ui, &self.state);
        });
        egui::CentralPanel::default().show(ui, |ui| {
            views::show(ui, &mut self.state, &self.settings);
        });
    }
}
