//! A progress reporter that surfaces install progress to the web UI.
//!
//! `launcher_core` reporters are called from many download tasks at high
//! frequency, so emitting a Tauri event per byte would flood the IPC channel.
//! Instead this accumulates into atomics, and the play command runs a ~10 Hz
//! pump task that snapshots and emits `"mc-progress"` events.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use launcher_core::progress::Reporter;
use serde::Serialize;

#[derive(Default)]
pub struct EventReporter {
    total: AtomicU64,
    done: AtomicU64,
    stage: Mutex<String>,
}

/// Snapshot payload emitted to the UI.
#[derive(Debug, Clone, Serialize)]
pub struct ProgressSnapshot {
    pub stage: String,
    pub total: u64,
    pub done: u64,
    pub fraction: f32,
}

impl EventReporter {
    pub fn snapshot(&self) -> ProgressSnapshot {
        let total = self.total.load(Ordering::Relaxed);
        let done = self.done.load(Ordering::Relaxed);
        let fraction = if total == 0 {
            0.0
        } else {
            (done as f32 / total as f32).clamp(0.0, 1.0)
        };
        ProgressSnapshot {
            stage: self.stage.lock().unwrap().clone(),
            total,
            done,
            fraction,
        }
    }
}

impl Reporter for EventReporter {
    fn stage(&self, name: &str) {
        self.total.store(0, Ordering::Relaxed);
        self.done.store(0, Ordering::Relaxed);
        *self.stage.lock().unwrap() = name.to_string();
    }
    fn set_total_bytes(&self, total: u64) {
        self.total.store(total, Ordering::Relaxed);
    }
    fn add_bytes(&self, n: u64) {
        self.done.fetch_add(n, Ordering::Relaxed);
    }
}
