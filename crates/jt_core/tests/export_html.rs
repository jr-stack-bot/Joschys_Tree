// Joschys_Tree – Test: HTML-Export (Branding, Escaping, Umlaute) – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

use jt_core::export::{ExportContent, ExportFormat, ExportRequest, export};
use jt_core::i18n::Lang;
use jt_core::model::testutil::sample_tree;
use jt_core::stats::TypeStat;

fn html(ext: &str, title: &str) -> String {
    let req = ExportRequest {
        tree: None,
        content: ExportContent::FileTypes(vec![TypeStat {
            ext: ext.into(),
            files: 1,
            size: 10,
            allocated: 4096,
        }]),
        lang: Lang::De,
        title: title.into(),
    };
    let mut buf = Vec::new();
    export(&req, ExportFormat::Html, &mut buf).unwrap();
    String::from_utf8(buf).unwrap()
}

#[test]
fn html_branding_and_escaping() {
    let s = html("<script>alert(1)</script>", "Bericht <b>&</b>");
    assert!(s.starts_with("<!DOCTYPE html>"));
    assert!(s.contains("@ Joschy"));
    assert!(s.contains("Erstellt mit Joschys_Tree @ Joschy · "));
    assert!(!s.contains("<script>"), "unescaped script");
    assert!(s.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
    assert!(s.contains("Bericht &lt;b&gt;&amp;&lt;/b&gt;"));
    // keine externen Ressourcen
    assert!(!s.contains("src=") && !s.contains("href="));
}

#[test]
fn html_umlauts_unchanged() {
    let s = html("größe", "Übersicht");
    assert!(s.contains("Größe"));
    assert!(s.contains("größe"));
    assert!(s.contains("Übersicht"));
    assert!(s.contains("<meta charset=\"utf-8\">"));
}

#[test]
fn html_tree_export() {
    let tree = sample_tree();
    let req = ExportRequest {
        tree: Some(&tree),
        content: ExportContent::TopFiles(vec![8, 7, 3]),
        lang: Lang::En,
        title: String::new(),
    };
    let mut buf = Vec::new();
    export(&req, ExportFormat::Html, &mut buf).unwrap();
    let s = String::from_utf8(buf).unwrap();
    assert!(s.contains("lang=\"en\""));
    assert!(s.contains("pagefile.sys"));
    assert!(s.contains("48.83 KiB"), "50000 B formatiert");
    assert!(s.contains("Created with Joschys_Tree @ Joschy"));
}
