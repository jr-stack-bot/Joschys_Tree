// Joschys_Tree – Windows-Plattformfunktionen (Admin, Papierkorb, ACL, ADS, lange Pfade) – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::io;

use super::{SecurityInfo, StreamInfo};

fn not_yet<T>() -> io::Result<T> {
    Err(io::Error::new(io::ErrorKind::Unsupported, "noch nicht implementiert"))
}

pub fn is_admin() -> bool {
    false
}

pub fn move_to_recycle_bin(_paths: &[String]) -> io::Result<()> {
    not_yet()
}

pub fn delete_permanently(_paths: &[String]) -> io::Result<()> {
    not_yet()
}

/// explorer /select,"<pfad>"
pub fn open_in_explorer(_path: &str) -> io::Result<()> {
    not_yet()
}

pub fn security_info(_path: &str) -> io::Result<SecurityInfo> {
    not_yet()
}

pub fn alternate_streams(_path: &str) -> io::Result<Vec<StreamInfo>> {
    not_yet()
}

/// (Dateisystem, Clustergröße, Gesamt, Frei)
pub fn volume_info(_root: &str) -> io::Result<(String, u32, u64, u64)> {
    not_yet()
}

/// "\\?\C:\..." bzw. "\\?\UNC\server\share\..."
pub fn long_path(path: &str) -> String {
    path.to_string()
}

pub fn list_drives() -> Vec<String> {
    Vec::new()
}
