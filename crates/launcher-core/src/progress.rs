//! Progress reporting.
//!
//! Long-running operations (downloading, extracting) report through a
//! [`Reporter`]. The core never assumes a UI: a CLI can print, the iced GUI can
//! forward events to a channel, and tests can use [`NoopReporter`].
//!
//! Reporters are shared across many concurrent download tasks, so all methods
//! take `&self` and the type must be `Send + Sync`. Implementations are
//! expected to be cheap and non-blocking (e.g. atomic adds, channel sends).

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Sink for progress events.
pub trait Reporter: Send + Sync {
    /// A new named stage has begun (e.g. "Downloading libraries").
    fn stage(&self, _name: &str) {}
    /// The total byte count for the current operation is now known.
    fn set_total_bytes(&self, _total: u64) {}
    /// `n` more bytes have been downloaded/processed.
    fn add_bytes(&self, _n: u64) {}
    /// One discrete item (file) finished.
    fn item_done(&self) {}
}

/// A reporter that discards everything. Useful in tests and headless paths.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopReporter;
impl Reporter for NoopReporter {}

/// A shareable reporter handle.
pub type SharedReporter = Arc<dyn Reporter>;

/// Returns a no-op shared reporter.
pub fn noop() -> SharedReporter {
    Arc::new(NoopReporter)
}

/// A simple in-memory counter reporter, handy for CLIs and tests: it tracks
/// total/done bytes atomically and can be polled.
#[derive(Debug, Default)]
pub struct CountingReporter {
    pub total: AtomicU64,
    pub done: AtomicU64,
}

impl Reporter for CountingReporter {
    fn set_total_bytes(&self, total: u64) {
        self.total.store(total, Ordering::Relaxed);
    }
    fn add_bytes(&self, n: u64) {
        self.done.fetch_add(n, Ordering::Relaxed);
    }
}

impl CountingReporter {
    /// Fraction complete in `0.0..=1.0`, or `0.0` if the total is unknown.
    pub fn fraction(&self) -> f32 {
        let total = self.total.load(Ordering::Relaxed);
        if total == 0 {
            return 0.0;
        }
        let done = self.done.load(Ordering::Relaxed);
        (done as f32 / total as f32).clamp(0.0, 1.0)
    }
}
