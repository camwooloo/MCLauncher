//! Elden Ring **in-app trainer** — live memory patching for offline/co-op
//! cinematic cheats (god mode, etc.). Built-in: no external trainer needed.
//!
//! **Fail-closed by design:** a cheat only writes when its byte signature (AOB)
//! is found in the running game. If a game patch shifts the bytes, the scan
//! simply misses and the cheat is a no-op — it never writes to a wrong address,
//! so it can't crash the game. Signatures are tuned live (`er_aob_test`).
//!
//! Offline / co-op only (anti-cheat off) — same rule as Seamless Co-op.

#![cfg(windows)]

use std::collections::HashMap;
use std::sync::Mutex;

use windows::core::Result as WResult;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::Diagnostics::Debug::{ReadProcessMemory, WriteProcessMemory};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Module32FirstW, Process32FirstW, Process32NextW, MODULEENTRY32W,
    PROCESSENTRY32W, TH32CS_SNAPMODULE, TH32CS_SNAPMODULE32, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Memory::{
    VirtualProtectEx, PAGE_EXECUTE_READWRITE, PAGE_PROTECTION_FLAGS,
};
use windows::Win32::System::Threading::{OpenProcess, PROCESS_ALL_ACCESS};

const PROCESS_NAME: &str = "eldenring.exe";

/// A cheat = a signature to find, an offset into the match, and bytes to write.
struct Cheat {
    id: &'static str,
    /// AOB with `??` wildcards, e.g. "0F 28 ?? ?? 48 8B". Empty = not tuned yet.
    aob: &'static str,
    /// Where to write, relative to the match start.
    offset: isize,
    /// Bytes written when enabled (e.g. NOPs / a jump).
    patch: &'static [u8],
}

// Starting signatures. These are best-effort placeholders to be CONFIRMED live
// against the running game (use `er_aob_test`); fail-closed means an unverified
// one just no-ops. We fill/refine these together per game patch.
const CHEATS: &[Cheat] = &[
    Cheat { id: "god", aob: "", offset: 0, patch: &[] },
    Cheat { id: "stamina", aob: "", offset: 0, patch: &[] },
    Cheat { id: "fp", aob: "", offset: 0, patch: &[] },
    Cheat { id: "keepRunes", aob: "", offset: 0, patch: &[] },
];

/// Bytes we overwrote, so a cheat can be turned back off.
struct Applied {
    addr: usize,
    original: Vec<u8>,
}

static APPLIED: Mutex<Option<HashMap<String, Applied>>> = Mutex::new(None);

fn applied() -> std::sync::MutexGuard<'static, Option<HashMap<String, Applied>>> {
    let mut g = APPLIED.lock().unwrap();
    if g.is_none() {
        *g = Some(HashMap::new());
    }
    g
}

/// Find the Elden Ring process id by executable name.
fn find_pid() -> Option<u32> {
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0).ok()?;
        let mut entry = PROCESSENTRY32W { dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
        let mut found = None;
        if Process32FirstW(snap, &mut entry).is_ok() {
            loop {
                let name = String::from_utf16_lossy(&entry.szExeFile);
                let name = name.trim_end_matches('\u{0}');
                if name.eq_ignore_ascii_case(PROCESS_NAME) {
                    found = Some(entry.th32ProcessID);
                    break;
                }
                if Process32NextW(snap, &mut entry).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snap);
        found
    }
}

/// Base address + size of the main module for `pid`.
fn module_range(pid: u32) -> Option<(usize, usize)> {
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, pid).ok()?;
        let mut entry = MODULEENTRY32W { dwSize: std::mem::size_of::<MODULEENTRY32W>() as u32, ..Default::default() };
        let res = Module32FirstW(snap, &mut entry);
        let _ = CloseHandle(snap);
        res.ok()?;
        Some((entry.modBaseAddr as usize, entry.modBaseSize as usize))
    }
}

struct Proc {
    handle: HANDLE,
    base: usize,
    size: usize,
}
impl Drop for Proc {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}

fn attach() -> Result<Proc, String> {
    let pid = find_pid().ok_or("Elden Ring isn't running — launch it (Co-op or Modded) first.")?;
    let (base, size) = module_range(pid).ok_or("Couldn't read the game's memory layout.")?;
    let handle = unsafe { OpenProcess(PROCESS_ALL_ACCESS, false, pid) }
        .map_err(|_| "Couldn't open Elden Ring (try running Aurora as admin).".to_string())?;
    Ok(Proc { handle, base, size })
}

fn read(p: &Proc, addr: usize, buf: &mut [u8]) -> WResult<()> {
    unsafe {
        ReadProcessMemory(
            p.handle,
            addr as *const _,
            buf.as_mut_ptr() as *mut _,
            buf.len(),
            None,
        )
    }
}

fn write(p: &Proc, addr: usize, bytes: &[u8]) -> Result<(), String> {
    unsafe {
        let mut old = PAGE_PROTECTION_FLAGS(0);
        VirtualProtectEx(p.handle, addr as *const _, bytes.len(), PAGE_EXECUTE_READWRITE, &mut old)
            .map_err(|e| e.to_string())?;
        let r = WriteProcessMemory(p.handle, addr as *const _, bytes.as_ptr() as *const _, bytes.len(), None);
        let mut restore = PAGE_PROTECTION_FLAGS(0);
        let _ = VirtualProtectEx(p.handle, addr as *const _, bytes.len(), old, &mut restore);
        r.map_err(|e| e.to_string())
    }
}

/// Parse an AOB string ("48 8B ?? 89") into match bytes (None = wildcard).
fn parse_aob(aob: &str) -> Option<Vec<Option<u8>>> {
    if aob.trim().is_empty() {
        return None;
    }
    let mut out = Vec::new();
    for tok in aob.split_whitespace() {
        if tok == "??" || tok == "?" {
            out.push(None);
        } else {
            out.push(Some(u8::from_str_radix(tok, 16).ok()?));
        }
    }
    (!out.is_empty()).then_some(out)
}

/// Scan the main module for `pattern`; return the absolute address of the match.
fn scan(p: &Proc, pattern: &[Option<u8>]) -> Option<usize> {
    // Read the whole module image once (ER's is large but this is a one-shot).
    let mut buf = vec![0u8; p.size];
    if read(p, p.base, &mut buf).is_err() {
        // Fall back to chunked reads if a single read fails.
        let chunk = 1 << 20;
        let mut ok = false;
        let mut off = 0;
        while off < p.size {
            let end = (off + chunk).min(p.size);
            if read(p, p.base + off, &mut buf[off..end]).is_ok() {
                ok = true;
            }
            off = end;
        }
        if !ok {
            return None;
        }
    }
    let n = pattern.len();
    if n == 0 || buf.len() < n {
        return None;
    }
    'outer: for i in 0..=buf.len() - n {
        for (j, want) in pattern.iter().enumerate() {
            if let Some(b) = want {
                if buf[i + j] != *b {
                    continue 'outer;
                }
            }
        }
        return Some(p.base + i);
    }
    None
}

// --- Tauri commands ------------------------------------------------------

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrainerStatus {
    pub running: bool,
    pub applied: Vec<String>,
}

#[tauri::command]
pub fn er_cheat_status() -> TrainerStatus {
    let running = find_pid().is_some();
    let applied = applied().as_ref().map(|m| m.keys().cloned().collect()).unwrap_or_default();
    TrainerStatus { running, applied }
}

/// Enable/disable one cheat against the running game.
#[tauri::command]
pub fn er_cheat_set(id: String, enabled: bool) -> Result<(), String> {
    let cheat = CHEATS.iter().find(|c| c.id == id).ok_or("Unknown cheat")?;
    let p = attach()?;

    if enabled {
        let pattern = parse_aob(cheat.aob)
            .ok_or("This cheat's signature isn't tuned for your game version yet.")?;
        let at = scan(&p, &pattern).ok_or(
            "Couldn't find this cheat in memory — your Elden Ring version may differ from the signature (we'll tune it).",
        )?;
        let target = (at as isize + cheat.offset) as usize;
        let mut original = vec![0u8; cheat.patch.len()];
        read(&p, target, &mut original).map_err(|e| e.to_string())?;
        write(&p, target, cheat.patch)?;
        if let Some(m) = applied().as_mut() {
            m.insert(id, Applied { addr: target, original });
        }
        Ok(())
    } else {
        let entry = applied().as_mut().and_then(|m| m.remove(&id));
        if let Some(a) = entry {
            write(&p, a.addr, &a.original)?;
        }
        Ok(())
    }
}

/// Live signature tuning: report whether an AOB matches the running game and
/// where. Lets us confirm/refine cheat signatures interactively.
#[tauri::command]
pub fn er_aob_test(aob: String) -> Result<serde_json::Value, String> {
    let pattern = parse_aob(&aob).ok_or("Empty or invalid AOB")?;
    let p = attach()?;
    match scan(&p, &pattern) {
        Some(addr) => Ok(serde_json::json!({ "found": true, "offset": format!("{:#x}", addr - p.base) })),
        None => Ok(serde_json::json!({ "found": false })),
    }
}
