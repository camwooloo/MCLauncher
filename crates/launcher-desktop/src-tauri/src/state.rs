//! Shared application state managed by Tauri.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::Arc;

use launcher_core::paths::Paths;
use tokio::process::ChildStdin;
use tokio::sync::{oneshot, Mutex};

/// A running hosted Minecraft server instance.
///
/// The child process is owned by a background wait-task (so we can capture its
/// exit code); we keep its stdin for commands and a one-shot to force-kill it.
pub struct ServerProc {
    pub id: String,
    pub name: String,
    pub version: String,
    pub port: u16,
    pub max_players: u32,
    /// OS process id — used to force-kill on launcher exit so servers never orphan.
    pub pid: u32,
    pub stdin: ChildStdin,
    /// Send to force-kill the process (taken on stop).
    pub kill: Option<oneshot::Sender<()>>,
    /// Connected player names (updated by the log-reader task).
    pub players: Arc<Mutex<HashSet<String>>>,
    /// Latest sampled resident memory in MiB (updated by the sampler task).
    pub memory_mb: Arc<AtomicU64>,
    /// Cleared when the process exits.
    pub running: Arc<AtomicBool>,
    /// Rolling console history so reopening the dashboard replays past output.
    pub log: Arc<std::sync::Mutex<Vec<crate::commands::ServerLogLine>>>,
    /// Whether the process stops gracefully via a `stop` stdin command
    /// (Minecraft). Native servers like Skyrim Together are force-killed instead.
    pub graceful_stop: bool,
}

/// Process-wide launcher state.
pub struct AppState {
    pub paths: Paths,
    /// Running servers keyed by config id.
    pub servers: Mutex<HashMap<String, ServerProc>>,
}

impl AppState {
    pub fn new() -> Self {
        let paths = Paths::discover()
            .unwrap_or_else(|_| Paths::with_dirs("./.minecraft", "./aurora-data"));
        Self {
            paths,
            servers: Mutex::new(HashMap::new()),
        }
    }
}
