// Joschys_Tree – Windows-Plattformfunktionen (Admin, Papierkorb, ACL, ADS, lange Pfade) – @ Joschy
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Nur unter Windows kompiliert. Logik (Pfad-Umwandlung, Rechte-Text, Stream-Namen)
//! liegt plattformneutral in `platform/mod.rs`; hier stehen nur die API-Aufrufe.

use std::ffi::c_void;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::process::Command;

use windows::Win32::Foundation::{
    CloseHandle, ERROR_HANDLE_EOF, ERROR_SUCCESS, HANDLE, HLOCAL, HWND, LocalFree,
};
use windows::Win32::Security::Authorization::{
    ConvertSidToStringSidW, GetNamedSecurityInfoW, SE_FILE_OBJECT,
};
use windows::Win32::Security::{
    ACCESS_ALLOWED_ACE, ACE_HEADER, ACL, DACL_SECURITY_INFORMATION, GetAce, GetTokenInformation,
    LookupAccountSidW, OWNER_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, PSID, SID_NAME_USE,
    TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation,
};
use windows::Win32::Storage::FileSystem::{
    FindClose, FindFirstStreamW, FindNextStreamW, FindStreamInfoStandard, GetCompressedFileSizeW,
    GetDiskFreeSpaceExW, GetDiskFreeSpaceW, GetLogicalDrives, GetVolumeInformationW,
    GetVolumePathNameW, INVALID_FILE_SIZE, WIN32_FIND_STREAM_DATA,
};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::Win32::UI::Shell::{
    FO_DELETE, FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_NOERRORUI, FOF_SILENT, SHFILEOPSTRUCTW,
    SHFileOperationW,
};
use windows::core::{PCWSTR, PWSTR};

use super::{Ace, SecurityInfo, StreamInfo};

/// UTF-16 mit abschließender Null.
fn wide(s: &str) -> Vec<u16> {
    std::ffi::OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

fn from_wide(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

fn other(msg: &str) -> io::Error {
    io::Error::other(msg.to_string())
}

/// Schließt ein HANDLE beim Verlassen des Gültigkeitsbereichs.
struct OwnedHandle(HANDLE);
impl Drop for OwnedHandle {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            // SAFETY: Handle stammt aus einer erfolgreichen API und wird nur hier geschlossen.
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
}

/// Gibt einen per LocalAlloc reservierten Speicher frei.
struct LocalMem(*mut c_void);
impl Drop for LocalMem {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: Zeiger stammt von einer API, die LocalFree verlangt.
            unsafe {
                let _ = LocalFree(Some(HLOCAL(self.0)));
            }
        }
    }
}

pub fn is_admin() -> bool {
    // SAFETY: Standardmuster; alle Zeiger zeigen auf lokale Variablen passender Größe.
    unsafe {
        let mut token = HANDLE::default();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
            return false;
        }
        let token = OwnedHandle(token);
        let mut elev = TOKEN_ELEVATION::default();
        let mut ret = 0u32;
        let ok = GetTokenInformation(
            token.0,
            TokenElevation,
            Some(&mut elev as *mut TOKEN_ELEVATION as *mut c_void),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut ret,
        );
        ok.is_ok() && elev.TokenIsElevated != 0
    }
}

/// Doppelt nullterminierte Liste für SHFileOperationW (ohne `\\?\`, das kennt die Shell nicht).
fn multi_sz(paths: &[String]) -> Vec<u16> {
    let mut v: Vec<u16> = Vec::new();
    for p in paths {
        v.extend(std::ffi::OsStr::new(&super::strip_long_prefix(p)).encode_wide());
        v.push(0);
    }
    v.push(0);
    v
}

fn shell_delete(paths: &[String], recycle: bool) -> io::Result<()> {
    if paths.is_empty() {
        return Ok(());
    }
    let from = multi_sz(paths);
    let mut flags = FOF_NOCONFIRMATION | FOF_SILENT | FOF_NOERRORUI;
    if recycle {
        flags |= FOF_ALLOWUNDO;
    }
    let mut op = SHFILEOPSTRUCTW {
        hwnd: HWND::default(),
        wFunc: FO_DELETE,
        pFrom: PCWSTR(from.as_ptr()),
        pTo: PCWSTR::null(),
        fFlags: flags.0 as u16,
        ..Default::default()
    };
    // SAFETY: `from` lebt bis nach dem Aufruf; Struktur vollständig initialisiert.
    let rc = unsafe { SHFileOperationW(&mut op) };
    if rc != 0 {
        return Err(other(&format!("SHFileOperationW Fehler 0x{rc:X}")));
    }
    if op.fAnyOperationsAborted.as_bool() {
        return Err(other("Löschen abgebrochen"));
    }
    Ok(())
}

pub fn move_to_recycle_bin(paths: &[String]) -> io::Result<()> {
    shell_delete(paths, true)
}

pub fn delete_permanently(paths: &[String]) -> io::Result<()> {
    // Lange Pfade direkt über std::fs (unterstützt \\?\), Shell nur als Fallback.
    for p in paths {
        let lp = long_path(p);
        let md = std::fs::symlink_metadata(&lp)?;
        let r = if md.file_type().is_symlink() {
            // Link selbst entfernen, nie das Ziel (Ordner-Links brauchen remove_dir).
            std::fs::remove_dir(&lp).or_else(|_| std::fs::remove_file(&lp))
        } else if md.is_dir() {
            // remove_dir_all folgt Junctions/Symlinks nicht.
            std::fs::remove_dir_all(&lp)
        } else {
            std::fs::remove_file(&lp)
        };
        if r.is_err() {
            shell_delete(std::slice::from_ref(p), false)?;
        }
    }
    Ok(())
}

/// explorer /select,"<pfad>"
pub fn open_in_explorer(path: &str) -> io::Result<()> {
    let p = super::strip_long_prefix(path);
    Command::new("explorer.exe")
        .arg(format!("/select,{p}"))
        .spawn()
        .map(|_| ())
}

/// SID → "DOMÄNE\Name", ersatzweise "S-1-5-…".
fn sid_to_name(sid: PSID) -> String {
    if sid.0.is_null() {
        return String::new();
    }
    let mut name = [0u16; 256];
    let mut domain = [0u16; 256];
    let mut nlen = name.len() as u32;
    let mut dlen = domain.len() as u32;
    let mut use_ = SID_NAME_USE::default();
    // SAFETY: Puffer und Längen passen zusammen.
    let ok = unsafe {
        LookupAccountSidW(
            PCWSTR::null(),
            sid,
            Some(PWSTR(name.as_mut_ptr())),
            &mut nlen,
            Some(PWSTR(domain.as_mut_ptr())),
            &mut dlen,
            &mut use_,
        )
    };
    if ok.is_ok() {
        let n = from_wide(&name);
        let d = from_wide(&domain);
        return if d.is_empty() { n } else { format!("{d}\\{n}") };
    }
    let mut s = PWSTR::null();
    // SAFETY: ConvertSidToStringSidW reserviert per LocalAlloc; wird über LocalMem freigegeben.
    unsafe {
        if ConvertSidToStringSidW(sid, &mut s).is_ok() && !s.is_null() {
            let _mem = LocalMem(s.0 as *mut c_void);
            return s.to_string().unwrap_or_default();
        }
    }
    String::from("?")
}

pub fn security_info(path: &str) -> io::Result<SecurityInfo> {
    let w = wide(&long_path(path));
    let mut owner = PSID::default();
    let mut dacl: *mut ACL = std::ptr::null_mut();
    let mut sd = PSECURITY_DESCRIPTOR::default();
    // SAFETY: Ausgabezeiger auf lokale Variablen; Deskriptor wird per LocalFree freigegeben.
    let rc = unsafe {
        GetNamedSecurityInfoW(
            PCWSTR(w.as_ptr()),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            Some(&mut owner),
            None,
            Some(&mut dacl),
            None,
            &mut sd,
        )
    };
    if rc != ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(rc.0 as i32));
    }
    let _sd_mem = LocalMem(sd.0);
    let mut info = SecurityInfo {
        owner: sid_to_name(owner),
        aces: Vec::new(),
    };
    if dacl.is_null() {
        return Ok(info); // NULL-DACL: jeder hat Vollzugriff
    }
    // SAFETY: dacl zeigt in den Deskriptor, der bis zum Ende dieser Funktion lebt.
    let count = unsafe { (*dacl).AceCount };
    for i in 0..count as u32 {
        let mut ace: *mut c_void = std::ptr::null_mut();
        // SAFETY: Index < AceCount.
        if unsafe { GetAce(dacl, i, &mut ace) }.is_err() || ace.is_null() {
            continue;
        }
        // SAFETY: Jede ACE beginnt mit ACE_HEADER.
        let header = unsafe { *(ace as *const ACE_HEADER) };
        // Nur einfache Allow (0) / Deny (1) ACEs haben das Layout ACCESS_ALLOWED_ACE.
        if header.AceType > 1 {
            continue;
        }
        let a = ace as *const ACCESS_ALLOWED_ACE;
        // SAFETY: Typ geprüft; SidStart ist der Beginn der eingebetteten SID.
        let (mask, sid) = unsafe {
            (
                (*a).Mask,
                PSID(std::ptr::addr_of!((*a).SidStart) as *mut c_void),
            )
        };
        info.aces.push(Ace {
            principal: sid_to_name(sid),
            allow: header.AceType == 0,
            rights: super::format_access_mask(mask),
            inherited: header.AceFlags & 0x10 != 0, // INHERITED_ACE
        });
    }
    Ok(info)
}

pub fn alternate_streams(path: &str) -> io::Result<Vec<StreamInfo>> {
    let w = wide(&long_path(path));
    let mut data = WIN32_FIND_STREAM_DATA::default();
    // SAFETY: data hat das von FindStreamInfoStandard verlangte Layout.
    let h = unsafe {
        FindFirstStreamW(
            PCWSTR(w.as_ptr()),
            FindStreamInfoStandard,
            &mut data as *mut WIN32_FIND_STREAM_DATA as *mut c_void,
            None,
        )
    };
    let h = match h {
        Ok(h) => h,
        Err(e) if e.code() == ERROR_HANDLE_EOF.to_hresult() => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };
    let mut out = Vec::new();
    loop {
        if let Some(name) = super::stream_display_name(&from_wide(&data.cStreamName)) {
            out.push(StreamInfo {
                name,
                size: data.StreamSize.max(0) as u64,
            });
        }
        // SAFETY: Handle aus FindFirstStreamW.
        if unsafe { FindNextStreamW(h, &mut data as *mut WIN32_FIND_STREAM_DATA as *mut c_void) }
            .is_err()
        {
            break;
        }
    }
    // SAFETY: Handle aus FindFirstStreamW, wird genau einmal geschlossen.
    unsafe {
        let _ = FindClose(h);
    }
    Ok(out)
}

/// Volume-Wurzel ("C:\", "\\server\share\") für einen beliebigen Pfad.
fn volume_root(root: &str) -> String {
    let w = wide(&super::strip_long_prefix(root));
    let mut buf = [0u16; 1024];
    // SAFETY: Puffergröße wird über den Slice übergeben.
    if unsafe { GetVolumePathNameW(PCWSTR(w.as_ptr()), &mut buf) }.is_ok() {
        let s = from_wide(&buf);
        if !s.is_empty() {
            return s;
        }
    }
    let mut s = super::strip_long_prefix(root);
    if !s.ends_with('\\') {
        s.push('\\');
    }
    s
}

/// (Dateisystem, Clustergröße, Gesamt, Frei)
pub fn volume_info(root: &str) -> io::Result<(String, u32, u64, u64)> {
    let vol = volume_root(root);
    let w = wide(&vol);
    let mut fs_name = [0u16; 64];
    // SAFETY: Puffer über Slices; nicht benötigte Ausgaben sind None.
    unsafe {
        GetVolumeInformationW(
            PCWSTR(w.as_ptr()),
            None,
            None,
            None,
            None,
            Some(&mut fs_name),
        )?;
    }
    let (mut spc, mut bps, mut free_cl, mut total_cl) = (0u32, 0u32, 0u32, 0u32);
    // SAFETY: Ausgaben auf lokale u32.
    let cluster = unsafe {
        GetDiskFreeSpaceW(
            PCWSTR(w.as_ptr()),
            Some(&mut spc),
            Some(&mut bps),
            Some(&mut free_cl),
            Some(&mut total_cl),
        )
    }
    .map(|_| spc.saturating_mul(bps))
    .unwrap_or(0);
    let (mut avail, mut total, mut free) = (0u64, 0u64, 0u64);
    // SAFETY: Ausgaben auf lokale u64.
    unsafe {
        GetDiskFreeSpaceExW(
            PCWSTR(w.as_ptr()),
            Some(&mut avail),
            Some(&mut total),
            Some(&mut free),
        )?;
    }
    Ok((from_wide(&fs_name), cluster, total, free))
}

/// Belegte Größe komprimierter/sparse Dateien (für den Walk).
pub(crate) fn compressed_size(path: &str) -> io::Result<u64> {
    let w = wide(&long_path(path));
    let mut high = 0u32;
    // SAFETY: high ist eine lokale Variable.
    let low = unsafe { GetCompressedFileSizeW(PCWSTR(w.as_ptr()), Some(&mut high)) };
    if low == INVALID_FILE_SIZE {
        let e = io::Error::last_os_error();
        if e.raw_os_error().unwrap_or(0) != 0 {
            return Err(e);
        }
    }
    Ok(((high as u64) << 32) | low as u64)
}

/// "\\?\C:\..." bzw. "\\?\UNC\server\share\..."
pub fn long_path(path: &str) -> String {
    super::to_long_path(path)
}

pub fn list_drives() -> Vec<String> {
    // SAFETY: keine Parameter.
    let mask = unsafe { GetLogicalDrives() };
    super::drives_from_mask(mask)
}
