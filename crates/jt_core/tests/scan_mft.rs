// Joschys_Tree – Tests MFT-Parser: Fixup, Records, Runlists, Waisen – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::PathBuf;

use jt_core::model::{FileTime, NodeFlags, ScanMeta, ScanMode};
use jt_core::scan::mft_parse::{
    MftError, MftRecord, SPARSE_LCN, apply_fixup, parse_chunk, parse_record, parse_runlist,
    plan_reads, records_to_tree, unnamed_data_runs,
};

// ---------------------------------------------------------------------------
// Record-Generator (erzeugt NTFS-FILE-Records wie Windows sie schreibt)
// ---------------------------------------------------------------------------

const REC: usize = 1024;
const SECTOR: usize = 512;
const USN: u16 = 0x0042;

fn ft(unix: i64) -> u64 {
    FileTime::from_unix_secs(unix).0 as u64
}

fn align8(n: usize) -> usize {
    (n + 7) & !7
}

fn utf16(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(|u| u.to_le_bytes()).collect()
}

struct RecBuilder {
    attrs: Vec<u8>,
    flags: u16,
    base: u64,
    id: u16,
}

impl RecBuilder {
    fn new(is_dir: bool) -> Self {
        RecBuilder {
            attrs: Vec::new(),
            flags: 1 | if is_dir { 2 } else { 0 },
            base: 0,
            id: 0,
        }
    }

    fn resident(&mut self, kind: u32, name: &str, value: &[u8]) -> &mut Self {
        let name_b = utf16(name);
        let name_off = 24usize;
        let value_off = align8(name_off + name_b.len());
        let len = align8(value_off + value.len());
        let mut a = vec![0u8; len];
        a[0..4].copy_from_slice(&kind.to_le_bytes());
        a[4..8].copy_from_slice(&(len as u32).to_le_bytes());
        a[8] = 0;
        a[9] = (name_b.len() / 2) as u8;
        a[10..12].copy_from_slice(&(name_off as u16).to_le_bytes());
        a[14..16].copy_from_slice(&self.id.to_le_bytes());
        a[16..20].copy_from_slice(&(value.len() as u32).to_le_bytes());
        a[20..22].copy_from_slice(&(value_off as u16).to_le_bytes());
        a[name_off..name_off + name_b.len()].copy_from_slice(&name_b);
        a[value_off..value_off + value.len()].copy_from_slice(value);
        self.id += 1;
        self.attrs.extend(a);
        self
    }

    #[allow(clippy::too_many_arguments)]
    fn non_resident(
        &mut self,
        kind: u32,
        name: &str,
        lowest_vcn: u64,
        alloc: u64,
        size: u64,
        runlist: &[u8],
    ) -> &mut Self {
        let name_b = utf16(name);
        let name_off = 64usize;
        let rl_off = align8(name_off + name_b.len());
        let len = align8(rl_off + runlist.len());
        let mut a = vec![0u8; len];
        a[0..4].copy_from_slice(&kind.to_le_bytes());
        a[4..8].copy_from_slice(&(len as u32).to_le_bytes());
        a[8] = 1;
        a[9] = (name_b.len() / 2) as u8;
        a[10..12].copy_from_slice(&(name_off as u16).to_le_bytes());
        a[14..16].copy_from_slice(&self.id.to_le_bytes());
        a[16..24].copy_from_slice(&lowest_vcn.to_le_bytes());
        a[32..34].copy_from_slice(&(rl_off as u16).to_le_bytes());
        a[40..48].copy_from_slice(&alloc.to_le_bytes());
        a[48..56].copy_from_slice(&size.to_le_bytes());
        a[56..64].copy_from_slice(&size.to_le_bytes());
        a[name_off..name_off + name_b.len()].copy_from_slice(&name_b);
        a[rl_off..rl_off + runlist.len()].copy_from_slice(runlist);
        self.id += 1;
        self.attrs.extend(a);
        self
    }

    fn std_info(&mut self, created: i64, modified: i64, accessed: i64, attrs: u32) -> &mut Self {
        let mut v = vec![0u8; 72];
        v[0..8].copy_from_slice(&ft(created).to_le_bytes());
        v[8..16].copy_from_slice(&ft(modified).to_le_bytes());
        v[16..24].copy_from_slice(&ft(modified).to_le_bytes());
        v[24..32].copy_from_slice(&ft(accessed).to_le_bytes());
        v[32..36].copy_from_slice(&attrs.to_le_bytes());
        self.resident(0x10, "", &v)
    }

    fn file_name(&mut self, parent: u64, seq: u16, name: &str, namespace: u8) -> &mut Self {
        self.file_name_tag(parent, seq, name, namespace, 0)
    }

    /// $FILE_NAME mit Reparse-Tag im Feld 0x3C.
    fn file_name_tag(
        &mut self,
        parent: u64,
        seq: u16,
        name: &str,
        namespace: u8,
        reparse: u32,
    ) -> &mut Self {
        let n = utf16(name);
        let mut v = vec![0u8; 66 + n.len()];
        let parent_ref = parent | ((seq as u64) << 48);
        v[0..8].copy_from_slice(&parent_ref.to_le_bytes());
        v[8..16].copy_from_slice(&ft(1).to_le_bytes());
        v[60..64].copy_from_slice(&reparse.to_le_bytes());
        v[64] = (n.len() / 2) as u8;
        v[65] = namespace;
        v[66..].copy_from_slice(&n);
        self.resident(0x30, "", &v)
    }

    fn base(&mut self, base: u64) -> &mut Self {
        self.base = base | (3u64 << 48);
        self
    }

    /// Fertiger Record mit angewendetem Update-Sequence-Schutz (wie auf Platte).
    fn build(&self, record_no: u32) -> Vec<u8> {
        let mut r = vec![0u8; REC];
        r[0..4].copy_from_slice(b"FILE");
        let usa_off = 48usize;
        let usa_cnt = (REC / SECTOR + 1) as u16;
        r[4..6].copy_from_slice(&(usa_off as u16).to_le_bytes());
        r[6..8].copy_from_slice(&usa_cnt.to_le_bytes());
        r[16..18].copy_from_slice(&1u16.to_le_bytes());
        r[18..20].copy_from_slice(&1u16.to_le_bytes());
        let first = align8(usa_off + 2 * usa_cnt as usize);
        r[20..22].copy_from_slice(&(first as u16).to_le_bytes());
        r[22..24].copy_from_slice(&self.flags.to_le_bytes());
        let end = first + self.attrs.len();
        let used = end + 8;
        assert!(used <= REC, "Record zu groß");
        r[24..28].copy_from_slice(&(used as u32).to_le_bytes());
        r[28..32].copy_from_slice(&(REC as u32).to_le_bytes());
        r[32..40].copy_from_slice(&self.base.to_le_bytes());
        r[40..42].copy_from_slice(&self.id.to_le_bytes());
        r[44..48].copy_from_slice(&record_no.to_le_bytes());
        r[first..end].copy_from_slice(&self.attrs);
        r[end..end + 4].copy_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
        // Update Sequence: Sektorenden sichern und durch USN ersetzen.
        r[usa_off..usa_off + 2].copy_from_slice(&USN.to_le_bytes());
        for i in 1..usa_cnt as usize {
            let e = i * SECTOR - 2;
            let (a, b) = (r[e], r[e + 1]);
            r[usa_off + 2 * i] = a;
            r[usa_off + 2 * i + 1] = b;
            r[e..e + 2].copy_from_slice(&USN.to_le_bytes());
        }
        r
    }
}

fn data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data")
}

/// Prüft die eingecheckte Fixture gegen den Generator. Fehlt sie, wird sie angelegt
/// (bzw. mit `JT_REGEN_FIXTURES=1` neu geschrieben).
fn fixture(name: &str, generated: &[u8]) -> Vec<u8> {
    let p = data_dir().join(name);
    if std::env::var_os("JT_REGEN_FIXTURES").is_some() || !p.exists() {
        std::fs::create_dir_all(data_dir()).expect("tests/data anlegen");
        std::fs::write(&p, generated).expect("Fixture schreiben");
    }
    let on_disk = std::fs::read(&p).expect("Fixture lesen");
    assert_eq!(on_disk, generated, "Fixture {name} weicht vom Generator ab");
    on_disk
}

/// Datei "Bericht.txt" (Record 42, Elternordner 5) mit DOS- und Win32-Namen,
/// residentem $DATA (100 Bytes) und einem ADS "geheim" (7 Bytes).
fn file_record_with_ads() -> Vec<u8> {
    let mut b = RecBuilder::new(false);
    b.std_info(1_700_000_000, 1_750_000_000, 1_760_000_000, 0x20 | 0x02)
        .file_name(5, 5, "BERICH~1.TXT", 2) // DOS zuerst: darf nicht gewinnen
        .file_name(5, 5, "Bericht.txt", 1) // Win32
        .resident(0x80, "", &[b'x'; 100])
        .resident(0x80, "geheim", b"secret!");
    b.build(42)
}

// ---------------------------------------------------------------------------
// (2) Fixup + parse_record
// ---------------------------------------------------------------------------

#[test]
fn scan_fixup_and_parse_file_with_ads() {
    let mut buf = fixture("mft_file_ads.bin", &file_record_with_ads());
    assert_eq!(buf.len(), 1024);
    // Vor dem Fixup stehen die USN-Werte an den Sektorenden.
    assert_eq!(&buf[510..512], &USN.to_le_bytes());
    apply_fixup(&mut buf, SECTOR).expect("Fixup");
    assert_ne!(&buf[1022..1024], &USN.to_le_bytes());

    let r = parse_record(&buf, 42)
        .expect("parse")
        .expect("in Benutzung");
    assert_eq!(r.record_no, 42);
    assert_eq!(r.name, "Bericht.txt");
    assert_eq!(r.parent_ref, 5, "Sequenznummer muss abgeschnitten sein");
    assert!(!r.is_dir);
    assert!(r.in_use);
    assert_eq!(r.size, 100);
    assert_eq!(r.allocated, 0, "resident = keine Cluster");
    assert_eq!(r.ads_size, 7);
    assert!(r.flags.contains(NodeFlags::HAS_ADS));
    assert!(r.flags.contains(NodeFlags::HIDDEN));
    assert!(!r.flags.contains(NodeFlags::DIR));
    assert_eq!(r.times[0], FileTime::from_unix_secs(1_700_000_000));
    assert_eq!(r.times[1], FileTime::from_unix_secs(1_750_000_000));
    assert_eq!(r.times[2], FileTime::from_unix_secs(1_760_000_000));
    assert_eq!(r.base_record, 0);
}

#[test]
fn scan_fixup_detects_torn_write() {
    let mut buf = file_record_with_ads();
    buf[1023] ^= 0xFF; // Sektor 2 unvollständig geschrieben
    assert_eq!(apply_fixup(&mut buf, SECTOR), Err(MftError::FixupMismatch));
    let mut short = vec![0u8; 4];
    assert_eq!(apply_fixup(&mut short, SECTOR), Err(MftError::TooShort));
}

#[test]
fn scan_fixup_stride_is_always_512() {
    // 4K-Native-Platte: Sektorgröße 4096, Fixup trotzdem alle 512 Byte.
    let mut a = file_record_with_ads();
    let mut b = a.clone();
    apply_fixup(&mut a, SECTOR).expect("Fixup 512");
    apply_fixup(&mut b, 4096).expect("Fixup bei 4096-Byte-Sektoren");
    assert_eq!(a, b);
    let r = parse_record(&b, 42).unwrap().unwrap();
    assert_eq!(r.name, "Bericht.txt");
}

#[test]
fn scan_nonresident_reparse_uses_file_name_tag() {
    // 0x400 = FILE_ATTRIBUTE_REPARSE_POINT; $REPARSE_POINT (0xC0) nicht resident.
    let build = |tag: u32| {
        let mut b = RecBuilder::new(false);
        b.std_info(1, 1, 1, 0x400)
            .file_name_tag(5, 1, "x.dat", 1, tag)
            .non_resident(0xC0, "", 0, 4096, 2000, &[0x11, 0x01, 0x10, 0x00]);
        let mut r = b.build(60);
        apply_fixup(&mut r, SECTOR).unwrap();
        parse_record(&r, 60).unwrap().unwrap()
    };
    // Cloud-Platzhalter (kein Name-Surrogate): normale Datei, kein REPARSE.
    assert!(!build(0x9000_601A).flags.contains(NodeFlags::REPARSE));
    // Symlink (Name-Surrogate): bleibt REPARSE.
    assert!(build(0xA000_000C).flags.contains(NodeFlags::REPARSE));
    // Tag unbekannt: vorsichtshalber REPARSE.
    assert!(build(0).flags.contains(NodeFlags::REPARSE));
}

#[test]
fn scan_parse_record_edge_cases() {
    // leerer Record
    assert_eq!(parse_record(&[0u8; 1024], 7), Ok(None));
    // nicht in Benutzung
    let mut b = RecBuilder::new(false);
    b.file_name(5, 5, "weg.txt", 1);
    b.flags = 0;
    let mut buf = b.build(9);
    apply_fixup(&mut buf, SECTOR).unwrap();
    assert_eq!(parse_record(&buf, 9), Ok(None));
    // falsche Signatur
    let mut bad = file_record_with_ads();
    bad[0..4].copy_from_slice(b"BAAD");
    assert_eq!(parse_record(&bad, 1), Err(MftError::BadSignature));
    // kaputte Attributlänge
    let mut buf = file_record_with_ads();
    apply_fixup(&mut buf, SECTOR).unwrap();
    let first = u16::from_le_bytes([buf[20], buf[21]]) as usize;
    buf[first + 4..first + 8].copy_from_slice(&5000u32.to_le_bytes());
    assert!(matches!(
        parse_record(&buf, 1),
        Err(MftError::BadAttribute(_))
    ));
}

#[test]
fn scan_hardlink_counts_first_name_and_nonresident_data() {
    let mut b = RecBuilder::new(false);
    b.std_info(1, 2, 3, 0)
        .file_name(100, 1, "erster.bin", 1)
        .file_name(200, 1, "zweiter.bin", 3) // Win32+DOS: zweiter Hardlink
        // 3 Cluster zu je 4096 an LCN 0x1000, Größe 10000
        .non_resident(0x80, "", 0, 12288, 10000, &[0x21, 0x03, 0x00, 0x10, 0x00])
        .non_resident(0x80, "zone", 0, 4096, 26, &[0x11, 0x01, 0x20, 0x00]);
    let mut buf = b.build(77);
    apply_fixup(&mut buf, SECTOR).unwrap();
    let r = parse_record(&buf, 77).unwrap().unwrap();
    assert_eq!(r.name, "erster.bin");
    assert_eq!(r.parent_ref, 100);
    assert_eq!(r.size, 10000);
    assert_eq!(r.allocated, 12288);
    assert_eq!(r.ads_size, 26);
    assert_eq!(unnamed_data_runs(&buf).unwrap(), vec![(0x1000, 3)]);
}

#[test]
fn scan_directory_record() {
    let mut b = RecBuilder::new(true);
    b.std_info(1, 2, 3, 0x10).file_name(5, 5, "Ordner", 3);
    let mut buf = b.build(30);
    apply_fixup(&mut buf, SECTOR).unwrap();
    let r = parse_record(&buf, 30).unwrap().unwrap();
    assert!(r.is_dir);
    assert!(r.flags.contains(NodeFlags::DIR));
    assert_eq!(r.name, "Ordner");
}

// ---------------------------------------------------------------------------
// (3) parse_runlist
// ---------------------------------------------------------------------------

#[test]
fn scan_runlist_known_sequences() {
    // a) ein Run: Länge 0x18 Cluster an LCN 0x5634
    assert_eq!(
        parse_runlist(&[0x21, 0x18, 0x34, 0x56, 0x00]).unwrap(),
        vec![(0x5634, 0x18)]
    );
    // b) zwei Runs mit negativem Offset: +0x5634 (Länge 0x18), dann -0x34 (0xCC = -52) Länge 0x10
    assert_eq!(
        parse_runlist(&[0x21, 0x18, 0x34, 0x56, 0x11, 0x10, 0xCC, 0x00]).unwrap(),
        vec![(0x5634, 0x18), (0x5634 - 0x34, 0x10)]
    );
    // c) sparse dazwischen: Run, Loch (Offset-Größe 0), Run relativ zum vorigen echten Run
    assert_eq!(
        parse_runlist(&[0x11, 0x30, 0x60, 0x01, 0x10, 0x11, 0x08, 0x20, 0x00]).unwrap(),
        vec![(0x60, 0x30), (SPARSE_LCN, 0x10), (0x80, 0x08)]
    );
    // d) mehrbytiger negativer Offset: 0x0100_0000 dann -0x10_0000 (0xF00000)
    assert_eq!(
        parse_runlist(&[
            0x41, 0x01, 0x00, 0x00, 0x00, 0x01, 0x31, 0x02, 0x00, 0x00, 0xF0, 0x00
        ])
        .unwrap(),
        vec![(0x0100_0000, 1), (0x00F0_0000, 2)]
    );
}

#[test]
fn scan_runlist_errors() {
    assert_eq!(parse_runlist(&[0x21, 0x18]), Err(MftError::BadRunlist)); // abgeschnitten
    assert_eq!(
        parse_runlist(&[0x11, 0x01, 0x80, 0x00]),
        Err(MftError::BadRunlist)
    ); // LCN < 0
    assert_eq!(parse_runlist(&[0x20, 0x00]), Err(MftError::BadRunlist)); // Länge 0 Bytes
    assert_eq!(parse_runlist(&[0x00]).unwrap(), vec![]);
}

#[test]
fn scan_read_plan_chunks_and_skips_sparse() {
    let runs = [(10, 300), (SPARSE_LCN, 2), (1000, 10)];
    // Cluster 4096, Blockgröße 1 MiB = 256 Cluster
    let plan = plan_reads(&runs, 4096, (300 + 2 + 10) * 4096, 1 << 20);
    assert_eq!(plan.len(), 3);
    assert_eq!(plan[0].disk_offset, 10 * 4096);
    assert_eq!(plan[0].len, 1 << 20);
    assert_eq!(plan[0].mft_offset, 0);
    assert_eq!(plan[1].len, 44 * 4096);
    assert_eq!(plan[1].mft_offset, 256 * 4096);
    assert_eq!(plan[2].disk_offset, 1000 * 4096);
    assert_eq!(plan[2].mft_offset, 302 * 4096);
    // Valid-Data-Length kürzt
    let short = plan_reads(&runs, 4096, 5000, 1 << 20);
    assert_eq!(short.len(), 1);
    assert_eq!(short[0].len, 5000);
}

#[test]
fn scan_parse_chunk_parallel() {
    let mut buf = Vec::new();
    buf.extend(vec![0u8; REC]); // Record 100: leer
    buf.extend(file_record_with_ads()); // Record 101
    let mut bad = file_record_with_ads();
    bad[1023] ^= 1; // Record 102: Fixup kaputt
    buf.extend(bad);
    let (recs, errs) = parse_chunk(&mut buf, 100, REC, SECTOR);
    assert_eq!(errs, 1);
    assert_eq!(recs.len(), 1);
    assert_eq!(recs[0].record_no, 101);
    assert_eq!(recs[0].name, "Bericht.txt");
}

// ---------------------------------------------------------------------------
// (4) records_to_tree
// ---------------------------------------------------------------------------

fn rec(no: u64, parent: u64, name: &str, is_dir: bool, size: u64) -> MftRecord {
    MftRecord {
        record_no: no,
        parent_ref: parent,
        name: name.to_string(),
        is_dir,
        in_use: true,
        size,
        allocated: size.div_ceil(4096) * 4096,
        times: [FileTime(1), FileTime(2), FileTime(3)],
        flags: if is_dir {
            NodeFlags::DIR
        } else {
            NodeFlags::empty()
        },
        ads_size: 0,
        base_record: 0,
    }
}

#[test]
fn scan_records_to_tree_links_orphans() {
    // Absichtlich ungeordnet: Kinder vor Eltern, tiefe Kette, Wurzel-Record 5.
    let mut recs = vec![
        rec(40, 31, "tief.dat", false, 5000),
        rec(31, 30, "b", true, 0),
        rec(41, 5, "oben.txt", false, 100),
        rec(30, 5, "a", true, 0),
        rec(5, 5, ".", true, 0),
        rec(50, 999, "verloren.txt", false, 7), // Elternteil fehlt -> Wurzel
        rec(0, 5, "$MFT", false, 4096),
    ];
    // Nicht benutzt -> ignoriert
    let mut gone = rec(60, 5, "geloescht.txt", false, 1_000_000);
    gone.in_use = false;
    recs.push(gone);
    // Erweiterungs-Record von 41 ($ATTRIBUTE_LIST): bringt einen ADS mit
    let mut ext = rec(70, 0, "", false, 0);
    ext.allocated = 0;
    ext.ads_size = 33;
    ext.base_record = 41;
    recs.push(ext);
    // Erweiterungs-Record aus Bytes (Basis 31) – ohne Namen, darf keinen Knoten erzeugen
    let mut eb = RecBuilder::new(true);
    eb.base(31).resident(0x80, "", &[]);
    let mut eb = eb.build(71);
    apply_fixup(&mut eb, SECTOR).unwrap();
    let e71 = parse_record(&eb, 71).unwrap().unwrap();
    assert_eq!(e71.base_record, 31);
    recs.push(e71);

    let meta = ScanMeta::new("C:\\", ScanMode::Mft);
    let t = records_to_tree(recs.into_iter(), meta, "C:\\");

    assert_eq!(t.meta.mode, ScanMode::Mft);
    assert_eq!(
        t.nodes.len(),
        7,
        "Wurzel + a + b + tief + oben + verloren + $MFT"
    );
    let tief = t.find_path("C:\\a\\b\\tief.dat").expect("tief.dat");
    assert_eq!(t.node(tief).size, 5000);
    let a = t.find_path("C:\\a").unwrap();
    assert_eq!(t.node(a).size, 5000);
    assert_eq!(t.node(a).file_count, 1);
    assert_eq!(t.node(a).dir_count, 1);
    let oben = t.find_path("c:\\OBEN.TXT").unwrap();
    assert_eq!(t.node(oben).ads_size, 33);
    assert!(t.node(oben).flags.contains(NodeFlags::HAS_ADS));
    assert!(t.find_path("C:\\verloren.txt").is_some());
    assert!(t.find_path("C:\\geloescht.txt").is_none());
    let root = t.node(0);
    assert_eq!(root.size, 5000 + 100 + 7 + 4096);
    assert_eq!(root.file_count, 4);
    assert_eq!(root.dir_count, 2);
    assert_eq!(t.node(tief).modified, FileTime(2));
    // Eltern-/Kind-Konsistenz
    for id in 0..t.nodes.len() as u32 {
        for c in t.children(id) {
            assert_eq!(t.node(c).parent, id);
        }
    }
}
