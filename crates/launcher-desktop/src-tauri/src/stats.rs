//! Lightweight, local play-stats: last-played, launch count, and total
//! playtime per launchable (instances + native games). Stored in
//! `<data>/stats.json`. No network, no accounts.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayRecord {
    /// Stable key, e.g. `instance:<id>` or `game:skyrim`.
    pub key: String,
    pub name: String,
    /// "instance" | "skyrim" | "eldenring" | "cyberpunk".
    pub kind: String,
    #[serde(default)]
    pub icon: Option<String>,
    /// Last launch (unix seconds).
    pub last_played: u64,
    pub total_seconds: u64,
    pub launches: u32,
}

#[derive(Default, Serialize, Deserialize)]
struct StatsFile {
    #[serde(default)]
    entries: HashMap<String, PlayRecord>,
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}
fn path(data_dir: &Path) -> PathBuf {
    data_dir.join("stats.json")
}
fn load(data_dir: &Path) -> StatsFile {
    std::fs::read(path(data_dir))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}
fn store(data_dir: &Path, f: &StatsFile) {
    if let Some(parent) = path(data_dir).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(bytes) = serde_json::to_vec_pretty(f) {
        let _ = std::fs::write(path(data_dir), bytes);
    }
}

/// Record a launch (bumps last-played + launch count) and spawn a background
/// thread that adds the session's playtime once the process exits. Sessions
/// under 30s are ignored (failed launches / quick-exiting loaders). Uses a
/// thread (not tokio) so it works from sync and async commands alike.
pub fn record_session(
    data_dir: &Path,
    key: &str,
    name: &str,
    kind: &str,
    icon: Option<String>,
    pid: u32,
) {
    {
        let mut f = load(data_dir);
        let e = f.entries.entry(key.to_string()).or_default();
        e.key = key.to_string();
        e.name = name.to_string();
        e.kind = kind.to_string();
        if icon.is_some() {
            e.icon = icon;
        }
        e.last_played = now_secs();
        e.launches += 1;
        store(data_dir, &f);
    }
    if pid == 0 {
        return;
    }
    let dir = data_dir.to_path_buf();
    let key = key.to_string();
    std::thread::spawn(move || {
        use sysinfo::{Pid, ProcessesToUpdate, System};
        let start = Instant::now();
        let p = Pid::from_u32(pid);
        let mut sys = System::new();
        loop {
            std::thread::sleep(Duration::from_secs(15));
            sys.refresh_processes(ProcessesToUpdate::Some(&[p]), true);
            if sys.process(p).is_none() {
                break;
            }
            if start.elapsed() > Duration::from_secs(12 * 3600) {
                break; // safety cap against a recycled PID
            }
        }
        let secs = start.elapsed().as_secs();
        if secs >= 30 {
            let mut f = load(&dir);
            if let Some(e) = f.entries.get_mut(&key) {
                e.total_seconds += secs;
                store(&dir, &f);
            }
        }
    });
}

/// All play records, most-recently-played first.
pub fn list(data_dir: &Path) -> Vec<PlayRecord> {
    let mut v: Vec<PlayRecord> = load(data_dir).entries.into_values().collect();
    v.sort_by(|a, b| b.last_played.cmp(&a.last_played));
    v
}
