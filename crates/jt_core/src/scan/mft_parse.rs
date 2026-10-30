// Joschys_Tree – Plattformneutraler Parser für NTFS-MFT-Records – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Reiner Parser für NTFS-MFT-Records (FILE-Records). Kein Windows-Code,
//! alles mit Byte-Fixtures auf Linux testbar.
//!
//! Unterstützt: Fixup (Update Sequence Array), `$STANDARD_INFORMATION` (0x10),
//! `$ATTRIBUTE_LIST` (0x20, über Zusammenführen der Erweiterungs-Records),
//! `$FILE_NAME` (0x30, Win32/POSIX vor DOS, bei Hardlinks nur der erste Name),
//! `$DATA` (0x80, resident und nicht resident, benannte Ströme = ADS), Runlists.

use std::fmt;

use crate::model::{EntryInfo, FileTime, NodeFlags, ScanMeta, Tree, TreeBuilder};

/// Record-Nummer des Wurzelverzeichnisses "." auf jedem NTFS-Volume.
pub const ROOT_RECORD: u64 = 5;

/// LCN-Wert für einen Sparse-Run (keine Cluster auf Platte) in `parse_runlist`.
pub const SPARSE_LCN: u64 = u64::MAX;

/// Untere 48 Bit einer File-Reference = Record-Nummer (obere 16 Bit = Sequenz).
const REF_MASK: u64 = 0x0000_FFFF_FFFF_FFFF;

const ATTR_STANDARD_INFORMATION: u32 = 0x10;
const ATTR_ATTRIBUTE_LIST: u32 = 0x20;
const ATTR_FILE_NAME: u32 = 0x30;
const ATTR_DATA: u32 = 0x80;
const ATTR_REPARSE_POINT: u32 = 0xC0;
/// Reparse-Tag-Bit "Name-Surrogate" (Symlink, Junction, Mountpoint).
const REPARSE_TAG_NAME_SURROGATE: u32 = 0x2000_0000;
const ATTR_END: u32 = 0xFFFF_FFFF;

const RECORD_IN_USE: u16 = 0x0001;
const RECORD_IS_DIR: u16 = 0x0002;

const ATTR_FLAG_COMPRESSED: u16 = 0x0001;
const ATTR_FLAG_SPARSE: u16 = 0x8000;

const NAMESPACE_DOS: u8 = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MftError {
    TooShort,
    BadSignature,
    FixupMismatch,
    BadAttribute(String),
    BadRunlist,
}

impl fmt::Display for MftError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MftError::TooShort => write!(f, "Record zu kurz"),
            MftError::BadSignature => write!(f, "ungültige Record-Signatur"),
            MftError::FixupMismatch => write!(f, "Fixup-Prüfung fehlgeschlagen"),
            MftError::BadAttribute(s) => write!(f, "ungültiges Attribut: {s}"),
            MftError::BadRunlist => write!(f, "ungültige Runlist"),
        }
    }
}

impl std::error::Error for MftError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MftRecord {
    pub record_no: u64,
    /// Record-Nummer des Elternordners (ohne Sequenznummer)
    pub parent_ref: u64,
    pub name: String,
    pub is_dir: bool,
    pub in_use: bool,
    /// logische Größe des unbenannten `$DATA`-Stroms
    pub size: u64,
    /// belegter Platz des unbenannten `$DATA`-Stroms (0 bei residenten Daten)
    pub allocated: u64,
    /// created, modified, accessed
    pub times: [FileTime; 3],
    pub flags: NodeFlags,
    /// Summe der benannten `$DATA`-Ströme
    pub ads_size: u64,
    /// 0 = Basis-Record, sonst Record-Nummer des Basis-Records (Erweiterungs-Record)
    pub base_record: u64,
}

// ---------------------------------------------------------------------------
// Byte-Helfer mit Grenzprüfung
// ---------------------------------------------------------------------------

fn slice(b: &[u8], off: usize, len: usize) -> Result<&[u8], MftError> {
    let end = off.checked_add(len).ok_or(MftError::TooShort)?;
    b.get(off..end).ok_or(MftError::TooShort)
}

fn u16_at(b: &[u8], off: usize) -> Result<u16, MftError> {
    let s = slice(b, off, 2)?;
    Ok(u16::from_le_bytes([s[0], s[1]]))
}

fn u32_at(b: &[u8], off: usize) -> Result<u32, MftError> {
    let s = slice(b, off, 4)?;
    Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn u64_at(b: &[u8], off: usize) -> Result<u64, MftError> {
    let s = slice(b, off, 8)?;
    let mut a = [0u8; 8];
    a.copy_from_slice(s);
    Ok(u64::from_le_bytes(a))
}

fn time_at(b: &[u8], off: usize) -> Result<FileTime, MftError> {
    Ok(FileTime(u64_at(b, off)? as i64))
}

/// Windows-Dateiattribute (FILE_ATTRIBUTE_*) in `NodeFlags` umsetzen.
/// Wird vom MFT-Parser und vom Walk unter Windows benutzt.
pub fn flags_from_win_attributes(attr: u32) -> NodeFlags {
    let mut f = NodeFlags::empty();
    if attr & 0x0001 != 0 {
        f |= NodeFlags::READONLY;
    }
    if attr & 0x0002 != 0 {
        f |= NodeFlags::HIDDEN;
    }
    if attr & 0x0004 != 0 {
        f |= NodeFlags::SYSTEM;
    }
    if attr & 0x0010 != 0 {
        f |= NodeFlags::DIR;
    }
    if attr & 0x0200 != 0 {
        f |= NodeFlags::SPARSE;
    }
    if attr & 0x0400 != 0 {
        f |= NodeFlags::REPARSE;
    }
    if attr & 0x0800 != 0 {
        f |= NodeFlags::COMPRESSED;
    }
    // OFFLINE (0x1000) und RECALL_ON_DATA_ACCESS (0x400000, z. B. OneDrive "nur online")
    if attr & 0x1000 != 0 || attr & 0x0040_0000 != 0 {
        f |= NodeFlags::OFFLINE;
    }
    f
}

// ---------------------------------------------------------------------------
// Fixup
// ---------------------------------------------------------------------------

/// NTFS-Fixup arbeitet immer in 512-Byte-Schritten, unabhängig von der Sektorgröße
/// des Datenträgers (auch bei 4K-Native-Platten).
const FIXUP_STRIDE: usize = 512;

/// Wendet das Update Sequence Array an: prüft das Ende jedes 512-Byte-Blocks gegen
/// die Update Sequence Number und stellt die Originalbytes wieder her.
/// Ein komplett leerer Record (USA-Anzahl 0) bleibt unverändert.
///
/// `_bytes_per_sector` bleibt aus Kompatibilität (Schnittstelle laut PLAN) erhalten,
/// wird aber nicht als Schrittweite verwendet: die ist bei NTFS fest 512 Byte.
pub fn apply_fixup(buf: &mut [u8], _bytes_per_sector: usize) -> Result<(), MftError> {
    if buf.len() < 8 {
        return Err(MftError::TooShort);
    }
    let usa_off = u16_at(buf, 4)? as usize;
    let usa_cnt = u16_at(buf, 6)? as usize;
    if usa_cnt == 0 {
        return Ok(());
    }
    let sectors = usa_cnt - 1;
    if usa_off < 8 || usa_off + 2 * usa_cnt > buf.len() {
        return Err(MftError::TooShort);
    }
    if sectors * FIXUP_STRIDE > buf.len() {
        return Err(MftError::TooShort);
    }
    let usn = [buf[usa_off], buf[usa_off + 1]];
    for i in 1..=sectors {
        let end = i * FIXUP_STRIDE - 2;
        if buf[end..end + 2] != usn {
            return Err(MftError::FixupMismatch);
        }
        let src = usa_off + 2 * i;
        buf[end] = buf[src];
        buf[end + 1] = buf[src + 1];
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Record-Parser
// ---------------------------------------------------------------------------

struct FileNameAttr {
    parent: u64,
    namespace: u8,
    name: String,
    times: [FileTime; 3],
    /// Feld 0x3C "EA/Reparse": bei Reparse-Points der Reparse-Tag (sonst 0).
    reparse: u32,
}

/// Ein Attribut-Header mit den Feldern, die wir brauchen.
struct Attr<'a> {
    kind: u32,
    raw: &'a [u8],
    non_resident: bool,
    name_len: u8,
    flags: u16,
}

impl Attr<'_> {
    /// Wert eines residenten Attributs.
    fn value(&self) -> Result<&[u8], MftError> {
        let len = u32_at(self.raw, 16)? as usize;
        let off = u16_at(self.raw, 20)? as usize;
        slice(self.raw, off, len)
            .map_err(|_| MftError::BadAttribute(format!("Wert 0x{:X} ragt heraus", self.kind)))
    }

    /// Größenangaben eines Datenstroms: (logische Größe, belegt, ist erster Abschnitt).
    fn data_sizes(&self) -> Result<(u64, u64, bool), MftError> {
        if !self.non_resident {
            return Ok((u32_at(self.raw, 16)? as u64, 0, true));
        }
        let lowest_vcn = u64_at(self.raw, 16)?;
        if lowest_vcn != 0 {
            // Folgeabschnitt eines fragmentierten Attributs: Größen stehen nur im ersten.
            return Ok((0, 0, false));
        }
        let mut allocated = u64_at(self.raw, 40)?;
        let size = u64_at(self.raw, 48)?;
        if self.flags & (ATTR_FLAG_COMPRESSED | ATTR_FLAG_SPARSE) != 0 && self.raw.len() >= 72 {
            allocated = u64_at(self.raw, 64)?;
        }
        Ok((size, allocated, true))
    }
}

/// Iteriert über die Attribute eines (bereits gefixten) Records.
fn attributes(buf: &[u8]) -> Result<Vec<Attr<'_>>, MftError> {
    let first = u16_at(buf, 20)? as usize;
    let used = (u32_at(buf, 24)? as usize).min(buf.len());
    let mut out = Vec::new();
    let mut off = first;
    while off + 8 <= used {
        let kind = u32_at(buf, off)?;
        if kind == ATTR_END {
            break;
        }
        let len = u32_at(buf, off + 4)? as usize;
        if len < 16 || off + len > used {
            return Err(MftError::BadAttribute(format!(
                "Länge {len} bei Offset {off} (Typ 0x{kind:X})"
            )));
        }
        let raw = &buf[off..off + len];
        out.push(Attr {
            kind,
            raw,
            non_resident: raw[8] != 0,
            name_len: raw[9],
            flags: u16_at(raw, 12)?,
        });
        off += len;
    }
    Ok(out)
}

fn parse_file_name(v: &[u8]) -> Result<FileNameAttr, MftError> {
    let parent = u64_at(v, 0)? & REF_MASK;
    let times = [time_at(v, 8)?, time_at(v, 16)?, time_at(v, 32)?];
    let len = *v.get(64).ok_or(MftError::TooShort)? as usize;
    let namespace = *v.get(65).ok_or(MftError::TooShort)?;
    let raw = slice(v, 66, len * 2)
        .map_err(|_| MftError::BadAttribute(String::from("$FILE_NAME-Name ragt heraus")))?;
    let units: Vec<u16> = raw
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| u16::from_le_bytes(*c))
        .collect();
    Ok(FileNameAttr {
        parent,
        namespace,
        name: String::from_utf16_lossy(&units),
        times,
        reparse: u32_at(v, 60)?,
    })
}

/// Parst einen MFT-Record (Fixup muss vorher angewendet sein).
///
/// * `Ok(None)`: Record ist leer (nur Nullen) oder nicht in Benutzung.
/// * Erweiterungs-Records (`base_record != 0`) werden ebenfalls geliefert, damit
///   `records_to_tree` ihre Attribute (aus `$ATTRIBUTE_LIST`) dem Basis-Record zuschlagen kann.
pub fn parse_record(buf: &[u8], record_no: u64) -> Result<Option<MftRecord>, MftError> {
    if buf.len() < 48 {
        return Err(MftError::TooShort);
    }
    if &buf[0..4] != b"FILE" {
        if buf.iter().all(|&b| b == 0) {
            return Ok(None);
        }
        return Err(MftError::BadSignature);
    }
    let rec_flags = u16_at(buf, 22)?;
    if rec_flags & RECORD_IN_USE == 0 {
        return Ok(None);
    }
    let is_dir = rec_flags & RECORD_IS_DIR != 0;
    let base_record = u64_at(buf, 32)? & REF_MASK;

    let mut si_times: Option<[FileTime; 3]> = None;
    let mut flags = NodeFlags::empty();
    let mut best_name: Option<FileNameAttr> = None;
    let mut size = 0u64;
    let mut allocated = 0u64;
    let mut ads_size = 0u64;
    let mut reparse_tag: Option<u32> = None;
    let mut reparse_non_resident = false;

    for a in attributes(buf)? {
        match a.kind {
            ATTR_STANDARD_INFORMATION if !a.non_resident => {
                let v = a.value()?;
                si_times = Some([time_at(v, 0)?, time_at(v, 8)?, time_at(v, 24)?]);
                flags |= flags_from_win_attributes(u32_at(v, 32)?);
            }
            ATTR_FILE_NAME if !a.non_resident => {
                let fname = parse_file_name(a.value()?)?;
                // Win32/POSIX vor DOS; bei mehreren Nicht-DOS-Namen (Hardlinks) gilt der erste.
                let replace = match &best_name {
                    None => true,
                    Some(b) => b.namespace == NAMESPACE_DOS && fname.namespace != NAMESPACE_DOS,
                };
                if replace {
                    best_name = Some(fname);
                }
            }
            ATTR_DATA => {
                let (s, al, first) = a.data_sizes()?;
                if !first {
                    continue;
                }
                if a.name_len == 0 {
                    size = s;
                    allocated = al;
                } else {
                    ads_size = ads_size.saturating_add(s);
                }
            }
            // $ATTRIBUTE_LIST: verweist auf Erweiterungs-Records. Diese tragen ihre
            // Basis-Record-Nummer selbst und werden in records_to_tree zusammengeführt.
            ATTR_ATTRIBUTE_LIST => {}
            ATTR_REPARSE_POINT if !a.non_resident => {
                reparse_tag = a.value().and_then(|v| u32_at(v, 0)).ok();
            }
            ATTR_REPARSE_POINT => reparse_non_resident = true,
            _ => {}
        }
    }

    // DIR-Bit aus den Attributen ist nicht aussagekräftig, maßgeblich ist der Record-Header.
    flags.remove(NodeFlags::DIR);
    if is_dir {
        flags |= NodeFlags::DIR;
    }
    if ads_size > 0 {
        flags |= NodeFlags::HAS_ADS;
    }
    // Nicht residenter $REPARSE_POINT: Tag steht nicht im Record, aber im $FILE_NAME
    // (Feld 0x3C). Fehlt er auch dort, bleibt REPARSE vorsichtshalber stehen.
    if reparse_tag.is_none() && reparse_non_resident {
        reparse_tag = best_name.as_ref().map(|f| f.reparse).filter(|&t| t != 0);
    }
    // REPARSE nur für echte Links (OneDrive-Platzhalter, Dedup usw. sind normale Dateien).
    if reparse_tag.is_some_and(|t| t & REPARSE_TAG_NAME_SURROGATE == 0) {
        flags.remove(NodeFlags::REPARSE);
    }
    let (name, parent_ref, fn_times) = match best_name {
        Some(f) => (f.name, f.parent, Some(f.times)),
        None => (String::new(), 0, None),
    };
    let times = si_times.or(fn_times).unwrap_or_default();

    Ok(Some(MftRecord {
        record_no,
        parent_ref,
        name,
        is_dir,
        in_use: true,
        size,
        allocated,
        times,
        flags,
        ads_size,
        base_record,
    }))
}

// ---------------------------------------------------------------------------
// Runlist
// ---------------------------------------------------------------------------

fn read_le_unsigned(b: &[u8]) -> u64 {
    b.iter().rev().fold(0u64, |acc, &x| (acc << 8) | x as u64)
}

fn read_le_signed(b: &[u8]) -> i64 {
    if b.is_empty() {
        return 0;
    }
    let mut v = read_le_unsigned(b) as i64;
    let bits = b.len() * 8;
    if bits < 64 && b[b.len() - 1] & 0x80 != 0 {
        v -= 1i64 << bits;
    }
    v
}

/// Liefert (lcn, len)-Paare. Sparse-Runs haben `lcn == SPARSE_LCN`.
/// Offsets sind relativ zum vorigen (nicht-sparse) Run und können negativ sein.
pub fn parse_runlist(data: &[u8]) -> Result<Vec<(u64, u64)>, MftError> {
    let mut out = Vec::new();
    let mut pos = 0usize;
    let mut lcn: i64 = 0;
    loop {
        let Some(&header) = data.get(pos) else {
            // Kein Endmarker: nur ok, wenn die Daten exakt nach einem Run enden.
            return if out.is_empty() && data.is_empty() {
                Ok(out)
            } else {
                Err(MftError::BadRunlist)
            };
        };
        if header == 0 {
            return Ok(out);
        }
        let len_size = (header & 0x0F) as usize;
        let off_size = (header >> 4) as usize;
        if len_size == 0 || len_size > 8 || off_size > 8 {
            return Err(MftError::BadRunlist);
        }
        pos += 1;
        let len_bytes = data.get(pos..pos + len_size).ok_or(MftError::BadRunlist)?;
        let len = read_le_unsigned(len_bytes);
        pos += len_size;
        if off_size == 0 {
            out.push((SPARSE_LCN, len));
            continue;
        }
        let off_bytes = data.get(pos..pos + off_size).ok_or(MftError::BadRunlist)?;
        pos += off_size;
        lcn = lcn
            .checked_add(read_le_signed(off_bytes))
            .ok_or(MftError::BadRunlist)?;
        if lcn < 0 {
            return Err(MftError::BadRunlist);
        }
        out.push((lcn as u64, len));
    }
}

/// Runlist des unbenannten, nicht residenten `$DATA`-Stroms eines (gefixten) Records,
/// z. B. von `$MFT` selbst (Record 0). Mehrere Abschnitte im selben Record werden
/// nach `lowest_vcn` aneinandergehängt.
pub fn unnamed_data_runs(buf: &[u8]) -> Result<Vec<(u64, u64)>, MftError> {
    if buf.len() < 48 || &buf[0..4] != b"FILE" {
        return Err(MftError::BadSignature);
    }
    let mut parts: Vec<(u64, Vec<(u64, u64)>)> = Vec::new();
    for a in attributes(buf)? {
        if a.kind != ATTR_DATA || a.name_len != 0 || !a.non_resident {
            continue;
        }
        let vcn = u64_at(a.raw, 16)?;
        let off = u16_at(a.raw, 32)? as usize;
        let rl = a.raw.get(off..).ok_or(MftError::BadRunlist)?;
        parts.push((vcn, parse_runlist(rl)?));
    }
    if parts.is_empty() {
        return Err(MftError::BadAttribute(String::from(
            "kein nicht residenter $DATA-Strom",
        )));
    }
    parts.sort_by_key(|p| p.0);
    Ok(parts.into_iter().flat_map(|p| p.1).collect())
}

// ---------------------------------------------------------------------------
// Lese-Plan und Block-Parser (reine Logik für mft_win)
// ---------------------------------------------------------------------------

/// Ein zusammenhängender Lesevorgang auf dem Volume.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReadChunk {
    /// Byte-Offset auf dem Volume
    pub disk_offset: u64,
    /// Anzahl Bytes (Vielfaches der Clustergröße, letzter Block evtl. gekürzt auf `total`)
    pub len: u64,
    /// Byte-Offset innerhalb der $MFT (=> erste Record-Nummer = mft_offset / record_size)
    pub mft_offset: u64,
}

/// Zerlegt die Runlist der $MFT in Lesevorgänge von höchstens `chunk` Bytes.
/// Sparse-Runs werden übersprungen, gelesen wird nur bis `total` (MftValidDataLength).
/// `chunk` wird auf ein Vielfaches von `cluster` abgerundet (mindestens 1 Cluster).
pub fn plan_reads(runs: &[(u64, u64)], cluster: u64, total: u64, chunk: u64) -> Vec<ReadChunk> {
    let mut out = Vec::new();
    if cluster == 0 {
        return out;
    }
    let chunk = (chunk / cluster).max(1) * cluster;
    let mut mft_pos = 0u64;
    for &(lcn, len) in runs {
        if mft_pos >= total {
            break;
        }
        let run_bytes = len.saturating_mul(cluster);
        if lcn == SPARSE_LCN {
            mft_pos = mft_pos.saturating_add(run_bytes);
            continue;
        }
        let run_bytes = run_bytes.min(total - mft_pos);
        let mut done = 0u64;
        while done < run_bytes {
            let n = chunk.min(run_bytes - done);
            out.push(ReadChunk {
                disk_offset: lcn.saturating_mul(cluster).saturating_add(done),
                len: n,
                mft_offset: mft_pos + done,
            });
            done += n;
        }
        mft_pos += run_bytes;
    }
    out
}

/// Parst einen gelesenen $MFT-Block (Fixup + Records) parallel.
/// Rückgabe: (gültige, benutzte Records; Anzahl defekter Records).
pub fn parse_chunk(
    buf: &mut [u8],
    first_record_no: u64,
    record_size: usize,
    bytes_per_sector: usize,
) -> (Vec<MftRecord>, u32) {
    use rayon::prelude::*;
    if record_size < 48 {
        return (Vec::new(), 0);
    }
    let results: Vec<Result<Option<MftRecord>, MftError>> = buf
        .par_chunks_exact_mut(record_size)
        .enumerate()
        .map(|(i, rec)| {
            let no = first_record_no + i as u64;
            if &rec[0..4] != b"FILE" {
                // Leer oder nie benutzt ("BAAD" bzw. Nullen) – kein Fehler zählen außer BAAD.
                return if &rec[0..4] == b"BAAD" {
                    Err(MftError::BadSignature)
                } else {
                    Ok(None)
                };
            }
            apply_fixup(rec, bytes_per_sector)?;
            parse_record(rec, no)
        })
        .collect();
    let mut errors = 0u32;
    let mut out = Vec::with_capacity(results.len());
    for r in results {
        match r {
            Ok(Some(rec)) => out.push(rec),
            Ok(None) => {}
            Err(_) => errors += 1,
        }
    }
    (out, errors)
}

// ---------------------------------------------------------------------------
// Records -> Tree
// ---------------------------------------------------------------------------

fn merge_extension(base: &mut MftRecord, ext: MftRecord) {
    base.size = base.size.saturating_add(ext.size);
    base.allocated = base.allocated.saturating_add(ext.allocated);
    base.ads_size = base.ads_size.saturating_add(ext.ads_size);
    if ext.ads_size > 0 {
        base.flags |= NodeFlags::HAS_ADS;
    }
    if base.name.is_empty() && !ext.name.is_empty() {
        base.name = ext.name;
        base.parent_ref = ext.parent_ref;
        if base.times == [FileTime::default(); 3] {
            base.times = ext.times;
        }
    }
}

/// Baut aus beliebig sortierten Records einen Tree.
///
/// * Erweiterungs-Records werden ihrem Basis-Record zugeschlagen (`$ATTRIBUTE_LIST`).
/// * Record 5 (Wurzel ".") wird zur Tree-Wurzel; Einträge mit unbekanntem Elternteil
///   landen ebenfalls an der Wurzel.
/// * Nicht benutzte Records und Records ohne Namen werden übersprungen.
/// * Hardlinks erscheinen nur einmal (erster Name, siehe `parse_record`).
pub fn records_to_tree(
    records: impl Iterator<Item = MftRecord>,
    meta: ScanMeta,
    root_name: &str,
) -> Tree {
    let mut bases: Vec<MftRecord> = Vec::new();
    let mut exts: Vec<MftRecord> = Vec::new();
    for r in records {
        if !r.in_use {
            continue;
        }
        if r.base_record != 0 && r.base_record != r.record_no {
            exts.push(r);
        } else {
            bases.push(r);
        }
    }
    bases.sort_unstable_by_key(|r| r.record_no);
    bases.dedup_by_key(|r| r.record_no);
    for e in exts {
        if let Ok(i) = bases.binary_search_by_key(&e.base_record, |r| r.record_no) {
            merge_extension(&mut bases[i], e);
        }
    }

    let mut b = TreeBuilder::new(meta, root_name);
    for r in bases {
        if r.record_no == ROOT_RECORD || r.name.is_empty() {
            continue;
        }
        let info = EntryInfo {
            size: if r.is_dir { 0 } else { r.size },
            allocated: if r.is_dir { 0 } else { r.allocated },
            created: r.times[0],
            modified: r.times[1],
            accessed: r.times[2],
            flags: r.flags,
            ads_size: r.ads_size,
        };
        b.add_orphan(r.record_no, r.parent_ref, &r.name, info, r.is_dir);
    }
    b.finish()
}
