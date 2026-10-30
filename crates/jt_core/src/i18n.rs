// Joschys_Tree – Sprachen und zentrale Texttabelle de/en – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Lang {
    #[default]
    De,
    En,
}

/// Alle GUI-Texte. Auftrag D pflegt die Liste.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Key {
    AppTitle,
    MenuScan,
    ColName,
    ColSize,
}

const ALL_KEYS: &[Key] = &[Key::AppTitle, Key::MenuScan, Key::ColName, Key::ColSize];

/// Eine große match-Tabelle: Key => (de, en).
pub fn tr(lang: Lang, key: Key) -> &'static str {
    let (de, en) = match key {
        Key::AppTitle => ("Joschys_Tree @ Joschy", "Joschys_Tree @ Joschy"),
        Key::MenuScan => ("Scannen", "Scan"),
        Key::ColName => ("Name", "Name"),
        Key::ColSize => ("Größe", "Size"),
    };
    match lang {
        Lang::De => de,
        Lang::En => en,
    }
}

/// Für den Vollständigkeitstest.
pub fn all_keys() -> &'static [Key] {
    ALL_KEYS
}
