// Joschys_Tree – Bibliothekswurzel: Modulliste, Re-Exports, Branding – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Joschys_Tree Kernbibliothek (ohne GUI). @ Joschy

pub mod aggregate;
pub mod delta;
pub mod duplicates;
pub mod export;
pub mod filter;
pub mod format;
pub mod history;
pub mod i18n;
pub mod model;
pub mod platform;
pub mod scan;
pub mod snapshot;
pub mod stats;

pub use i18n::{Key, Lang, tr};
pub use model::{
    BRANDING, ChildIter, EntryInfo, FileTime, NO_NODE, Node, NodeFlags, NodeId, ScanMeta, ScanMode,
    Tree, TreeBuilder,
};
