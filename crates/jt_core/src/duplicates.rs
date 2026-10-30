// Joschys_Tree – Duplikat-Suche (Größe, Teil-Hash, Voll-Hash xxh3-128) – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::io;
use std::sync::atomic::AtomicBool;

use crate::model::{NodeId, Tree};

pub trait FileReader: Sync {
    fn read_head(&self, path: &str, n: usize) -> io::Result<Vec<u8>>;
    fn hash_full(&self, path: &str, cancel: &AtomicBool) -> io::Result<u128>;
}

/// std::fs + xxh3
pub struct StdFileReader;

impl FileReader for StdFileReader {
    fn read_head(&self, path: &str, n: usize) -> io::Result<Vec<u8>> {
        let _ = (path, n);
        todo!("Auftrag C: StdFileReader::read_head")
    }

    fn hash_full(&self, path: &str, cancel: &AtomicBool) -> io::Result<u128> {
        let _ = (path, cancel);
        todo!("Auftrag C: StdFileReader::hash_full")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DupGroup {
    pub size: u64,
    pub hash: u128,
    pub files: Vec<NodeId>,
}

/// Nach Einsparpotenzial sortiert.
pub fn find_duplicates(
    tree: &Tree,
    scope: NodeId,
    min_size: u64,
    reader: &dyn FileReader,
    progress: &dyn Fn(u64, u64),
    cancel: &AtomicBool,
) -> Vec<DupGroup> {
    let _ = (tree, scope, min_size, reader, progress, cancel);
    todo!("Auftrag C: find_duplicates")
}
