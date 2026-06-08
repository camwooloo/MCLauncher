//! State shared between the async worker (install/launch/login) and the UI.
//!
//! Rather than thread a stream of progress events through iced, the worker
//! writes into this `Arc`-shared struct (atomics + small mutexes) and a 10 Hz
//! timer subscription re-renders the view, which reads the latest values. This
//! keeps the async code free of UI concerns — it just sees a
//! `launcher_core::progress::Reporter`.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use launcher_core::progress::Reporter;

#[derive(Default)]
pub struct Shared {
    total: AtomicU64,
    done: AtomicU64,
    stage_msg: Mutex<String>,
    /// Device-code prompt: (user_code, verification_uri), set during MS login.
    login_prompt: Mutex<Option<(String, String)>>,
}

impl Shared {
    /// Reset counters/stage at the start of a new operation.
    pub fn begin(&self, stage: &str) {
        self.total.store(0, Ordering::Relaxed);
        self.done.store(0, Ordering::Relaxed);
        *self.stage_msg.lock().unwrap() = stage.to_string();
        *self.login_prompt.lock().unwrap() = None;
    }

    pub fn current_stage(&self) -> String {
        self.stage_msg.lock().unwrap().clone()
    }

    /// Fraction complete in `0.0..=1.0`; 0 when total is unknown.
    pub fn fraction(&self) -> f32 {
        let total = self.total.load(Ordering::Relaxed);
        if total == 0 {
            return 0.0;
        }
        let done = self.done.load(Ordering::Relaxed);
        (done as f32 / total as f32).clamp(0.0, 1.0)
    }

    pub fn set_login_prompt(&self, user_code: String, uri: String) {
        *self.login_prompt.lock().unwrap() = Some((user_code, uri));
    }

    pub fn login_prompt(&self) -> Option<(String, String)> {
        self.login_prompt.lock().unwrap().clone()
    }

    pub fn clear_login_prompt(&self) {
        *self.login_prompt.lock().unwrap() = None;
    }
}

impl Reporter for Shared {
    fn stage(&self, name: &str) {
        // A new stage resets the byte counters so each stage shows 0→100%.
        self.total.store(0, Ordering::Relaxed);
        self.done.store(0, Ordering::Relaxed);
        *self.stage_msg.lock().unwrap() = name.to_string();
    }
    fn set_total_bytes(&self, total: u64) {
        self.total.store(total, Ordering::Relaxed);
    }
    fn add_bytes(&self, n: u64) {
        self.done.fetch_add(n, Ordering::Relaxed);
    }
}
