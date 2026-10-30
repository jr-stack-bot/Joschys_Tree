// Joschys_Tree – Formatierung von Bytes, Datum, Zahlen je Sprache – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::i18n::Lang;
use crate::model::FileTime;

/// Bytes -> "1,23 GiB"
pub fn bytes(n: u64, lang: Lang) -> String {
    let _ = (n, lang);
    todo!("Auftrag A: format::bytes")
}

pub fn time(t: FileTime, lang: Lang) -> String {
    let _ = (t, lang);
    todo!("Auftrag A: format::time")
}

pub fn number(n: u64, lang: Lang) -> String {
    let _ = (n, lang);
    todo!("Auftrag A: format::number")
}
