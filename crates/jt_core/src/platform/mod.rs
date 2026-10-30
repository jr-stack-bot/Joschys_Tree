// Joschys_Tree – Plattform-Schicht: Admin, Löschen, Explorer, ACL, ADS, Volume – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

#[cfg(windows)]
mod win;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SecurityInfo {
    pub owner: String,
    pub aces: Vec<Ace>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ace {
    pub principal: String,
    pub allow: bool,
    pub rights: String,
    pub inherited: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StreamInfo {
    pub name: String,
    pub size: u64,
}

#[cfg(windows)]
pub use win::{
    alternate_streams, delete_permanently, is_admin, list_drives, long_path,
    move_to_recycle_bin, open_in_explorer, security_info, volume_info,
};

#[cfg(not(windows))]
pub use stub::{
    alternate_streams, delete_permanently, is_admin, list_drives, long_path,
    move_to_recycle_bin, open_in_explorer, security_info, volume_info,
};

#[cfg(not(windows))]
mod stub {
    use super::{SecurityInfo, StreamInfo};
    use std::io;

    fn unsupported<T>() -> io::Result<T> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "nur unter Windows verfügbar",
        ))
    }

    pub fn is_admin() -> bool {
        false
    }

    pub fn move_to_recycle_bin(_paths: &[String]) -> io::Result<()> {
        unsupported()
    }

    pub fn delete_permanently(_paths: &[String]) -> io::Result<()> {
        unsupported()
    }

    /// explorer /select,"<pfad>"
    pub fn open_in_explorer(_path: &str) -> io::Result<()> {
        unsupported()
    }

    pub fn security_info(_path: &str) -> io::Result<SecurityInfo> {
        unsupported()
    }

    pub fn alternate_streams(_path: &str) -> io::Result<Vec<StreamInfo>> {
        unsupported()
    }

    /// (Dateisystem, Clustergröße, Gesamt, Frei)
    pub fn volume_info(_root: &str) -> io::Result<(String, u32, u64, u64)> {
        unsupported()
    }

    /// Auf Nicht-Windows unverändert.
    pub fn long_path(path: &str) -> String {
        path.to_string()
    }

    pub fn list_drives() -> Vec<String> {
        Vec::new()
    }
}

