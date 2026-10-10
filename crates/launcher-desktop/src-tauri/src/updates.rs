//! Self-update from kawma.app, silent like Kawma Studio's.
//!
//! Builds are published on kawma.app (built by `.github/workflows/launcher.yml`
//! and released from kawma.app admin → Updates). Everyone follows the stable
//! channel; Kawma testers with test mode on also get test builds.
//!
//! The flow: `check_app_update` finds a newer build, `download_app_update`
//! downloads it and checks its SHA-256 (it stays in memory until install), and
//! `install_app_update` writes it out and runs the NSIS installer in update
//! mode, with the same flags tauri-plugin-updater passes:
//!
//! - `/UPDATE`: the installer's own update mode (no "already installed" page,
//!   no uninstall, keeps shortcuts), see `installer/template.nsi`.
//! - `/P` (passive: a small progress window) when the person clicked
//!   "Restart to update", or `/S` (silent) for an automatic update.
//! - `/R`: reopen the launcher when done; `/ARGS --hidden` or `--minimized`
//!   reopens it the way it was during an automatic update.
//!
//! Integrity comes from the SHA-256 in the manifest, which is served over
//! https from kawma.app only (we don't use tauri-plugin-updater signatures).

use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::state::AppState;

const STABLE_MANIFEST: &str = "/launcher/releases/latest.json";
const TEST_MANIFEST: &str = "/launcher/test/latest.json";
const UPDATE_PLATFORM: &str = "windows-x86_64";
/// Started from the main window when the launcher reopens after an automatic
/// update, so it comes back the way it was.
pub const RELAUNCH_HIDDEN: &str = "--hidden";
pub const RELAUNCH_MINIMIZED: &str = "--minimized";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    /// Latest version (without the leading `v`).
    pub version: String,
    /// Currently-running version.
    pub current: String,
    /// Release notes (markdown).
    pub notes: String,
    /// Where the installer comes from (always https://kawma.app/…).
    pub download_url: String,
    /// "stable" | "test".
    pub channel: String,
    /// Already downloaded and verified, waiting to install.
    pub ready: bool,
}

#[derive(Deserialize)]
struct UpdateManifest {
    version: String,
    #[serde(default)]
    notes: String,
    #[serde(default)]
    platforms: std::collections::HashMap<String, UpdateBuild>,
}

#[derive(Deserialize, Clone, PartialEq)]
struct UpdateBuild {
    url: String,
    sha256: String,
    #[serde(default)]
    size: Option<u64>,
}

#[derive(Clone)]
struct Offered {
    version: String,
    build: UpdateBuild,
}

struct Ready {
    version: String,
    build: UpdateBuild,
    bytes: Vec<u8>,
}

/// The build `check_app_update` offered; the download and install use exactly this.
static OFFERED: Mutex<Option<Offered>> = Mutex::new(None);
/// The offered build, downloaded and verified.
static READY: Mutex<Option<Ready>> = Mutex::new(None);
/// One download at a time.
static DOWNLOADING: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Compare dotted versions numerically: is `latest` newer than `current`?
fn version_is_newer(latest: &str, current: &str) -> bool {
    fn parts(s: &str) -> [u32; 3] {
        let mut out = [0u32; 3];
        for (i, seg) in s.trim().trim_start_matches('v').split(['.', '-', '+']).take(3).enumerate() {
            let digits: String = seg.chars().take_while(|c| c.is_ascii_digit()).collect();
            out[i] = digits.parse().unwrap_or(0);
        }
        out
    }
    let (a, b) = (parts(latest), parts(current));
    a > b
}

/// Installers are only ever downloaded over https from kawma.app.
fn is_kawma_download(url: &str) -> bool {
    reqwest::Url::parse(url).is_ok_and(|u| {
        u.scheme() == "https"
            && u.host_str() == Some("kawma.app")
            && u.port().is_none()
            && u.username().is_empty()
            && u.password().is_none()
    })
}

fn is_sha256(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|c| c.is_ascii_hexdigit())
}

/// Release notes as a markdown list: plain lines become bullets.
pub(crate) fn notes_markdown<'a>(lines: impl Iterator<Item = &'a str>) -> String {
    lines
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(|l| if l.starts_with(['-', '*', '#']) { l.to_string() } else { format!("- {l}") })
        .collect::<Vec<_>>()
        .join("\n")
}

/// One channel's manifest; `None` when nothing is published there yet.
async fn fetch_update_manifest(path: &str) -> Result<Option<UpdateManifest>, String> {
    let resp = crate::kawma::client()?
        .get(format!("{}{path}", crate::kawma::ORIGIN))
        .send()
        .await
        .map_err(|_| "Couldn't reach kawma.app to check for updates".to_string())?;
    if resp.status().as_u16() == 404 {
        return Ok(None);
    }
    if !resp.status().is_success() {
        return Err(format!("kawma.app answered {} for updates", resp.status().as_u16()));
    }
    resp.json().await.map(Some).map_err(|_| "kawma.app returned an invalid update manifest".to_string())
}

fn is_ready(version: &str, build: &UpdateBuild) -> bool {
    READY.lock().ok().is_some_and(|r| r.as_ref().is_some_and(|r| r.version == version && &r.build == build))
}

/// Check kawma.app for a newer build. Returns `None` if up to date.
#[tauri::command]
pub async fn check_app_update(app: AppHandle) -> Result<Option<UpdateInfo>, String> {
    let current = app.package_info().version.to_string();
    // Testers with test mode on look at the test channel first.
    let tester = matches!(crate::kawma::fetch_profile().await, Ok(Some(p)) if p.tester && p.test_mode);
    let mut channels = vec![("stable", STABLE_MANIFEST)];
    if tester {
        channels.insert(0, ("test", TEST_MANIFEST));
    }
    for (channel, path) in channels {
        let manifest = match fetch_update_manifest(path).await {
            Ok(Some(m)) => m,
            Ok(None) => continue,
            Err(e) if channel == "test" => {
                eprintln!("[update] test channel: {e}");
                continue;
            }
            Err(e) => return Err(e),
        };
        let Some(build) = manifest.platforms.get(UPDATE_PLATFORM).cloned() else { continue };
        if !version_is_newer(&manifest.version, &current) || !is_kawma_download(&build.url) || !is_sha256(build.sha256.trim()) {
            continue;
        }
        let version = manifest.version.trim_start_matches('v').to_string();
        let download_url = build.url.clone();
        let ready = is_ready(&version, &build);
        if let Ok(mut offered) = OFFERED.lock() {
            *offered = Some(Offered { version: version.clone(), build });
        }
        return Ok(Some(UpdateInfo {
            version,
            current,
            notes: notes_markdown(manifest.notes.lines()),
            download_url,
            channel: channel.to_string(),
            ready,
        }));
    }
    // Nothing newer any more (e.g. a test build was pulled): drop what we had.
    if let Ok(mut offered) = OFFERED.lock() {
        *offered = None;
    }
    if let Ok(mut ready) = READY.lock() {
        *ready = None;
    }
    Ok(None)
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DownloadProgress {
    version: String,
    received: u64,
    total: Option<u64>,
}

/// Download the offered installer and verify its SHA-256. It is kept in
/// memory (not on disk) until `install_app_update`, so nothing can swap it.
/// Emits `app-update-progress` while downloading.
#[tauri::command]
pub async fn download_app_update(app: AppHandle) -> Result<(), String> {
    use sha2::{Digest, Sha256};

    let _one = DOWNLOADING.lock().await;
    let offered = OFFERED.lock().ok().and_then(|o| o.clone()).ok_or("Check for updates first")?;
    if is_ready(&offered.version, &offered.build) {
        return Ok(());
    }
    let build = offered.build.clone();
    if !is_kawma_download(&build.url) {
        return Err("Updates only install from kawma.app".into());
    }
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(20))
        .timeout(Duration::from_secs(30 * 60))
        .user_agent(concat!("Kawma-Launcher/", env!("CARGO_PKG_VERSION")))
        // Follow redirects only while they stay on kawma.app.
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() < 5 && is_kawma_download(attempt.url().as_str()) {
                attempt.follow()
            } else {
                attempt.stop()
            }
        }))
        .build()
        .map_err(|e| e.to_string())?;
    let mut resp = client
        .get(&build.url)
        .send()
        .await
        .map_err(|_| "Couldn't download the update".to_string())?;
    if !resp.status().is_success() {
        return Err(format!("Couldn't download the update ({})", resp.status().as_u16()));
    }
    let total = build.size.or(resp.content_length());
    let mut bytes: Vec<u8> = Vec::with_capacity(total.unwrap_or(0).min(512 << 20) as usize);
    let mut hasher = Sha256::new();
    let mut last_emit = Instant::now();
    let emit = |received: u64| {
        let _ = app.emit(
            "app-update-progress",
            DownloadProgress { version: offered.version.clone(), received, total },
        );
    };
    emit(0);
    while let Some(chunk) = resp.chunk().await.map_err(|_| "The update download was interrupted".to_string())? {
        hasher.update(&chunk);
        bytes.extend_from_slice(&chunk);
        if build.size.is_some_and(|size| bytes.len() as u64 > size) {
            return Err("The update download was larger than expected, so it wasn't installed.".into());
        }
        if last_emit.elapsed() > Duration::from_millis(150) {
            last_emit = Instant::now();
            emit(bytes.len() as u64);
        }
    }
    emit(bytes.len() as u64);
    if build.size.is_some_and(|size| size != bytes.len() as u64) {
        return Err("The update download was incomplete. Try again.".into());
    }
    let digest = hex::encode(hasher.finalize());
    if !digest.eq_ignore_ascii_case(build.sha256.trim()) {
        return Err("The update failed its integrity check, so it wasn't installed. Try again later.".into());
    }
    // The offer may have moved on while we downloaded.
    let still_offered = OFFERED
        .lock()
        .ok()
        .is_some_and(|o| o.as_ref().is_some_and(|o| o.version == offered.version && o.build == build));
    if !still_offered {
        return Err("A newer update came out while downloading. Check again.".into());
    }
    if let Ok(mut ready) = READY.lock() {
        *ready = Some(Ready { version: offered.version, build, bytes });
    }
    Ok(())
}

// --- What an automatic update must wait for ----------------------------------

struct Launched {
    game: &'static str,
    pid: u32,
    at: Instant,
}

/// Games launched from here this session (pruned once they have exited).
static GAMES: Mutex<Vec<Launched>> = Mutex::new(Vec::new());
/// Launchers (Steam, SKSE, the Seamless launcher) hand over to the real game
/// process; count a game as running for its first minute and a half.
const START_GRACE: Duration = Duration::from_secs(90);

/// Remember a game launched through the launcher (`pid` may be 0 when Steam
/// started it), so an automatic update waits until it closes.
pub fn game_launched(game: &'static str, pid: u32) {
    if let Ok(mut games) = GAMES.lock() {
        games.push(Launched { game, pid, at: Instant::now() });
    }
}

fn game_name(game: &str) -> &'static str {
    match game {
        "minecraft" | "instance" => "Minecraft",
        "skyrim" => "Skyrim",
        "eldenring" => "Elden Ring",
        "cyberpunk" => "Cyberpunk 2077",
        _ => "A game",
    }
}

/// The first launched game that is still running, if any.
fn running_game() -> Option<&'static str> {
    use sysinfo::{Pid, ProcessesToUpdate, System};
    let mut games = GAMES.lock().ok()?;
    if games.is_empty() {
        return None;
    }
    let mut sys = System::new();
    sys.refresh_processes(ProcessesToUpdate::All, true);
    games.retain(|g| {
        g.at.elapsed() < START_GRACE
            || (g.pid != 0 && sys.process(Pid::from_u32(g.pid)).is_some())
            || crate::activity::game_processes(g.game)
                .iter()
                .any(|n| sys.processes().values().any(|p| p.name().eq_ignore_ascii_case(n)))
    });
    games.first().map(|g| game_name(g.game))
}

/// Why the launcher can't restart right now, if anything: a hosted server
/// (it would stop) or a game launched from here.
pub async fn blocker(state: &AppState, include_games: bool) -> Option<String> {
    let servers = state
        .servers
        .lock()
        .await
        .values()
        .filter(|p| p.running.load(std::sync::atomic::Ordering::Relaxed))
        .count();
    if servers > 0 {
        return Some(if servers == 1 {
            "A hosted server is running".into()
        } else {
            format!("{servers} hosted servers are running")
        });
    }
    if include_games {
        if let Some(name) = tokio::task::spawn_blocking(running_game).await.ok().flatten() {
            return Some(format!("{name} is running"));
        }
    }
    None
}

/// What an automatic update is waiting for right now (`None` = clear to install).
#[tauri::command]
pub async fn update_blocker(state: State<'_, AppState>) -> Result<Option<String>, String> {
    Ok(blocker(&state, true).await)
}

/// Install the downloaded update: write the verified installer, run it in
/// update mode and quit so it can replace the app. `auto` is an automatic
/// (background) update: it runs silently and also waits for games.
#[tauri::command]
pub async fn install_app_update(app: AppHandle, state: State<'_, AppState>, auto: bool) -> Result<(), String> {
    use sha2::{Digest, Sha256};

    if let Some(reason) = blocker(&state, auto).await {
        return Err(if auto {
            format!("{reason}, so the update will wait.")
        } else {
            format!("{reason}. Stop it first: updating restarts Kawma Launcher.")
        });
    }
    let (version, bytes, sha256) = {
        let ready = READY.lock().map_err(|_| "Update state unavailable")?;
        let ready = ready.as_ref().ok_or("Download the update first")?;
        (ready.version.clone(), ready.bytes.clone(), ready.build.sha256.clone())
    };
    // Belt and braces: the bytes we run are the bytes we verified.
    if !hex::encode(Sha256::digest(&bytes)).eq_ignore_ascii_case(sha256.trim()) {
        if let Ok(mut ready) = READY.lock() {
            *ready = None;
        }
        return Err("The update failed its integrity check. Check for updates again.".into());
    }

    let dir = std::env::temp_dir().join("kawma-launcher-update");
    let _ = tokio::fs::remove_dir_all(&dir).await;
    tokio::fs::create_dir_all(&dir).await.map_err(|e| e.to_string())?;
    let path = dir.join(format!("Kawma-Launcher-{version}-setup.exe"));
    tokio::fs::write(&path, &bytes).await.map_err(|e| e.to_string())?;

    let relaunch = auto.then(|| {
        let window = app.get_webview_window("main");
        let visible = window.as_ref().and_then(|w| w.is_visible().ok()).unwrap_or(true);
        if visible { RELAUNCH_MINIMIZED } else { RELAUNCH_HIDDEN }
    });
    let args = installer_args(auto, relaunch);
    eprintln!("[update] installing {version}: {} {}", path.display(), args.join(" "));
    std::process::Command::new(&path)
        .args(&args)
        .spawn()
        .map_err(|e| format!("Couldn't start the installer: {e}"))?;

    // Quit straight away (after this reply reaches the window) so the
    // installer finds the app closed; it reopens it when done.
    let app2 = app.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(400)).await;
        app2.exit(0);
    });
    Ok(())
}

/// The NSIS flags, as tauri-plugin-updater passes them (passive or quiet
/// install mode, then `/UPDATE`, `/R` and `/ARGS`).
fn installer_args(auto: bool, relaunch: Option<&str>) -> Vec<String> {
    let mut args = vec![if auto { "/S" } else { "/P" }.to_string(), "/UPDATE".into(), "/R".into()];
    if let Some(arg) = relaunch {
        args.push("/ARGS".into());
        args.push(arg.into());
    }
    args
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_numerically() {
        assert!(version_is_newer("1.10.0", "1.9.3"));
        assert!(version_is_newer("v2.0.0", "1.99.99"));
        assert!(!version_is_newer("1.8.0", "1.8.0"));
        assert!(!version_is_newer("1.7.9", "1.8.0"));
    }

    #[test]
    fn downloads_only_from_kawma_over_https() {
        assert!(is_kawma_download("https://kawma.app/launcher/releases/1.9.0/setup.exe"));
        assert!(!is_kawma_download("http://kawma.app/x.exe"));
        assert!(!is_kawma_download("https://kawma.app.evil.com/x.exe"));
        assert!(!is_kawma_download("https://user@kawma.app/x.exe"));
        assert!(!is_kawma_download("https://kawma.app:8443/x.exe"));
    }

    #[test]
    fn sha256_must_be_64_hex() {
        assert!(is_sha256(&"a".repeat(64)));
        assert!(!is_sha256(&"a".repeat(63)));
        assert!(!is_sha256(&"g".repeat(64)));
    }

    #[test]
    fn installer_flags_match_the_tauri_updater() {
        assert_eq!(installer_args(false, None), ["/P", "/UPDATE", "/R"]);
        assert_eq!(installer_args(true, Some(RELAUNCH_HIDDEN)), ["/S", "/UPDATE", "/R", "/ARGS", "--hidden"]);
    }

    #[test]
    fn an_unknown_game_is_not_running() {
        assert_eq!(running_game(), None);
        game_launched("cyberpunk", 0);
        // Inside the start grace it counts as running even without a process.
        assert_eq!(running_game(), Some("Cyberpunk 2077"));
    }
}
