// Joschys_Tree – Filter-Ausdruck und Suche – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

use serde::{Deserialize, Serialize};

use crate::model::{FileTime, NodeId, Tree};

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum AgeField {
    #[default]
    Modified,
    Accessed,
    Created,
}

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct Filter {
    pub name_glob: Option<String>,
    pub min_size: Option<u64>,
    pub max_size: Option<u64>,
    pub older_than_days: Option<u32>,
    pub newer_than_days: Option<u32>,
    pub age_field: AgeField,
    pub extensions: Vec<String>,
    pub files_only: bool,
    pub include_hidden: bool,
}

impl Filter {
    pub fn matches(&self, tree: &Tree, id: NodeId, now: FileTime) -> bool {
        let _ = (tree, id, now);
        todo!("Auftrag A: Filter::matches")
    }
}

pub fn search(tree: &Tree, scope: NodeId, f: &Filter, now: FileTime, limit: usize) -> Vec<NodeId> {
    let _ = (tree, scope, f, now, limit);
    todo!("Auftrag A: search")
}
