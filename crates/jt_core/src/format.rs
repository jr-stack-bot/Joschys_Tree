// Joschys_Tree – Formatierung von Bytes, Datum, Zahlen je Sprache – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

use chrono::{DateTime, Local};

use crate::i18n::Lang;
use crate::model::FileTime;

const UNITS: [&str; 7] = ["B", "KiB", "MiB", "GiB", "TiB", "PiB", "EiB"];

fn dec_sep(lang: Lang) -> char {
    match lang {
        Lang::De => ',',
        Lang::En => '.',
    }
}

fn group_sep(lang: Lang) -> char {
    match lang {
        Lang::De => '.',
        Lang::En => ',',
    }
}

/// Bytes -> "1,23 GiB" (binäre Einheiten). Bis 3 signifikante Stellen, Nullen am Ende entfallen:
/// 1536 -> "1,5 KiB", 1024 -> "1 KiB", 999 -> "999 B", 12345678 -> "11,8 MiB".
pub fn bytes(n: u64, lang: Lang) -> String {
    if n < 1024 {
        return format!("{n} B");
    }
    let mut unit = 0;
    let mut v = n as f64;
    while v >= 1024.0 && unit < UNITS.len() - 1 {
        v /= 1024.0;
        unit += 1;
    }
    let mut decimals = if v < 10.0 {
        2
    } else if v < 100.0 {
        1
    } else {
        0
    };
    let mut s = format!("{v:.decimals$}");
    // Rundung kann 1024 erzeugen (z. B. 1023,9 -> "1024"): eine Einheit höher.
    if s.starts_with("1024") && unit < UNITS.len() - 1 {
        unit += 1;
        v /= 1024.0;
        decimals = 2;
        s = format!("{v:.decimals$}");
    }
    if decimals > 0 {
        let t = s.trim_end_matches('0').trim_end_matches('.');
        s.truncate(t.len());
    }
    let s = s.replace('.', &dec_sep(lang).to_string());
    format!("{s} {}", UNITS[unit])
}

/// Datum/Uhrzeit in lokaler Zeit: De "30.10.2026 14:05", En "2026-10-30 14:05". 0 -> "".
pub fn time(t: FileTime, lang: Lang) -> String {
    if t.0 == 0 {
        return String::new();
    }
    let Some(utc) = DateTime::from_timestamp(t.to_unix_secs(), 0) else {
        return String::new();
    };
    let local = utc.with_timezone(&Local);
    match lang {
        Lang::De => local.format("%d.%m.%Y %H:%M").to_string(),
        Lang::En => local.format("%Y-%m-%d %H:%M").to_string(),
    }
}

/// Ganzzahl mit Tausendertrennern: De "1.234.567", En "1,234,567".
pub fn number(n: u64, lang: Lang) -> String {
    let digits = n.to_string();
    let sep = group_sep(lang);
    let len = digits.len();
    let mut out = String::with_capacity(len + len / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (len - i).is_multiple_of(3) {
            out.push(sep);
        }
        out.push(c);
    }
    out
}
