// Joschys_Tree – Test: CSV-Export (BOM, Quoting, Trennzeichen, Umlaute) – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

use jt_core::export::{ExportContent, ExportFormat, ExportRequest, export};
use jt_core::i18n::Lang;
use jt_core::model::testutil::sample_tree;
use jt_core::stats::TypeStat;

fn types(ext: &str) -> ExportContent {
    ExportContent::FileTypes(vec![TypeStat {
        ext: ext.to_string(),
        files: 3,
        size: 1536,
        allocated: 4096,
    }])
}

fn run(content: ExportContent, lang: Lang) -> String {
    let req = ExportRequest {
        tree: None,
        content,
        lang,
        title: "Test".into(),
    };
    let mut buf = Vec::new();
    export(&req, ExportFormat::Csv, &mut buf).unwrap();
    assert!(buf.starts_with(b"\xEF\xBB\xBF"), "BOM fehlt");
    String::from_utf8(buf[3..].to_vec()).unwrap()
}

#[test]
fn csv_quotes_and_bom_de() {
    let text = run(types("a;\"b"), Lang::De);
    let mut lines = text.split("\r\n");
    let head = lines.next().unwrap();
    assert!(head.contains(';'), "Trennzeichen de muss ; sein: {head}");
    let row = lines.next().unwrap();
    assert!(row.starts_with("\"a;\"\"b\";3;1536;4096;"), "Zeile: {row}");
}

#[test]
fn csv_comma_en() {
    let text = run(types("x,y"), Lang::En);
    let row = text.split("\r\n").nth(1).unwrap();
    assert!(
        row.starts_with("\"x,y\",3,1536,4096,100.00"),
        "Zeile: {row}"
    );
}

#[test]
fn csv_umlauts_unchanged() {
    let text = run(types("größe"), Lang::De);
    assert!(text.contains("Größe"), "Kopf: {text}");
    assert!(text.contains("größe"));
}

#[test]
fn csv_tree_export() {
    let tree = sample_tree();
    let req = ExportRequest {
        tree: Some(&tree),
        content: ExportContent::Tree {
            root: 0,
            max_depth: 10,
        },
        lang: Lang::De,
        title: String::new(),
    };
    let mut buf = Vec::new();
    export(&req, ExportFormat::Csv, &mut buf).unwrap();
    let text = String::from_utf8(buf[3..].to_vec()).unwrap();
    // Kopf + 9 Knoten
    assert_eq!(text.trim_end().split("\r\n").count(), 10);
    assert!(text.contains("C:\\Users\\joschy\\Bild.JPG"));
    // Wurzel zuerst, danach größtes Kind (pagefile.sys)
    let second = text.split("\r\n").nth(2).unwrap();
    assert!(second.starts_with("C:\\pagefile.sys;"), "{second}");
}

#[test]
fn missing_tree_is_error() {
    let req = ExportRequest {
        tree: None,
        content: ExportContent::TopFiles(vec![1]),
        lang: Lang::De,
        title: String::new(),
    };
    let mut buf = Vec::new();
    assert!(export(&req, ExportFormat::Csv, &mut buf).is_err());
}
