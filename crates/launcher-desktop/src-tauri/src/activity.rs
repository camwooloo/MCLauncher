//! Kawma "Now Playing" and "Play with your community". Both are optional (off
//! by default in Settings), need the Kawma sign-in, and are gated per account
//! by kawma.app releases (`launcher-now-playing`, `launcher-community`).
//!
//! Now Playing: when a game launches through the launcher, post it to Kawma
//! and re-post every 5 minutes while it runs (Kawma forgets it 15 minutes
//! after the last post), then clear it when the game exits.
//!
//! Community: when a hosted Minecraft server is shared to the internet (bore
//! tunnel), post the session; delete it when sharing or the server stops.
//! While a whitelisted session is open, poll Kawma for viewers' whitelist
//! requests and run `whitelist add <name>` on the server console.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::json;
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::AsyncWriteExt;

use crate::kawma::{self, Reply};
use crate::settings::Settings;
use crate::state::AppState;

const NOW_PLAYING_RELEASE: &str = "launcher-now-playing";
const COMMUNITY_RELEASE: &str = "launcher-community";
const REPOST_EVERY: Duration = Duration::from_secs(5 * 60);
const CHECK_EVERY: Duration = Duration::from_secs(30);
/// Launchers (Steam, SKSE, the Seamless launcher) hand over to the real game
/// process; don't call a game closed during its first minute and a half.
const START_GRACE: Duration = Duration::from_secs(90);
const REQUEST_POLL: Duration = Duration::from_secs(10);

fn settings(app: &AppHandle) -> Settings {
    Settings::load_sync(&app.state::<AppState>().paths.settings_file())
}

fn clip(s: &str, max: usize) -> String {
    s.trim().chars().take(max).collect()
}

// --- Now Playing -------------------------------------------------------------

/// Bumped for every launch (and on sign-out); a watcher only reports while
/// its own generation is current, so the newest game wins.
static PLAYING: AtomicU64 = AtomicU64::new(0);

/// The game's own process names, for games launched through Steam or a
/// helper (where the pid we have isn't the game, or there is none).
pub(crate) fn game_processes(game: &str) -> &'static [&'static str] {
    match game {
        "skyrim" => &["SkyrimSE.exe"],
        "eldenring" => &["eldenring.exe"],
        "cyberpunk" => &["Cyberpunk2077.exe"],
        "deadlock" => &["deadlock.exe"],
        _ => &[],
    }
}

fn still_running(sys: &mut sysinfo::System, pid: u32, game: &str) -> bool {
    use sysinfo::{Pid, ProcessesToUpdate};
    let names = game_processes(game);
    if names.is_empty() {
        if pid == 0 {
            return false;
        }
        let p = Pid::from_u32(pid);
        sys.refresh_processes(ProcessesToUpdate::Some(&[p]), true);
        return sys.process(p).is_some();
    }
    sys.refresh_processes(ProcessesToUpdate::All, true);
    (pid != 0 && sys.process(Pid::from_u32(pid)).is_some())
        || sys
            .processes()
            .values()
            .any(|p| names.iter().any(|n| p.name().eq_ignore_ascii_case(n)))
}

/// `Ok(true)` posted, `Ok(false)` not available for this account (stop trying).
async fn post_activity(body: &serde_json::Value) -> Result<bool, String> {
    match kawma::request(reqwest::Method::POST, "/auth/desktop/launcher/activity", Some(body)).await? {
        Reply::Ok(_) => Ok(true),
        Reply::Status(401) | Reply::Status(404) => Ok(false),
        Reply::Status(s) => Err(format!("Kawma answered {s}")),
    }
}

async fn clear_activity() {
    if kawma::token().is_some() {
        let _ = post_activity(&json!({ "game": null })).await;
    }
}

/// A game launched through the launcher. `game` is the Kawma game id
/// ("minecraft" | "skyrim" | "eldenring" | "cyberpunk" | "deadlock"); `pid` may be 0 when
/// Steam started it.
pub fn game_started(app: &AppHandle, game: &'static str, title: &str, detail: Option<String>, pid: u32) {
    if !settings(app).now_playing || !kawma::released(NOW_PLAYING_RELEASE) || kawma::token().is_none() {
        return;
    }
    let generation = PLAYING.fetch_add(1, Ordering::SeqCst) + 1;
    let mut body = json!({ "game": game, "title": clip(title, 60) });
    if let Some(d) = detail.as_deref().map(|d| clip(d, 80)).filter(|d| !d.is_empty()) {
        body["detail"] = json!(d);
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let started = Instant::now();
        let mut next_post = Instant::now();
        let mut sys = sysinfo::System::new();
        loop {
            if PLAYING.load(Ordering::SeqCst) != generation {
                return; // a newer game (or a sign-out) took over
            }
            if !settings(&app).now_playing {
                break;
            }
            if Instant::now() >= next_post {
                match post_activity(&body).await {
                    Ok(true) => next_post = Instant::now() + REPOST_EVERY,
                    Ok(false) => return,
                    Err(e) => {
                        eprintln!("[kawma] now playing: {e}");
                        next_post = Instant::now() + Duration::from_secs(60);
                    }
                }
            }
            tokio::time::sleep(CHECK_EVERY).await;
            let elapsed = started.elapsed();
            if elapsed > Duration::from_secs(12 * 3600) {
                break; // safety cap against a recycled pid
            }
            if elapsed > START_GRACE && !still_running(&mut sys, pid, game) {
                break;
            }
        }
        if PLAYING.load(Ordering::SeqCst) == generation {
            clear_activity().await;
        }
    });
}

// --- Play with your community ------------------------------------------------

struct Session {
    server_id: String,
    generation: u64,
}
static SESSION: Mutex<Option<Session>> = Mutex::new(None);
static SESSIONS: AtomicU64 = AtomicU64::new(0);

fn session_is(generation: u64) -> bool {
    SESSION.lock().ok().is_some_and(|s| s.as_ref().is_some_and(|s| s.generation == generation))
}

/// Whether the server's `server.properties` has `white-list=true`.
fn whitelist_on(server_dir: &std::path::Path) -> bool {
    std::fs::read_to_string(server_dir.join("server.properties"))
        .ok()
        .and_then(|text| {
            text.lines()
                .find_map(|l| l.trim().strip_prefix("white-list=").map(|v| v.trim().eq_ignore_ascii_case("true")))
        })
        .unwrap_or(false)
}

/// A server was shared to the internet at `address`. `post` is the share's
/// "Post to my Kawma community" choice (defaults to the setting).
pub fn share_started(app: &AppHandle, server_id: String, address: String, post: Option<bool>) {
    if !post.unwrap_or_else(|| settings(app).community)
        || !kawma::released(COMMUNITY_RELEASE)
        || kawma::token().is_none()
    {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        let (name, version) = {
            let map = state.servers.lock().await;
            match map.get(&server_id) {
                // Minecraft servers only (they take console commands).
                Some(p) if p.graceful_stop && p.running.load(Ordering::Relaxed) => (p.name.clone(), p.version.clone()),
                _ => return,
            }
        };
        let whitelist = whitelist_on(&launcher_core::server::server_dir(&state.paths, &server_id));
        let body = json!({
            "game": "minecraft",
            "name": clip(&name, 60),
            "address": clip(&address, 120),
            "version": clip(&version, 40),
            "whitelist": whitelist,
        });
        let error = match kawma::request(reqwest::Method::POST, "/auth/desktop/launcher/session", Some(&body)).await {
            Ok(Reply::Ok(v)) => {
                let generation = SESSIONS.fetch_add(1, Ordering::SeqCst) + 1;
                if let Ok(mut s) = SESSION.lock() {
                    *s = Some(Session { server_id: server_id.clone(), generation });
                }
                let _ = app.emit(
                    "kawma-community",
                    json!({
                        "ok": true,
                        "name": name,
                        "discord": v["discord"].as_bool().unwrap_or(false),
                        "twitch": v["twitch"].as_bool().unwrap_or(false),
                        "whitelist": whitelist,
                    }),
                );
                if whitelist {
                    tauri::async_runtime::spawn(poll_requests(app.clone(), server_id, generation));
                }
                return;
            }
            Ok(Reply::Status(404)) => "Play with your community isn't available for your account yet".to_string(),
            Ok(Reply::Status(401)) => "Sign in to Kawma again to post to your community".to_string(),
            Ok(Reply::Status(s)) => format!("Kawma didn't post the server ({s})"),
            Err(e) => e,
        };
        let _ = app.emit("kawma-community", json!({ "ok": false, "name": name, "error": error }));
    });
}

/// Take the open session if it belongs to `server_id` and delete it on Kawma.
fn end_session_for(server_id: &str) {
    let ended = SESSION
        .lock()
        .ok()
        .and_then(|mut s| if s.as_ref().is_some_and(|s| s.server_id == server_id) { s.take() } else { None });
    if ended.is_some() {
        tauri::async_runtime::spawn(async {
            let _ = kawma::request(reqwest::Method::DELETE, "/auth/desktop/launcher/session", None).await;
        });
    }
}

/// Sharing stopped for this server.
pub fn share_stopped(server_id: &str) {
    end_session_for(server_id);
}

/// The hosted server stopped (or crashed).
pub fn server_stopped(server_id: &str) {
    end_session_for(server_id);
}

async fn poll_requests(app: AppHandle, server_id: String, generation: u64) {
    loop {
        tokio::time::sleep(REQUEST_POLL).await;
        if !session_is(generation) {
            return;
        }
        let requests = match kawma::request(reqwest::Method::GET, "/auth/desktop/launcher/session/requests", None).await {
            Ok(Reply::Ok(v)) => v["requests"].as_array().cloned().unwrap_or_default(),
            Ok(Reply::Status(401)) | Ok(Reply::Status(404)) => return,
            _ => continue,
        };
        for r in requests {
            if !session_is(generation) {
                return;
            }
            let (Some(id), Some(name)) = (r["id"].as_str(), r["name"].as_str()) else { continue };
            if id.is_empty() || id.len() > 100 || !id.bytes().all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c)) {
                continue;
            }
            let result = if kawma::valid_name(name) {
                whitelist_add(&app, &server_id, name).await
            } else {
                Err(format!("\"{}\" isn't a valid Minecraft name", clip(name, 32)))
            };
            let body = match &result {
                Ok(()) => json!({ "ok": true }),
                Err(e) => json!({ "ok": false, "error": clip(e, 200) }),
            };
            let _ = kawma::request(
                reqwest::Method::POST,
                &format!("/auth/desktop/launcher/session/requests/{id}"),
                Some(&body),
            )
            .await;
            let _ = app.emit(
                "kawma-community-request",
                json!({
                    "name": name,
                    "user": r["user"].as_str().unwrap_or(""),
                    "from": r["from"].as_str().unwrap_or(""),
                    "ok": result.is_ok(),
                    "error": result.err(),
                }),
            );
        }
    }
}

/// Run `whitelist add <name>` on the running server's console and read its answer.
async fn whitelist_add(app: &AppHandle, server_id: &str, name: &str) -> Result<(), String> {
    let state = app.state::<AppState>();
    let (log, before) = {
        let mut map = state.servers.lock().await;
        let proc = map.get_mut(server_id).ok_or("The server isn't running")?;
        let before = proc.log.lock().map(|v| v.len()).unwrap_or(0);
        proc.stdin
            .write_all(format!("whitelist add {name}\n").as_bytes())
            .await
            .map_err(|_| "Couldn't reach the server console".to_string())?;
        proc.stdin.flush().await.map_err(|_| "Couldn't reach the server console".to_string())?;
        (proc.log.clone(), before)
    };
    // Give the server a moment to answer, then read what it said.
    tokio::time::sleep(Duration::from_secs(2)).await;
    let reply: Vec<String> = log
        .lock()
        .map(|v| {
            // The buffer is capped; if it rolled over, just look at the tail.
            let from = if v.len() >= before { before } else { v.len().saturating_sub(20) };
            v[from..].iter().map(|l| l.line.to_ascii_lowercase()).collect()
        })
        .unwrap_or_default();
    if reply.iter().any(|l| l.contains("that player does not exist")) {
        return Err(format!("No Minecraft player is called {name}"));
    }
    Ok(())
}

// --- Sign-out / exit ------------------------------------------------------------

/// Called before the Kawma token is dropped: stop reporting and close the session.
pub async fn signed_out() {
    PLAYING.fetch_add(1, Ordering::SeqCst);
    let had_session = SESSION.lock().ok().and_then(|mut s| s.take()).is_some();
    if kawma::token().is_none() {
        return;
    }
    clear_activity().await;
    if had_session {
        let _ = kawma::request(reqwest::Method::DELETE, "/auth/desktop/launcher/session", None).await;
    }
}

/// The launcher is quitting (hosted servers are being killed): close any open
/// community session so it doesn't advertise a dead address.
pub fn shutdown() {
    let had_session = SESSION.lock().ok().and_then(|mut s| s.take()).is_some();
    if !had_session || kawma::token().is_none() {
        return;
    }
    tauri::async_runtime::block_on(async {
        let _ = tokio::time::timeout(
            Duration::from_secs(3),
            kawma::request(reqwest::Method::DELETE, "/auth/desktop/launcher/session", None),
        )
        .await;
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_whitelist_setting() {
        let dir = std::env::temp_dir().join(format!("kawma-wl-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("server.properties"), "motd=hi\nwhite-list=true\n").unwrap();
        assert!(whitelist_on(&dir));
        std::fs::write(dir.join("server.properties"), "white-list=false\n").unwrap();
        assert!(!whitelist_on(&dir));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(!whitelist_on(&dir));
    }
}
