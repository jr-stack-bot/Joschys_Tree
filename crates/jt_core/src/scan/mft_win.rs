// Joschys_Tree – MFT-Scan unter Windows (Volume öffnen, $MFT lesen) – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Nur I/O: Volume `\\.\C:` öffnen, `FSCTL_GET_NTFS_VOLUME_DATA`, Record 0 ($MFT)
//! lesen, dessen Runlist in 1-MiB-Blöcken abarbeiten. Fixup, Parsen, Lese-Plan und
//! Baumaufbau stehen plattformneutral in `mft_parse.rs`.

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use windows::Win32::Foundation::{CloseHandle, GENERIC_READ, HANDLE};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_BEGIN, FILE_SHARE_DELETE, FILE_SHARE_READ,
    FILE_SHARE_WRITE, OPEN_EXISTING, ReadFile, SetFilePointerEx,
};
use windows::Win32::System::IO::DeviceIoControl;
use windows::Win32::System::Ioctl::{FSCTL_GET_NTFS_VOLUME_DATA, NTFS_VOLUME_DATA_BUFFER};
use windows::core::PCWSTR;

use super::mft_parse::{self, MftRecord};
use super::{Phase, ScanError, ScanProgress, ScanRequest};
use crate::model::{ScanMeta, ScanMode, Tree};

const CHUNK: u64 = 1024 * 1024;

struct Volume(HANDLE);

impl Drop for Volume {
    fn drop(&mut self) {
        // SAFETY: Handle aus CreateFileW, wird nur hier geschlossen.
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

impl Volume {
    fn open(letter: char) -> Result<Self, ScanError> {
        let path: Vec<u16> = format!(r"\\.\{letter}:")
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        // SAFETY: path ist nullterminiert und lebt bis nach dem Aufruf.
        let h = unsafe {
            CreateFileW(
                PCWSTR(path.as_ptr()),
                GENERIC_READ.0,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                None,
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                None,
            )
        }
        .map_err(|e| ScanError::Io(e.into()))?;
        Ok(Volume(h))
    }

    fn ntfs_data(&self) -> Result<NTFS_VOLUME_DATA_BUFFER, ScanError> {
        let mut data = NTFS_VOLUME_DATA_BUFFER::default();
        let mut ret = 0u32;
        // SAFETY: Ausgabepuffer hat exakt die Größe der Struktur.
        unsafe {
            DeviceIoControl(
                self.0,
                FSCTL_GET_NTFS_VOLUME_DATA,
                None,
                0,
                Some(&mut data as *mut NTFS_VOLUME_DATA_BUFFER as *mut c_void),
                std::mem::size_of::<NTFS_VOLUME_DATA_BUFFER>() as u32,
                Some(&mut ret),
                None,
            )
        }
        .map_err(|e| ScanError::Unsupported(format!("kein NTFS: {e}")))?;
        Ok(data)
    }

    /// Liest genau `buf.len()` Bytes ab `offset` (Offset und Länge sektor-ausgerichtet).
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> Result<(), ScanError> {
        // SAFETY: Handle gültig; kein Ausgabezeiger nötig.
        unsafe { SetFilePointerEx(self.0, offset as i64, None, FILE_BEGIN) }
            .map_err(|e| ScanError::Io(e.into()))?;
        let mut done = 0usize;
        while done < buf.len() {
            let mut n = 0u32;
            // SAFETY: Slice gültig, n ist lokale Variable.
            unsafe { ReadFile(self.0, Some(&mut buf[done..]), Some(&mut n), None) }
                .map_err(|e| ScanError::Io(e.into()))?;
            if n == 0 {
                return Err(ScanError::Io(std::io::Error::from(
                    std::io::ErrorKind::UnexpectedEof,
                )));
            }
            done += n as usize;
        }
        Ok(())
    }
}

pub(crate) fn scan_mft(
    req: &ScanRequest,
    progress: &dyn Fn(&ScanProgress),
    cancel: &AtomicBool,
) -> Result<Tree, ScanError> {
    let t0 = Instant::now();
    let started = super::now_filetime();
    let letter = super::drive_root_letter(&req.root)
        .ok_or_else(|| ScanError::Unsupported(String::from("nur Laufwerkswurzeln")))?;
    let vol = Volume::open(letter)?;
    let nd = vol.ntfs_data()?;

    let cluster = nd.BytesPerCluster as u64;
    let rec_size = nd.BytesPerFileRecordSegment as usize;
    let sector = nd.BytesPerSector as usize;
    if cluster == 0 || rec_size < 256 || sector == 0 || rec_size % sector != 0 {
        return Err(ScanError::Unsupported(String::from(
            "unerwartete NTFS-Geometrie",
        )));
    }

    let mut prog = ScanProgress {
        phase: Phase::ReadingMft,
        current: req.root.clone(),
        ..Default::default()
    };
    progress(&prog);

    // Record 0 = $MFT selbst; dessen Runlist beschreibt, wo die MFT liegt.
    // Lesen mindestens einen Cluster (sektor-ausgerichtet), Record 0 steht am Anfang.
    let first_len = (rec_size as u64).max(cluster) as usize;
    let mut first = vec![0u8; first_len];
    vol.read_at(nd.MftStartLcn as u64 * cluster, &mut first)?;
    let rec0 = &mut first[..rec_size];
    mft_parse::apply_fixup(rec0, sector)?;
    let runs = mft_parse::unnamed_data_runs(rec0)?;
    let total = nd.MftValidDataLength.max(0) as u64;
    let plan = mft_parse::plan_reads(&runs, cluster, total, CHUNK);

    let mut records: Vec<MftRecord> = Vec::with_capacity((total / rec_size as u64) as usize);
    let mut errors = 0u32;
    let mut buf = vec![0u8; CHUNK as usize];
    for c in &plan {
        if cancel.load(Ordering::Relaxed) {
            return Err(ScanError::Cancelled);
        }
        let len = c.len as usize;
        // Länge auf ganze Records kürzen (letzter Block kann krumm sein).
        let len = len - len % rec_size;
        if len == 0 {
            continue;
        }
        let b = &mut buf[..len];
        if vol.read_at(c.disk_offset, b).is_err() {
            errors = errors.saturating_add((len / rec_size) as u32);
            continue;
        }
        let first_no = c.mft_offset / rec_size as u64;
        let (mut recs, errs) = mft_parse::parse_chunk(b, first_no, rec_size, sector);
        errors = errors.saturating_add(errs);
        prog.bytes += recs.iter().map(|r| r.size).sum::<u64>();
        records.append(&mut recs);
        prog.entries = records.len() as u64;
        progress(&prog);
    }
    drop(vol);

    prog.phase = Phase::Building;
    progress(&prog);
    let mut meta = ScanMeta::new(req.root.clone(), ScanMode::Mft);
    meta.started = started;
    meta.cluster_size = cluster as u32;
    meta.volume_total = (nd.TotalClusters.max(0) as u64).saturating_mul(cluster);
    meta.volume_free = (nd.FreeClusters.max(0) as u64).saturating_mul(cluster);
    meta.errors = errors;
    let root_name = format!("{letter}:\\");
    let mut tree = mft_parse::records_to_tree(records.into_iter(), meta, &root_name);
    tree.meta.duration_ms = t0.elapsed().as_millis() as u64;
    prog.phase = Phase::Done;
    progress(&prog);
    Ok(tree)
}
