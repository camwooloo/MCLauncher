//! The Kawma account: an optional sign-in (Twitch or Discord, through
//! kawma.app) that the launcher's Minecraft accounts are linked to, plus
//! finding Kawma Studio on this PC.
//!
//! Sign-in is the same desktop flow Kawma Studio uses: the launcher makes a
//! PKCE pair, opens the browser at kawma.app, and polls until the person
//! approves. The token (a 30-day bearer) lives in Windows Credential Manager.
//! Everything Minecraft keeps working signed out.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::state::AppState;

pub(crate) const ORIGIN: &str = "https://kawma.app";
const KEYRING_SERVICE: &str = "com.aurora.launcher";
const KEYRING_USER: &str = "kawma-account";
/// Where people get Kawma Studio when it isn't installed.
const STUDIO_PAGE: &str = "https://kawma.app/studio";

/// A Minecraft account linked to the Kawma account (uuid: 32 hex, no dashes).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkedMinecraft {
    pub uuid: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub id: String,
    pub display_name: String,
    #[serde(default)]
    pub avatar: Option<String>,
    #[serde(default)]
    pub twitch: Option<String>,
    #[serde(default)]
    pub discord: Option<String>,
    /// A Kawma tester (admin → People on the website).
    #[serde(default)]
    pub tester: bool,
    /// A tester with test mode on: the launcher takes test builds.
    #[serde(default)]
    pub test_mode: bool,
    /// Which updates this account gets, by id (admin → Updates on the website).
    #[serde(default)]
    pub releases: BTreeMap<String, bool>,
    /// The Minecraft accounts this launcher linked.
    #[serde(default)]
    pub minecraft: Vec<LinkedMinecraft>,
}

/// The last profile read, so background work (Now Playing, community
/// sessions, the update channel) can check releases without a round trip.
static PROFILE: Mutex<Option<Profile>> = Mutex::new(None);

fn remember(profile: Option<&Profile>) {
    if let Ok(mut p) = PROFILE.lock() {
        *p = profile.cloned();
    }
}

/// Whether the signed-in account has the update `id` switched on.
pub(crate) fn released(id: &str) -> bool {
    PROFILE
        .lock()
        .ok()
        .and_then(|p| p.as_ref().and_then(|p| p.releases.get(id).copied()))
        .unwrap_or(false)
}

fn entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER).map_err(|e| e.to_string())
}

/// The stored sign-in token, if signed in.
pub(crate) fn token() -> Option<String> {
    entry().ok()?.get_password().ok()
}

fn forget_token() {
    if let Ok(e) = entry() {
        let _ = e.delete_credential();
    }
    remember(None);
}

pub(crate) fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(concat!("Kawma-Launcher/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| e.to_string())
}

/// What a signed-in API call came back with.
pub(crate) enum Reply {
    Ok(serde_json::Value),
    /// A non-2xx status. 401 has already signed the launcher out.
    Status(u16),
}

/// Call a Bearer endpoint on kawma.app as the signed-in account.
pub(crate) async fn request(
    method: reqwest::Method,
    path: &str,
    body: Option<&serde_json::Value>,
) -> Result<Reply, String> {
    let token = token().ok_or("Sign in to Kawma first")?;
    let mut req = client()?.request(method, format!("{ORIGIN}{path}")).bearer_auth(token);
    if let Some(body) = body {
        req = req.json(body);
    }
    let reply = req.send().await.map_err(|_| "Could not reach Kawma. Check your connection.".to_string())?;
    let status = reply.status().as_u16();
    if status == 401 {
        forget_token();
        return Ok(Reply::Status(401));
    }
    if !reply.status().is_success() {
        return Ok(Reply::Status(status));
    }
    let bytes = reply.bytes().await.map_err(|e| e.to_string())?;
    if bytes.is_empty() {
        return Ok(Reply::Ok(serde_json::Value::Null));
    }
    serde_json::from_slice(&bytes)
        .map(Reply::Ok)
        .map_err(|_| "Kawma returned an unexpected response".to_string())
}

/// The signed-in profile, fresh from kawma.app. `Ok(None)` when signed out
/// (including a token the server no longer accepts).
pub(crate) async fn fetch_profile() -> Result<Option<Profile>, String> {
    if token().is_none() {
        remember(None);
        return Ok(None);
    }
    match request(reqwest::Method::GET, "/auth/desktop/profile", None).await? {
        Reply::Ok(v) => {
            let profile: Profile =
                serde_json::from_value(v).map_err(|_| "Kawma returned an unexpected profile".to_string())?;
            remember(Some(&profile));
            Ok(Some(profile))
        }
        Reply::Status(401) => Ok(None),
        Reply::Status(s) => Err(format!("Kawma answered {s}. Try again shortly.")),
    }
}

// --- Linked Minecraft accounts --------------------------------------------

pub(crate) fn valid_name(name: &str) -> bool {
    (1..=16).contains(&name.len()) && name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
}

/// The Microsoft (not offline) Minecraft accounts on this PC, as Kawma wants them.
async fn local_linked(paths: &launcher_core::paths::Paths) -> Vec<LinkedMinecraft> {
    let store = launcher_core::account::AccountStore::load(&paths.accounts_file())
        .await
        .unwrap_or_default();
    let mut out: Vec<LinkedMinecraft> = Vec::new();
    for stored in &store.accounts {
        let a = &stored.account;
        if a.user_type != "msa" {
            continue;
        }
        let uuid = a.uuid.replace('-', "").to_ascii_lowercase();
        if uuid.len() != 32 || !uuid.bytes().all(|c| c.is_ascii_hexdigit()) || !valid_name(&a.username) {
            continue;
        }
        if out.iter().any(|l| l.uuid == uuid) {
            continue;
        }
        out.push(LinkedMinecraft { uuid, name: a.username.clone() });
    }
    out.truncate(10);
    out
}

/// Replace the Kawma account's linked Minecraft list with this PC's Microsoft
/// accounts. `Ok(None)` when signed out.
pub(crate) async fn sync_minecraft(paths: &launcher_core::paths::Paths) -> Result<Option<Profile>, String> {
    if token().is_none() {
        return Ok(None);
    }
    let accounts = local_linked(paths).await;
    let body = serde_json::json!({ "accounts": accounts });
    match request(reqwest::Method::PUT, "/auth/desktop/launcher/minecraft", Some(&body)).await? {
        Reply::Ok(v) => {
            let profile: Profile =
                serde_json::from_value(v).map_err(|_| "Kawma returned an unexpected profile".to_string())?;
            remember(Some(&profile));
            Ok(Some(profile))
        }
        Reply::Status(401) => Ok(None),
        Reply::Status(s) => Err(format!("Kawma did not save your Minecraft accounts ({s})")),
    }
}

/// Sync the linked list in the background after Minecraft accounts change,
/// and hand the UI the updated profile.
pub(crate) fn spawn_sync(app: &AppHandle) {
    if token().is_none() {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let paths = app.state::<AppState>().paths.clone();
        match sync_minecraft(&paths).await {
            Ok(profile) => {
                let _ = app.emit("kawma-profile", profile);
            }
            Err(e) => eprintln!("[kawma] linking Minecraft accounts failed: {e}"),
        }
    });
}

// --- Sign-in ----------------------------------------------------------------

struct Pending {
    id: String,
    verifier: String,
    expires: Instant,
}
static PENDING: Mutex<Option<Pending>> = Mutex::new(None);

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Login {
    #[serde(skip_serializing)]
    id: String,
    pub code: String,
    pub expires_in: u64,
}

/// Start signing in with `provider` ("twitch" | "discord"): opens the browser.
#[tauri::command]
pub async fn kawma_sign_in_start(provider: String) -> Result<Login, String> {
    if !["twitch", "discord"].contains(&provider.as_str()) {
        return Err("Unknown sign-in provider".into());
    }
    let mut random = [0u8; 32];
    getrandom::getrandom(&mut random).map_err(|e| e.to_string())?;
    let verifier = URL_SAFE_NO_PAD.encode(random);
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let reply = client()?
        .post(format!("{ORIGIN}/auth/desktop/start"))
        .json(&serde_json::json!({ "challenge": challenge, "provider": provider, "app": "launcher" }))
        .send()
        .await
        .map_err(|_| "Could not reach Kawma. Check your connection.".to_string())?;
    if !reply.status().is_success() {
        return Err(format!("Kawma sign-in is unavailable right now ({}). Try again shortly.", reply.status().as_u16()));
    }
    let login: Login = reply.json().await.map_err(|_| "Unexpected sign-in response".to_string())?;
    if login.id.len() != 43 || !login.id.bytes().all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c)) {
        return Err("Unexpected sign-in response".into());
    }
    *PENDING.lock().map_err(|_| "Sign-in unavailable")? = Some(Pending {
        id: login.id.clone(),
        verifier,
        expires: Instant::now() + Duration::from_secs(login.expires_in.min(600)),
    });
    open::that(format!("{ORIGIN}/auth/desktop/login?id={}", login.id)).map_err(|e| e.to_string())?;
    Ok(login)
}

#[derive(Deserialize)]
struct Poll {
    token: Option<String>,
    profile: Option<Profile>,
}

/// Check whether the browser sign-in finished. `None` = still waiting.
#[tauri::command]
pub async fn kawma_sign_in_poll(state: State<'_, AppState>) -> Result<Option<Profile>, String> {
    let (id, verifier) = {
        let mut pending = PENDING.lock().map_err(|_| "Sign-in unavailable")?;
        let p = pending.as_ref().ok_or("No sign-in is in progress")?;
        if Instant::now() >= p.expires {
            *pending = None;
            return Err("Sign-in expired. Please try again.".into());
        }
        (p.id.clone(), p.verifier.clone())
    };
    let reply = client()?
        .post(format!("{ORIGIN}/auth/desktop/poll"))
        .json(&serde_json::json!({ "id": id, "verifier": verifier }))
        .send()
        .await
        .map_err(|_| "Could not reach Kawma".to_string())?;
    match reply.status().as_u16() {
        429 => return Ok(None),
        400 => {
            if let Ok(mut p) = PENDING.lock() {
                *p = None;
            }
            return Err("Sign-in expired. Please try again.".into());
        }
        s if !(200..300).contains(&s) => return Err(format!("Kawma sign-in failed ({s}). Please try again.")),
        _ => {}
    }
    let poll: Poll = reply.json().await.map_err(|_| "Unexpected sign-in response".to_string())?;
    let (Some(token), Some(profile)) = (poll.token, poll.profile) else {
        return Ok(None);
    };
    {
        let mut pending = PENDING.lock().map_err(|_| "Sign-in unavailable")?;
        if pending.as_ref().map(|p| p.id != id).unwrap_or(true) {
            return Ok(None); // cancelled or restarted meanwhile
        }
        *pending = None;
    }
    entry()?
        .set_password(&token)
        .map_err(|e| format!("Could not save your sign-in in Windows Credential Manager: {e}"))?;
    remember(Some(&profile));
    // Link this PC's Minecraft accounts straight away.
    match sync_minecraft(&state.paths).await {
        Ok(Some(updated)) => Ok(Some(updated)),
        Ok(None) => Ok(Some(profile)),
        Err(e) => {
            eprintln!("[kawma] linking Minecraft accounts failed: {e}");
            Ok(Some(profile))
        }
    }
}

#[tauri::command]
pub fn kawma_sign_in_cancel() -> Result<(), String> {
    *PENDING.lock().map_err(|_| "Sign-in unavailable")? = None;
    Ok(())
}

/// The signed-in profile (refreshed at startup). Signs out on 401. Also
/// catches the linked list up if Minecraft accounts changed while signed out.
#[tauri::command]
pub async fn kawma_profile(state: State<'_, AppState>) -> Result<Option<Profile>, String> {
    let Some(profile) = fetch_profile().await? else {
        return Ok(None);
    };
    if local_linked(&state.paths).await != profile.minecraft {
        if let Ok(Some(updated)) = sync_minecraft(&state.paths).await {
            return Ok(Some(updated));
        }
    }
    Ok(Some(profile))
}

/// Sign out: revoke this desktop sign-in on kawma.app (best effort) and forget it here.
#[tauri::command]
pub async fn kawma_sign_out() -> Result<(), String> {
    if let Ok(mut p) = PENDING.lock() {
        *p = None;
    }
    crate::activity::signed_out().await;
    if token().is_some() {
        if let Err(e) = request(reqwest::Method::POST, "/auth/desktop/logout", None).await {
            eprintln!("[kawma] couldn't revoke the sign-in: {e}");
        }
    }
    forget_token();
    Ok(())
}

// --- Star catcher (the rail logo's easter egg) -----------------------------

/// The highest score the server accepts.
const STARS_MAX: u32 = 500;

/// A Star catcher best, as kawma.app keeps it. `new_best`/`just_unlocked`
/// only come back from a submit.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stars {
    #[serde(default)]
    pub best: u32,
    /// Starfield is unlocked in Kawma Studio.
    #[serde(default)]
    pub unlocked: bool,
    #[serde(default)]
    pub new_best: bool,
    #[serde(default)]
    pub just_unlocked: bool,
}

fn stars_reply(reply: Reply) -> Result<Option<Stars>, String> {
    match reply {
        Reply::Ok(v) => serde_json::from_value(v)
            .map(Some)
            .map_err(|_| "Kawma returned an unexpected response".to_string()),
        // Signed out (401) or not released for this account (404): the
        // launcher keeps the best locally instead.
        Reply::Status(401 | 404) => Ok(None),
        Reply::Status(s) => Err(format!("Kawma answered {s}")),
    }
}

/// The saved Star catcher best. `None` when signed out or not released.
#[tauri::command]
pub async fn kawma_stars_best() -> Result<Option<Stars>, String> {
    if token().is_none() {
        return Ok(None);
    }
    stars_reply(request(reqwest::Method::GET, "/auth/desktop/launcher/stars", None).await?)
}

/// Save a finished round. `None` when signed out or not released.
#[tauri::command]
pub async fn kawma_stars_submit(score: u32) -> Result<Option<Stars>, String> {
    if token().is_none() {
        return Ok(None);
    }
    let body = serde_json::json!({ "score": score.min(STARS_MAX) });
    stars_reply(request(reqwest::Method::POST, "/auth/desktop/launcher/stars", Some(&body)).await?)
}

// --- Kawma Studio on this PC ----------------------------------------------

#[derive(Serialize)]
pub struct StudioStatus {
    pub installed: bool,
    pub path: Option<String>,
}

/// `DisplayIcon` is `"<dir>\app.exe"` (quoted), sometimes with an `,0` icon index.
#[cfg_attr(not(windows), allow(dead_code))]
fn exe_from_display_icon(raw: &str) -> PathBuf {
    let raw = raw.trim();
    let path = if let Some(rest) = raw.strip_prefix('"') {
        rest.split('"').next().unwrap_or(rest)
    } else {
        match raw.rsplit_once(',') {
            Some((p, idx)) if idx.trim().parse::<i32>().is_ok() => p,
            _ => raw,
        }
    };
    PathBuf::from(path.trim())
}

/// Find an NSIS-installed app by its uninstall entry (per-user, then per-machine).
#[cfg(windows)]
fn find_installed(product: &str) -> Option<PathBuf> {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    use winreg::RegKey;
    let keys = [
        (HKEY_CURRENT_USER, format!(r"Software\Microsoft\Windows\CurrentVersion\Uninstall\{product}")),
        (HKEY_LOCAL_MACHINE, format!(r"Software\Microsoft\Windows\CurrentVersion\Uninstall\{product}")),
        (HKEY_LOCAL_MACHINE, format!(r"Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\{product}")),
    ];
    for (hive, sub) in keys {
        let Ok(key) = RegKey::predef(hive).open_subkey(&sub) else { continue };
        let Ok(icon) = key.get_value::<String, _>("DisplayIcon") else { continue };
        let exe = exe_from_display_icon(&icon);
        if exe.is_file() {
            return Some(exe);
        }
    }
    None
}
#[cfg(not(windows))]
fn find_installed(_product: &str) -> Option<PathBuf> {
    None
}

#[tauri::command]
pub fn kawma_studio_status() -> StudioStatus {
    let path = find_installed("Kawma Studio");
    StudioStatus { installed: path.is_some(), path: path.map(|p| p.display().to_string()) }
}

/// Open Kawma Studio when it's installed, else its download page. Returns
/// whether Studio itself was launched.
#[tauri::command]
pub fn open_kawma_studio() -> Result<bool, String> {
    if let Some(exe) = find_installed("Kawma Studio") {
        let mut cmd = std::process::Command::new(&exe);
        if let Some(dir) = exe.parent() {
            cmd.current_dir(dir);
        }
        cmd.spawn().map_err(|e| format!("Couldn't open Kawma Studio: {e}"))?;
        return Ok(true);
    }
    open::that(STUDIO_PAGE).map_err(|e| e.to_string())?;
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_icon_paths() {
        assert_eq!(
            exe_from_display_icon(r#""C:\Users\a\AppData\Local\Kawma Studio\kawma-studio.exe""#),
            PathBuf::from(r"C:\Users\a\AppData\Local\Kawma Studio\kawma-studio.exe")
        );
        assert_eq!(exe_from_display_icon(r"C:\x\app.exe,0"), PathBuf::from(r"C:\x\app.exe"));
        assert_eq!(exe_from_display_icon(r"C:\x\app.exe"), PathBuf::from(r"C:\x\app.exe"));
    }

    #[test]
    fn stars_replies() {
        let v = serde_json::json!({ "best": 42, "unlocked": true, "newBest": true, "justUnlocked": false });
        let s = stars_reply(Reply::Ok(v)).unwrap().unwrap();
        assert_eq!(s, Stars { best: 42, unlocked: true, new_best: true, just_unlocked: false });
        let s = stars_reply(Reply::Ok(serde_json::json!({ "best": 7, "unlocked": false }))).unwrap().unwrap();
        assert_eq!(s.best, 7);
        assert!(!s.new_best);
        assert_eq!(stars_reply(Reply::Status(404)).unwrap(), None);
        assert_eq!(stars_reply(Reply::Status(401)).unwrap(), None);
        assert!(stars_reply(Reply::Status(500)).is_err());
    }

    #[test]
    fn minecraft_names() {
        assert!(valid_name("vKarmah"));
        assert!(valid_name("a_b_1"));
        assert!(!valid_name(""));
        assert!(!valid_name("seventeen_chars__"));
        assert!(!valid_name("bad name"));
    }
}
