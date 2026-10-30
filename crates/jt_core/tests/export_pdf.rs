// Joschys_Tree – Test: PDF-Export (Kopf, Fuß @ Joschy, Seitenumbruch) – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

use jt_core::export::{ExportContent, ExportFormat, ExportRequest, export};
use jt_core::i18n::Lang;
use jt_core::stats::TypeStat;

fn pdf(rows: usize) -> Vec<u8> {
    let stats = (0..rows)
        .map(|i| TypeStat {
            ext: format!("ext{i}"),
            files: i as u64,
            size: (rows - i) as u64 * 1000,
            allocated: (rows - i) as u64 * 4096,
        })
        .collect();
    let req = ExportRequest {
        tree: None,
        content: ExportContent::FileTypes(stats),
        lang: Lang::De,
        title: "Dateitypen Größe".into(),
    };
    let mut buf = Vec::new();
    export(&req, ExportFormat::Pdf, &mut buf).unwrap();
    buf
}

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

fn page_count(buf: &[u8]) -> usize {
    let s = String::from_utf8_lossy(buf);
    s.matches("/Type /Page\n").count()
        + s.matches("/Type /Page ").count()
        + s.matches("/Type /Page/").count()
        + s.matches("/Type /Page>").count()
}

#[test]
fn pdf_header_branding_and_pages() {
    let buf = pdf(500);
    assert!(buf.starts_with(b"%PDF-"));
    assert!(contains(&buf, b"(@ Joschy)"), "Fuß @ Joschy fehlt");
    assert!(contains(&buf, b"/Helvetica"));
    assert!(contains(&buf, b"/WinAnsiEncoding"));
    assert!(contains(&buf, b"/MediaBox [0 0 842 595]"), "A4 quer");
    let pages = page_count(&buf);
    assert!(
        pages > 1,
        "500 Zeilen müssen mehrere Seiten ergeben, sind {pages}"
    );
    // "Seite n/m" auf der letzten Seite
    let last = format!("Seite {pages}/{pages}");
    assert!(contains(&buf, last.as_bytes()), "{last} fehlt");
}

#[test]
fn pdf_single_page_and_umlaut() {
    let buf = pdf(3);
    assert_eq!(page_count(&buf), 1);
    // "Größe" in WinAnsi: G r F6 DF e -> als Hex-String im Content-Stream
    assert!(contains(&buf, b"<4772f6df65>") || contains(&buf, b"<4772F6DF65>"));
}

#[test]
fn pdf_empty_table_has_one_page() {
    let buf = pdf(0);
    assert!(buf.starts_with(b"%PDF-"));
    assert_eq!(page_count(&buf), 1);
}
