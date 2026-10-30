// Joschys_Tree – Test: XLSX-Export (ZIP-Struktur, Formate, Umlaute) – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Liest das ZIP ohne zusätzliche Abhängigkeit: Central Directory + kleiner Inflate.

use jt_core::export::{ExportContent, ExportFormat, ExportRequest, export};
use jt_core::i18n::Lang;
use jt_core::model::testutil::sample_tree;
use jt_core::stats::TypeStat;

fn u16le(b: &[u8], o: usize) -> usize {
    u16::from_le_bytes([b[o], b[o + 1]]) as usize
}
fn u32le(b: &[u8], o: usize) -> usize {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]) as usize
}

/// (Name, Methode, komprimierte Daten)
fn zip_entries(z: &[u8]) -> Vec<(String, usize, Vec<u8>)> {
    let eocd = (0..=z.len() - 22)
        .rev()
        .find(|&i| z[i..i + 4] == [0x50, 0x4b, 0x05, 0x06])
        .expect("EOCD");
    let count = u16le(z, eocd + 10);
    let mut p = u32le(z, eocd + 16);
    let mut out = Vec::new();
    for _ in 0..count {
        assert_eq!(&z[p..p + 4], &[0x50, 0x4b, 0x01, 0x02]);
        let method = u16le(z, p + 10);
        let csize = u32le(z, p + 20);
        let nlen = u16le(z, p + 28);
        let elen = u16le(z, p + 30);
        let clen = u16le(z, p + 32);
        let lho = u32le(z, p + 42);
        let name = String::from_utf8(z[p + 46..p + 46 + nlen].to_vec()).unwrap();
        let lnlen = u16le(z, lho + 26);
        let lelen = u16le(z, lho + 28);
        let start = lho + 30 + lnlen + lelen;
        out.push((name, method, z[start..start + csize].to_vec()));
        p += 46 + nlen + elen + clen;
    }
    out
}

// ---- Minimaler Inflate (RFC 1951) ----
struct Bits<'a> {
    d: &'a [u8],
    pos: usize,
}
impl Bits<'_> {
    fn bit(&mut self) -> u32 {
        let b = (self.d[self.pos >> 3] >> (self.pos & 7)) & 1;
        self.pos += 1;
        b as u32
    }
    fn bits(&mut self, n: u32) -> u32 {
        (0..n).fold(0, |acc, i| acc | (self.bit() << i))
    }
}
struct Huff {
    counts: [u16; 16],
    symbols: Vec<u16>,
}
impl Huff {
    fn new(lengths: &[u8]) -> Huff {
        let mut counts = [0u16; 16];
        for &l in lengths {
            counts[l as usize] += 1;
        }
        counts[0] = 0;
        let mut offs = [0u16; 16];
        for i in 1..16 {
            offs[i] = offs[i - 1] + counts[i - 1];
        }
        let mut symbols = vec![0u16; lengths.len()];
        for (s, &l) in lengths.iter().enumerate() {
            if l != 0 {
                symbols[offs[l as usize] as usize] = s as u16;
                offs[l as usize] += 1;
            }
        }
        Huff { counts, symbols }
    }
    fn decode(&self, b: &mut Bits) -> u16 {
        let (mut code, mut first, mut index) = (0i32, 0i32, 0i32);
        for len in 1..16 {
            code |= b.bit() as i32;
            let count = self.counts[len] as i32;
            if code - count < first {
                return self.symbols[(index + (code - first)) as usize];
            }
            index += count;
            first += count;
            first <<= 1;
            code <<= 1;
        }
        panic!("ungültiger Huffman-Code");
    }
}
const LBASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
const LEXT: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const DBASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DEXT: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];

fn inflate(data: &[u8]) -> Vec<u8> {
    let mut b = Bits { d: data, pos: 0 };
    let mut out: Vec<u8> = Vec::new();
    loop {
        let last = b.bit();
        match b.bits(2) {
            0 => {
                b.pos = (b.pos + 7) & !7;
                let o = b.pos / 8;
                let len = u16le(data, o);
                out.extend_from_slice(&data[o + 4..o + 4 + len]);
                b.pos = (o + 4 + len) * 8;
            }
            t @ (1 | 2) => {
                let (lit, dist) = if t == 1 {
                    let mut l = [0u8; 288];
                    l[..144].fill(8);
                    l[144..256].fill(9);
                    l[256..280].fill(7);
                    l[280..].fill(8);
                    (Huff::new(&l), Huff::new(&[5u8; 30]))
                } else {
                    let hlit = b.bits(5) as usize + 257;
                    let hdist = b.bits(5) as usize + 1;
                    let hclen = b.bits(4) as usize + 4;
                    const ORD: [usize; 19] = [
                        16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
                    ];
                    let mut cl = [0u8; 19];
                    for &o in ORD.iter().take(hclen) {
                        cl[o] = b.bits(3) as u8;
                    }
                    let clh = Huff::new(&cl);
                    let mut lens = Vec::with_capacity(hlit + hdist);
                    while lens.len() < hlit + hdist {
                        match clh.decode(&mut b) {
                            s @ 0..=15 => lens.push(s as u8),
                            16 => {
                                let prev = *lens.last().unwrap();
                                for _ in 0..3 + b.bits(2) {
                                    lens.push(prev);
                                }
                            }
                            17 => lens.extend(std::iter::repeat_n(0, 3 + b.bits(3) as usize)),
                            _ => lens.extend(std::iter::repeat_n(0, 11 + b.bits(7) as usize)),
                        }
                    }
                    (Huff::new(&lens[..hlit]), Huff::new(&lens[hlit..]))
                };
                loop {
                    let s = lit.decode(&mut b) as usize;
                    if s < 256 {
                        out.push(s as u8);
                    } else if s == 256 {
                        break;
                    } else {
                        let i = s - 257;
                        let len = LBASE[i] as usize + b.bits(LEXT[i] as u32) as usize;
                        let di = dist.decode(&mut b) as usize;
                        let d = DBASE[di] as usize + b.bits(DEXT[di] as u32) as usize;
                        let start = out.len() - d;
                        for k in 0..len {
                            out.push(out[start + k]);
                        }
                    }
                }
            }
            _ => panic!("ungültiger Blocktyp"),
        }
        if last == 1 {
            return out;
        }
    }
}

fn file(entries: &[(String, usize, Vec<u8>)], name: &str) -> String {
    let (_, method, data) = entries.iter().find(|e| e.0 == name).expect(name);
    let raw = match method {
        0 => data.clone(),
        8 => inflate(data),
        m => panic!("Methode {m}"),
    };
    String::from_utf8(raw).unwrap()
}

fn xlsx(content: ExportContent, tree: Option<&jt_core::Tree>) -> Vec<u8> {
    let req = ExportRequest {
        tree,
        content,
        lang: Lang::De,
        title: "Größe & Co".into(),
    };
    let mut buf = Vec::new();
    export(&req, ExportFormat::Xlsx, &mut buf).unwrap();
    buf
}

#[test]
fn xlsx_is_zip_with_sheet() {
    let buf = xlsx(
        ExportContent::FileTypes(vec![TypeStat {
            ext: "größe".into(),
            files: 2,
            size: 1536,
            allocated: 8192,
        }]),
        None,
    );
    assert!(buf.starts_with(b"PK"));
    assert!(!buf.is_empty());
    let entries = zip_entries(&buf);
    assert!(
        entries.iter().any(|e| e.0 == "xl/worksheets/sheet1.xml"),
        "{:?}",
        entries.iter().map(|e| &e.0).collect::<Vec<_>>()
    );
    let sheet = file(&entries, "xl/worksheets/sheet1.xml");
    assert!(
        sheet.contains("<autoFilter ref=\"A1:E2\""),
        "AutoFilter fehlt"
    );
    assert!(
        sheet.contains("state=\"frozen\""),
        "fixierte Kopfzeile fehlt"
    );
    assert!(sheet.contains("<v>1536</v>"), "Bytes als Zahl");
    let shared = file(&entries, "xl/sharedStrings.xml");
    assert!(shared.contains("Größe"), "Kopf mit Umlaut");
    assert!(shared.contains("größe"), "Zelle mit Umlaut");
    let wb = file(&entries, "xl/workbook.xml");
    assert!(wb.contains("Größe &amp; Co"), "Blattname: {wb}");
    let styles = file(&entries, "xl/styles.xml");
    assert!(styles.contains("#,##0"), "Zahlenformat");
    let core = file(&entries, "docProps/core.xml");
    assert!(core.contains("@ Joschy"));
}

#[test]
fn xlsx_dates_are_dates() {
    let tree = sample_tree();
    let buf = xlsx(ExportContent::TopFiles(vec![8, 7]), Some(&tree));
    let entries = zip_entries(&buf);
    let sheet = file(&entries, "xl/worksheets/sheet1.xml");
    // Spalte G = Geändert, Excel-Seriennummer für Okt. 2026 beginnt mit 46
    assert!(sheet.contains("<c r=\"G2\" s="), "{sheet}");
    let g2 = &sheet[sheet.find("<c r=\"G2\"").unwrap()..];
    let v = &g2[g2.find("<v>").unwrap() + 3..g2.find("</v>").unwrap()];
    let serial: f64 = v.parse().unwrap();
    assert!((46_000.0..47_000.0).contains(&serial), "Serial {serial}");
    assert!(sheet.contains("<headerFooter") && sheet.contains("@ Joschy"));
    let styles = file(&entries, "xl/styles.xml");
    assert!(styles.contains("dd.mm.yyyy hh:mm"), "Datumsformat");
}
