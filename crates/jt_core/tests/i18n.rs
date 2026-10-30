// Joschys_Tree – Test: i18n-Vollständigkeit – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashSet;

use jt_core::i18n::{Key, Lang, all_keys, tr};

#[test]
fn every_key_has_nonempty_de_and_en() {
    let keys = all_keys();
    assert!(keys.len() > 100, "Key-Liste zu klein: {}", keys.len());
    for &k in keys {
        for lang in Lang::ALL {
            let t = tr(lang, k);
            assert!(!t.trim().is_empty(), "{k:?} ohne Text in {lang:?}");
        }
    }
}

#[test]
fn all_keys_has_no_duplicates() {
    let set: HashSet<Key> = all_keys().iter().copied().collect();
    assert_eq!(set.len(), all_keys().len());
}

#[test]
fn important_texts() {
    assert_eq!(tr(Lang::De, Key::DelConfirmWord), "LÖSCHEN");
    assert_eq!(tr(Lang::En, Key::DelConfirmWord), "DELETE");
    assert_eq!(tr(Lang::De, Key::ColSize), "Größe");
    assert!(tr(Lang::De, Key::AppTitle).contains("@ Joschy"));
    assert!(tr(Lang::En, Key::FooterBrand).contains("@ Joschy"));
    assert_eq!(tr(Lang::De, Key::FooterReadOnly), "Nur-Lesen");
    assert_eq!(Lang::default(), Lang::De);
}
