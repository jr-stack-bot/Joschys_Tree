// Joschys_Tree – Einstellungen (Sprache, Theme, letzte Ziele) als JSON – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

use jt_core::Lang;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub lang: Lang,
    pub dark: bool,
    pub recent_targets: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            lang: Lang::De,
            dark: true,
            recent_targets: Vec::new(),
        }
    }
}

impl Settings {
    /// Lädt die Einstellungen; bei Fehler Standardwerte (Auftrag E implementiert Pfad/JSON).
    pub fn load() -> Self {
        Settings::default()
    }

    #[allow(dead_code)]
    pub fn save(&self) -> std::io::Result<()> {
        todo!("Auftrag E: Settings::save")
    }
}
