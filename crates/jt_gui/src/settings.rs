// Joschys_Tree – Einstellungen (Sprache, Theme, letzte Ziele) als JSON – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::{Path, PathBuf};

use jt_core::Lang;
use serde::{Deserialize, Serialize};

/// Dateiname der Einstellungen.
const FILE_NAME: &str = "joschys_tree_settings.json";
/// Maximale Anzahl gemerkter Ziele.
const MAX_RECENT: usize = 10;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub lang: Lang,
    pub dark: bool,
    pub recent_targets: Vec<String>,
    /// MFT bevorzugen (nur wirksam als Admin auf lokalem NTFS).
    pub prefer_mft: bool,
    /// Mindestgröße für die Duplikat-Suche in Bytes.
    pub dup_min_size: u64,
    /// Maximale Tiefe beim Baum-Export.
    pub export_depth: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            lang: Lang::De,
            dark: true,
            recent_targets: Vec::new(),
            prefer_mft: true,
            dup_min_size: 1024 * 1024,
            export_depth: 3,
        }
    }
}

impl Settings {
    /// Speicherort: portabel neben der exe, falls dort schon eine Datei liegt,
    /// sonst %APPDATA%\Joschys_Tree bzw. $XDG_CONFIG_HOME/joschys_tree.
    pub fn default_path() -> Option<PathBuf> {
        if let Ok(exe) = std::env::current_exe()
            && let Some(dir) = exe.parent()
        {
            let portable = dir.join(FILE_NAME);
            if portable.is_file() {
                return Some(portable);
            }
        }
        config_dir().map(|d| d.join(FILE_NAME))
    }

    /// Lädt die Einstellungen; bei Fehler Standardwerte.
    pub fn load() -> Self {
        Self::default_path()
            .map(|p| Self::load_from(&p))
            .unwrap_or_default()
    }

    pub fn load_from(path: &Path) -> Self {
        std::fs::read(path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> std::io::Result<()> {
        match Self::default_path() {
            Some(p) => self.save_to(&p),
            None => Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "no config directory",
            )),
        }
    }

    pub fn save_to(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let json = serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(path, json)
    }

    /// Ziel vorne einreihen (ohne Duplikate, case-insensitiv).
    pub fn remember_target(&mut self, target: &str) {
        let t = target.trim();
        if t.is_empty() {
            return;
        }
        self.recent_targets.retain(|x| !x.eq_ignore_ascii_case(t));
        self.recent_targets.insert(0, t.to_string());
        self.recent_targets.truncate(MAX_RECENT);
    }
}

fn config_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join("Joschys_Tree"))
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
            .map(|d| d.join("joschys_tree"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_dark_german() {
        let s = Settings::default();
        assert!(s.dark);
        assert_eq!(s.lang, Lang::De);
    }

    #[test]
    fn roundtrip() {
        let dir = std::env::temp_dir().join(format!("jt_settings_test_{}", std::process::id()));
        let path = dir.join("sub").join(FILE_NAME);
        let mut s = Settings {
            lang: Lang::En,
            dark: false,
            dup_min_size: 42,
            ..Default::default()
        };
        s.remember_target("C:\\");
        s.remember_target("\\\\server\\share");
        s.remember_target("c:\\");
        s.save_to(&path).unwrap();
        let loaded = Settings::load_from(&path);
        assert_eq!(loaded, s);
        assert_eq!(loaded.recent_targets, vec!["c:\\", "\\\\server\\share"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn broken_file_gives_defaults() {
        let dir = std::env::temp_dir().join(format!("jt_settings_bad_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(FILE_NAME);
        std::fs::write(&path, b"{ kaputt").unwrap();
        assert_eq!(Settings::load_from(&path), Settings::default());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
