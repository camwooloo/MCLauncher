//! Tauri commands callable from the web UI via `invoke()`.
//!
//! These are the bridge between the React frontend and `launcher-core`:
//! Minecraft (versions, accounts, Microsoft login, install+launch with live
//! progress events) and the native games (detect, launch, co-op config).
//!
//! Long-running install progress is streamed as `"mc-progress"` events; the
//! Microsoft device-code prompt is streamed as a `"login-prompt"` event.

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::Mutex;

use launcher_core::account::{Account, AccountStore};
use launcher_core::auth::Auth;
use launcher_core::games::{cyberpunk, eldenring, skyrim, steam_run_url};
use launcher_core::launch::{self, LaunchOptions};
use launcher_core::manifest::VersionManifest;
use launcher_core::modloader::{fabric, quilt};
use launcher_core::platform::Environment;
use launcher_core::progress::{Reporter, SharedReporter};
use launcher_core::{java, Installer};

use crate::progress::EventReporter;
use crate::settings::Settings;
use crate::state::AppState;

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

// --- App / misc ----------------------------------------------------------

#[tauri::command]
pub fn app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[tauri::command]
pub fn open_url(url: String) -> Result<(), String> {
    open::that(url).map_err(err)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PathsInfo {
    pub game_dir: String,
    pub data_dir: String,
}

/// Total physical system memory in MiB (for sizing the RAM sliders).
#[tauri::command]
pub fn system_memory_mb() -> u64 {
    let mut sys = sysinfo::System::new();
    sys.refresh_memory();
    (sys.total_memory() / 1_048_576).max(2048)
}

#[tauri::command]
pub fn paths_info(state: State<'_, AppState>) -> PathsInfo {
    PathsInfo {
        game_dir: state.paths.game_dir.to_string_lossy().into_owned(),
        data_dir: state.paths.data_dir.to_string_lossy().into_owned(),
    }
}

// --- Settings ------------------------------------------------------------

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<Settings, String> {
    Ok(Settings::load(&state.paths.settings_file()).await)
}

#[tauri::command]
pub async fn save_settings(state: State<'_, AppState>, settings: Settings) -> Result<(), String> {
    settings.save(&state.paths.settings_file()).await
}

// --- Self-update ---------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    /// Latest version (without the leading `v`).
    pub version: String,
    /// Currently-running version.
    pub current: String,
    /// Release notes (markdown).
    pub notes: String,
    /// Direct download URL of the installer asset.
    pub download_url: String,
}

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

/// Check GitHub for a newer release. Returns `None` if up to date.
#[tauri::command]
pub async fn check_app_update(app: AppHandle) -> Result<Option<UpdateInfo>, String> {
    let current = app.package_info().version.to_string();
    let resp = launcher_core::http::client()
        .get("https://api.github.com/repos/camwooloo/MCLauncher/releases/latest")
        .header("User-Agent", "Aurora-Launcher")
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(err)?;
    if !resp.status().is_success() {
        return Ok(None);
    }
    let v: serde_json::Value = resp.json().await.map_err(err)?;
    let tag = v.get("tag_name").and_then(|s| s.as_str()).unwrap_or("");
    let notes = v.get("body").and_then(|s| s.as_str()).unwrap_or("").to_string();
    let download_url = v
        .get("assets")
        .and_then(|a| a.as_array())
        .and_then(|a| {
            a.iter().find(|x| {
                x.get("name")
                    .and_then(|n| n.as_str())
                    .map(|n| n.ends_with(".exe"))
                    .unwrap_or(false)
            })
        })
        .and_then(|x| x.get("browser_download_url"))
        .and_then(|u| u.as_str())
        .unwrap_or("")
        .to_string();

    if version_is_newer(tag, &current) && !download_url.is_empty() {
        Ok(Some(UpdateInfo {
            version: tag.trim_start_matches('v').to_string(),
            current,
            notes,
            download_url,
        }))
    } else {
        Ok(None)
    }
}

/// Download the installer and run it, then quit so it can replace our files.
#[tauri::command]
pub async fn apply_app_update(app: AppHandle, download_url: String) -> Result<(), String> {
    let bytes = launcher_core::http::client()
        .get(&download_url)
        .header("User-Agent", "Aurora-Launcher")
        .send()
        .await
        .map_err(err)?
        .error_for_status()
        .map_err(|_| "Couldn't download the update".to_string())?
        .bytes()
        .await
        .map_err(err)?;
    let path = std::env::temp_dir().join("Aurora-Launcher-Update.exe");
    tokio::fs::write(&path, &bytes).await.map_err(err)?;

    // Launch the installer, then exit shortly after so it isn't blocked by our
    // running exe (the installer relaunches the app when it finishes).
    std::process::Command::new(&path)
        .spawn()
        .map_err(|e| format!("Couldn't start the installer: {e}"))?;
    let app2 = app.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(1200)).await;
        app2.exit(0);
    });
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseInfo {
    pub version: String,
    pub name: String,
    pub notes: String,
    /// Publish date (YYYY-MM-DD).
    pub date: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleasesResult {
    pub current: String,
    pub releases: Vec<ReleaseInfo>,
}

/// Recent GitHub releases (newest first) for the in-app patch-notes view.
#[tauri::command]
pub async fn list_releases(app: AppHandle) -> Result<ReleasesResult, String> {
    let current = app.package_info().version.to_string();
    let resp = launcher_core::http::client()
        .get("https://api.github.com/repos/camwooloo/MCLauncher/releases?per_page=20")
        .header("User-Agent", "Aurora-Launcher")
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(err)?;
    if !resp.status().is_success() {
        return Ok(ReleasesResult { current, releases: vec![] });
    }
    let arr: serde_json::Value = resp.json().await.map_err(err)?;
    let releases = arr
        .as_array()
        .map(|a| {
            a.iter()
                .filter(|r| !r.get("draft").and_then(|d| d.as_bool()).unwrap_or(false))
                .filter_map(|r| {
                    let tag = r.get("tag_name").and_then(|s| s.as_str())?;
                    Some(ReleaseInfo {
                        version: tag.trim_start_matches('v').to_string(),
                        name: r.get("name").and_then(|s| s.as_str()).filter(|s| !s.is_empty()).unwrap_or(tag).to_string(),
                        notes: r.get("body").and_then(|s| s.as_str()).unwrap_or("").to_string(),
                        date: r
                            .get("published_at")
                            .and_then(|s| s.as_str())
                            .unwrap_or("")
                            .chars()
                            .take(10)
                            .collect(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(ReleasesResult { current, releases })
}

// --- Aurora Net (built-in Tailscale VPN) ---------------------------------

#[tauri::command]
pub async fn vpn_status() -> Result<crate::vpn::VpnStatus, String> {
    Ok(crate::vpn::status().await)
}

/// Download + run the Tailscale installer (UAC prompt). Phase 1.
#[tauri::command]
pub async fn vpn_install() -> Result<(), String> {
    crate::vpn::install().await
}

/// Begin interactive sign-in; returns a URL to open in the browser (or `None`
/// if already signed in). Phase 1.
#[tauri::command]
pub async fn vpn_login() -> Result<Option<String>, String> {
    crate::vpn::login().await
}

#[tauri::command]
pub async fn vpn_disconnect() -> Result<(), String> {
    crate::vpn::down().await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VpnConfig {
    /// Whether a Tailscale API token is stored (enables hosting). We never
    /// return the token itself.
    pub has_token: bool,
}

#[tauri::command]
pub async fn vpn_config(state: State<'_, AppState>) -> Result<VpnConfig, String> {
    let s = Settings::load(&state.paths.settings_file()).await;
    Ok(VpnConfig {
        has_token: !s.tailscale_api_token.trim().is_empty(),
    })
}

#[tauri::command]
pub async fn vpn_set_token(state: State<'_, AppState>, token: String) -> Result<(), String> {
    let mut s = Settings::load(&state.paths.settings_file()).await;
    s.tailscale_api_token = token.trim().to_string();
    s.save(&state.paths.settings_file()).await
}

/// Player side: decode a join code and join the host's network. Phase 2.
#[tauri::command]
pub async fn vpn_join(code: String) -> Result<crate::vpn::JoinPayload, String> {
    let payload = crate::vpn::decode_code(&code)?;
    if crate::vpn::tailscale_exe().is_none() {
        return Err("Install Aurora Net first — it sets up the secure connection.".into());
    }
    crate::vpn::up_with_authkey(&payload.key).await?;
    Ok(payload)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareArgs {
    pub name: String,
    pub port: u16,
    /// "minecraft" | "skyrim" | "eldenring" | "cyberpunk".
    pub game: String,
    /// Also set Tailscale access rules so guests reach only this server.
    pub configure_access: bool,
}

/// Host side: mint a guest key (+ optionally lock access to this server) and
/// return a shareable join code. Phase 3.
#[tauri::command]
pub async fn vpn_share(state: State<'_, AppState>, args: ShareArgs) -> Result<String, String> {
    let s = Settings::load(&state.paths.settings_file()).await;
    let token = s.tailscale_api_token.trim().to_string();
    if token.is_empty() {
        return Err("Add your Tailscale access token first (Aurora Net → Hosting).".into());
    }
    let st = crate::vpn::status().await;
    let ip = st
        .ip
        .ok_or("Connect to Aurora Net first — you need to be online to share.")?;
    if args.configure_access {
        crate::vpn::ensure_access_rules(&token, &ip, &[args.port]).await?;
    }
    let key = crate::vpn::mint_join_key(&token).await?;
    let payload = crate::vpn::JoinPayload {
        v: 1,
        key,
        ip,
        port: args.port,
        name: args.name,
        game: args.game,
    };
    crate::vpn::encode_code(&payload)
}

// --- Accounts ------------------------------------------------------------

#[tauri::command]
pub async fn list_accounts(state: State<'_, AppState>) -> Result<AccountStore, String> {
    AccountStore::load(&state.paths.accounts_file())
        .await
        .map_err(err)
}

#[tauri::command]
pub async fn add_offline_account(
    state: State<'_, AppState>,
    username: String,
) -> Result<Account, String> {
    let account = Account::offline(username.trim());
    let path = state.paths.accounts_file();
    let mut store = AccountStore::load(&path).await.unwrap_or_default();
    store.upsert(account.clone(), String::new());
    store.save(&path).await.map_err(err)?;
    Ok(account)
}

#[tauri::command]
pub async fn set_active_account(state: State<'_, AppState>, uuid: String) -> Result<(), String> {
    let path = state.paths.accounts_file();
    let mut store = AccountStore::load(&path).await.unwrap_or_default();
    if store.accounts.iter().any(|a| a.account.uuid == uuid) {
        store.active_uuid = Some(uuid);
        store.save(&path).await.map_err(err)?;
    }
    Ok(())
}

#[tauri::command]
pub async fn remove_account(state: State<'_, AppState>, uuid: String) -> Result<(), String> {
    let path = state.paths.accounts_file();
    let mut store = AccountStore::load(&path).await.unwrap_or_default();
    store.remove(&uuid);
    store.save(&path).await.map_err(err)?;
    Ok(())
}

/// The default, "no-code" sign-in: open the browser, the user signs in and
/// approves, we capture the redirect. No code to copy.
#[tauri::command]
pub async fn microsoft_login(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Account, String> {
    let auth = Auth::new(crate::secrets::AZURE_CLIENT_ID.to_string());
    let browser_app = app.clone();
    let result = auth
        .login_auth_code(|url| {
            // Let the UI show "check your browser…" and open the sign-in page.
            let _ = browser_app.emit("login-opened", serde_json::json!({}));
            if let Err(e) = open::that(url) {
                eprintln!("[microsoft_login] couldn't open browser: {e}");
            }
        })
        .await;
    finish_login(&app, &state, result, "microsoft_login").await
}

/// Fallback sign-in using the device-code flow (visit a URL, type a short
/// code). Works even if the Azure app has no redirect URI registered.
#[tauri::command]
pub async fn microsoft_login_code(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Account, String> {
    let auth = Auth::new(crate::secrets::AZURE_CLIENT_ID.to_string());
    let prompt_app = app.clone();
    let result = auth
        .login_device_code(|dc| {
            let _ = prompt_app.emit(
                "login-prompt",
                serde_json::json!({
                    "userCode": dc.user_code,
                    "verificationUri": dc.verification_uri,
                    "message": dc.message,
                }),
            );
        })
        .await;
    finish_login(&app, &state, result, "microsoft_login_code").await
}

/// Load the active account, silently refreshing a Microsoft token first so
/// online actions (launching, skins) use a valid token. The refreshed token is
/// persisted. Falls back to the stored account if the refresh fails (offline,
/// transient error) so the caller still gets *something* usable.
pub(crate) async fn active_account_refreshed(
    paths: &launcher_core::paths::Paths,
) -> Result<Account, String> {
    let path = paths.accounts_file();
    let mut store = AccountStore::load(&path).await.map_err(err)?;
    let active = store
        .active()
        .cloned()
        .ok_or_else(|| "No active account".to_string())?;
    if active.account.is_online() && !active.refresh_token.is_empty() {
        if let Ok(res) = Auth::new(crate::secrets::AZURE_CLIENT_ID.to_string())
            .login_with_refresh(&active.refresh_token)
            .await
        {
            store.upsert(res.account.clone(), res.refresh_token);
            let _ = store.save(&path).await;
            return Ok(res.account);
        }
    }
    Ok(active.account)
}

/// Shared tail of both sign-in flows: persist the account or surface the error.
async fn finish_login(
    app: &AppHandle,
    state: &State<'_, AppState>,
    result: Result<launcher_core::auth::AuthResult, launcher_core::Error>,
    tag: &str,
) -> Result<Account, String> {
    let result = match result {
        Ok(r) => r,
        Err(e) => {
            let msg = e.to_string();
            eprintln!("[{tag}] failed: {msg}");
            let _ = app.emit("login-error", serde_json::json!({ "message": msg.clone() }));
            return Err(msg);
        }
    };

    let path = state.paths.accounts_file();
    let mut store = AccountStore::load(&path).await.unwrap_or_default();
    store.upsert(result.account.clone(), result.refresh_token);
    if let Err(e) = store.save(&path).await {
        let msg = e.to_string();
        let _ = app.emit("login-error", serde_json::json!({ "message": msg.clone() }));
        return Err(msg);
    }
    let _ = app.emit("login-ok", serde_json::json!({ "username": result.account.username }));
    eprintln!("[{tag}] signed in as {}", result.account.username);
    Ok(result.account)
}

// --- Minecraft -----------------------------------------------------------

#[tauri::command]
pub async fn minecraft_versions() -> Result<Vec<String>, String> {
    let manifest = VersionManifest::fetch().await.map_err(err)?;
    Ok(manifest.releases().map(|v| v.id.clone()).collect())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayArgs {
    pub version: String,
    /// "vanilla" | "fabric" | "quilt"
    pub loader: String,
    pub account: Account,
    pub memory_mb: u32,
    /// Optional server address to auto-join via Quick Play (1.20+).
    #[serde(default)]
    pub server: Option<String>,
}

#[tauri::command]
pub async fn play_minecraft(
    app: AppHandle,
    state: State<'_, AppState>,
    args: PlayArgs,
) -> Result<String, String> {
    let paths = state.paths.clone();
    let installer = Installer::new(paths.clone());

    // Resolve loader → launchable version id.
    let version_id = match args.loader.as_str() {
        "fabric" => {
            let lv = fabric::latest_stable(&args.version).await.map_err(err)?;
            fabric::install(&paths, &args.version, &lv).await.map_err(err)?
        }
        "quilt" => {
            let lv = quilt::latest_stable(&args.version).await.map_err(err)?;
            quilt::install(&paths, &args.version, &lv).await.map_err(err)?
        }
        _ => args.version.clone(),
    };

    let manifest = VersionManifest::fetch().await.map_err(err)?;
    let version = installer.resolve_version(&manifest, &version_id).await.map_err(err)?;

    // Progress pump: emit ~8 Hz until the work signals completion.
    let reporter = Arc::new(EventReporter::default());
    let pump_reporter = reporter.clone();
    let pump_app = app.clone();
    let done = Arc::new(AtomicBool::new(false));
    let pump_done = done.clone();
    tokio::spawn(async move {
        loop {
            let _ = pump_app.emit("mc-progress", pump_reporter.snapshot());
            if pump_done.load(Ordering::Relaxed) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(120)).await;
        }
    });

    let rep: SharedReporter = reporter.clone();
    let run = async {
        let installed = installer.install(&version, rep.clone()).await?;
        let major = version
            .java_version
            .as_ref()
            .map(|j| j.major_version)
            .unwrap_or(21);
        let java = java::ensure_java(&paths, major, &rep).await?;
        reporter.stage("Launching");
        let mut options = LaunchOptions {
            max_memory_mb: args.memory_mb,
            ..Default::default()
        };
        if let Some(addr) = args.server.as_deref().filter(|a| !a.is_empty()) {
            options.extra_game_args = vec!["--quickPlayMultiplayer".into(), addr.to_string()];
        }
        let env = Environment::detect();
        let child = launch::launch(&installed, &paths, &java, &args.account, &options, &env).await?;
        let pid = child.id();
        drop(child); // detach: keep running after we return
        Ok::<_, launcher_core::Error>(pid)
    };

    let outcome = run.await;
    done.store(true, Ordering::Relaxed);
    let _ = app.emit("mc-progress", reporter.snapshot());

    match outcome {
        Ok(pid) => {
            let msg = format!("Launched {version_id}");
            let _ = app.emit("mc-done", serde_json::json!({ "message": msg, "pid": pid }));
            Ok(msg)
        }
        Err(e) => {
            let msg = e.to_string();
            let _ = app.emit("mc-error", serde_json::json!({ "message": msg }));
            Err(msg)
        }
    }
}

// --- Native games --------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GamesStatus {
    pub skyrim: skyrim::SkyrimInfo,
    pub elden_ring: eldenring::EldenRingInfo,
    pub cyberpunk: cyberpunk::CyberpunkInfo,
}

#[tauri::command]
pub fn detect_games() -> GamesStatus {
    GamesStatus {
        skyrim: skyrim::detect(),
        elden_ring: eldenring::detect(),
        cyberpunk: cyberpunk::detect(),
    }
}

#[tauri::command]
pub fn launch_skyrim(mode: String) -> Result<u32, String> {
    use skyrim::SkyrimLaunch::*;
    let info = skyrim::detect();
    let m = match mode.as_str() {
        "skse" => Skse,
        "together" => SkyrimTogether,
        _ => Vanilla,
    };
    skyrim::launch(&info, m).map_err(err)
}

#[tauri::command]
pub fn launch_elden_ring(mode: String) -> Result<u32, String> {
    let info = eldenring::detect();
    match mode.as_str() {
        "seamless" => eldenring::launch(&info, eldenring::EldenRingLaunch::SeamlessCoop).map_err(err),
        "modded" => eldenring::launch(&info, eldenring::EldenRingLaunch::Modded).map_err(err),
        _ => {
            // Vanilla: go through Steam so EAC and online services start.
            open::that(steam_run_url(eldenring::APP_ID)).map_err(err)?;
            Ok(0)
        }
    }
}

#[tauri::command]
pub fn launch_cyberpunk(mode: String) -> Result<u32, String> {
    let info = cyberpunk::detect();
    match mode.as_str() {
        "mp" => cyberpunk::launch(&info, cyberpunk::CyberpunkLaunch::Mp).map_err(err),
        "skip-launcher" => {
            cyberpunk::launch(&info, cyberpunk::CyberpunkLaunch::SkipLauncher).map_err(err)
        }
        _ => {
            // Steam installs go through Steam (overlay etc.); Epic installs
            // launch REDprelauncher directly.
            if info.source.as_deref() == Some("steam") {
                open::that(steam_run_url(cyberpunk::APP_ID)).map_err(err)?;
                Ok(0)
            } else {
                cyberpunk::launch(&info, cyberpunk::CyberpunkLaunch::Vanilla).map_err(err)
            }
        }
    }
}

#[tauri::command]
pub fn set_elden_ring_password(password: String) -> Result<(), String> {
    let info = eldenring::detect();
    let game_dir = info
        .game_dir
        .ok_or_else(|| "Elden Ring is not installed".to_string())?;
    eldenring::set_coop_password(&game_dir, &password).map_err(err)
}

/// One-click install of a game tool/mod from its official GitHub release.
/// `tool`: "seamless" | "modengine2" | "skse" | "cet" | "cyberpunkmp".
#[tauri::command]
pub async fn install_game_tool(app: AppHandle, tool: String) -> Result<String, String> {
    let reporter = std::sync::Arc::new(crate::progress::EventReporter::default());
    let pump_reporter = reporter.clone();
    let pump_app = app.clone();
    let done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let pump_done = done.clone();
    tokio::spawn(async move {
        loop {
            let _ = pump_app.emit("mc-progress", pump_reporter.snapshot());
            if pump_done.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(120)).await;
        }
    });
    let rep: launcher_core::progress::SharedReporter = reporter.clone();
    use launcher_core::progress::Reporter as _;
    reporter.stage("Downloading");

    let outcome = async {
        match tool.as_str() {
            "seamless" => {
                let info = eldenring::detect();
                let game_dir = info
                    .game_dir
                    .filter(|g| g.exists())
                    .ok_or_else(|| launcher_core::Error::other("Elden Ring is not installed"))?;
                let tag = eldenring::install_seamless(&game_dir, &rep).await?;
                Ok(format!("Seamless Co-op {tag} installed"))
            }
            "modengine2" => {
                let info = eldenring::detect();
                let dir = info
                    .install_dir
                    .ok_or_else(|| launcher_core::Error::other("Elden Ring is not installed"))?;
                let tag = eldenring::install_mod_engine(&dir, &rep).await?;
                Ok(format!("Mod Engine 2 {tag} installed"))
            }
            "skse" => {
                let info = skyrim::detect();
                let dir = info
                    .install_dir
                    .ok_or_else(|| launcher_core::Error::other("Skyrim is not installed"))?;
                let tag = skyrim::install_skse(&dir, &rep).await?;
                Ok(format!("SKSE64 {tag} installed"))
            }
            "cet" => {
                let info = cyberpunk::detect();
                let dir = info
                    .install_dir
                    .ok_or_else(|| launcher_core::Error::other("Cyberpunk 2077 is not installed"))?;
                let tag = cyberpunk::install_cet(&dir, &rep).await?;
                Ok(format!("Cyber Engine Tweaks {tag} installed"))
            }
            "cyberpunkmp" => {
                let info = cyberpunk::detect();
                let dir = info
                    .install_dir
                    .ok_or_else(|| launcher_core::Error::other("Cyberpunk 2077 is not installed"))?;
                let tag = cyberpunk::install_mp(&dir, &rep).await?;
                Ok(format!("CyberpunkMP {tag} installed"))
            }
            other => Err(launcher_core::Error::other(format!("unknown tool {other}"))),
        }
    }
    .await;

    done.store(true, std::sync::atomic::Ordering::Relaxed);
    let _ = app.emit("mc-progress", reporter.snapshot());
    outcome.map_err(err)
}

/// Guided Skyrim Together install: find the zip the user downloaded from
/// Nexus (newest `*skyrim*together*.zip` in Downloads, or an explicit path)
/// and extract it into the game folder.
#[tauri::command]
pub async fn install_skyrim_together(path: Option<String>) -> Result<String, String> {
    let info = skyrim::detect();
    let install_dir = info
        .install_dir
        .ok_or_else(|| "Skyrim is not installed".to_string())?;
    let zip = match path.filter(|p| !p.trim().is_empty()) {
        Some(p) => std::path::PathBuf::from(p),
        None => skyrim::find_downloaded_together_zip().ok_or_else(|| {
            "No Skyrim Together zip found in your Downloads folder. Download the main file from \
             the Nexus page first, then press this again."
                .to_string()
        })?,
    };
    if !zip.exists() {
        return Err(format!("File not found: {}", zip.display()));
    }
    let name = zip.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    tokio::task::spawn_blocking(move || skyrim::install_together_from_zip(&install_dir, &zip))
        .await
        .map_err(|e| e.to_string())?
        .map_err(err)?;
    Ok(format!("Skyrim Together Reborn installed from {name}"))
}

/// Open the Skyrim Together Nexus page (its files aren't API-downloadable).
#[tauri::command]
pub fn open_together_page() -> Result<(), String> {
    open::that(skyrim::TOGETHER_NEXUS_URL).map_err(err)
}

/// Guided Address Library install (required by Skyrim Together at runtime).
#[tauri::command]
pub async fn install_address_library(path: Option<String>) -> Result<String, String> {
    let info = skyrim::detect();
    let install_dir = info
        .install_dir
        .ok_or_else(|| "Skyrim is not installed".to_string())?;
    let zip = match path.filter(|p| !p.trim().is_empty()) {
        Some(p) => std::path::PathBuf::from(p),
        None => skyrim::find_downloaded_addrlib_zip().ok_or_else(|| {
            "No Address Library zip found in Downloads. On the Nexus page download \"All in one\" \
             for your Skyrim version (1.6.x for current Steam), then press this again."
                .to_string()
        })?,
    };
    tokio::task::spawn_blocking(move || skyrim::install_address_library_from_zip(&install_dir, &zip))
        .await
        .map_err(|e| e.to_string())?
        .map_err(err)?;
    Ok("Address Library installed".to_string())
}

#[tauri::command]
pub fn open_address_library_page() -> Result<(), String> {
    open::that(skyrim::ADDRESS_LIBRARY_NEXUS_URL).map_err(err)
}

/// Guided Seamless Co-op update from a Nexus zip — used when the mod's own
/// check says the GitHub build is out of date (Nexus gets updates first).
#[tauri::command]
pub async fn install_seamless_update(path: Option<String>) -> Result<String, String> {
    let info = eldenring::detect();
    let game_dir = info
        .game_dir
        .filter(|g| g.exists())
        .ok_or_else(|| "Elden Ring is not installed".to_string())?;
    let zip = match path.filter(|p| !p.trim().is_empty()) {
        Some(p) => std::path::PathBuf::from(p),
        None => eldenring::find_downloaded_seamless_zip().ok_or_else(|| {
            "No Seamless Co-op zip found in Downloads. Download the main file from the Nexus \
             page, then press this again."
                .to_string()
        })?,
    };
    tokio::task::spawn_blocking(move || eldenring::install_seamless_from_zip(&game_dir, &zip))
        .await
        .map_err(|e| e.to_string())?
        .map_err(err)?;
    Ok("Seamless Co-op updated from your downloaded zip".to_string())
}

#[tauri::command]
pub fn open_seamless_page() -> Result<(), String> {
    open::that(eldenring::SEAMLESS_NEXUS_URL).map_err(err)
}

/// Open a folder that detection reported (e.g. the Mod Engine 2 mods dir),
/// creating it first so the click always works.
#[tauri::command]
pub fn open_path(path: String) -> Result<(), String> {
    let p = std::path::PathBuf::from(path);
    let _ = std::fs::create_dir_all(&p);
    open::that(p).map_err(err)
}

// --- Minecraft server hosting (multi-server) ----------------------------

/// A persisted server configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerConfig {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub version: String,
    pub port: u16,
    pub max_players: u32,
    pub max_ram_mb: u32,
    /// "vanilla" | "fabric" — Fabric enables server-side mods.
    #[serde(default)]
    pub loader: Option<String>,
    /// Optional icon (URL or data URI) shown on the server card.
    #[serde(default)]
    pub icon: Option<String>,
}

#[derive(Default, Serialize, Deserialize)]
struct ServersFile {
    #[serde(default)]
    servers: Vec<ServerConfig>,
}

/// Live status of a server (emitted as events and returned by `servers_status`).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerStatus {
    pub id: String,
    pub name: String,
    pub version: String,
    pub running: bool,
    pub players: usize,
    pub max_players: u32,
    pub port: u16,
    pub memory_mb: u64,
}

#[derive(Clone)]
struct ServerMeta {
    id: String,
    name: String,
    version: String,
    port: u16,
    max_players: u32,
}

fn configs_path(state: &AppState) -> std::path::PathBuf {
    state.paths.data_dir.join("servers.json")
}

pub(crate) async fn load_configs(state: &AppState) -> Vec<ServerConfig> {
    match tokio::fs::read(configs_path(state)).await {
        Ok(bytes) => serde_json::from_slice::<ServersFile>(&bytes)
            .map(|f| f.servers)
            .unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

pub(crate) async fn store_configs(state: &AppState, servers: &[ServerConfig]) -> Result<(), String> {
    let path = configs_path(state);
    if let Some(parent) = path.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    let bytes = serde_json::to_vec_pretty(&ServersFile { servers: servers.to_vec() })
        .map_err(err)?;
    tokio::fs::write(&path, bytes).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_servers(state: State<'_, AppState>) -> Result<Vec<ServerConfig>, String> {
    Ok(load_configs(&state).await)
}

/// Resolve a server to (content dir, game version, loader for mods).
/// `loader` is `None` for vanilla (mods don't apply).
pub async fn server_content_target(
    state: &AppState,
    id: &str,
) -> Option<(std::path::PathBuf, String, Option<String>)> {
    let cfg = load_configs(state).await.into_iter().find(|c| c.id == id)?;
    let loader = match cfg.loader.as_deref() {
        Some("fabric") => Some("fabric".to_string()),
        Some("forge") => Some("forge".to_string()),
        Some("paper") => Some("paper".to_string()),
        _ => None,
    };
    Some((
        launcher_core::server::server_dir(&state.paths, id),
        cfg.version,
        loader,
    ))
}

#[tauri::command]
pub async fn save_server(state: State<'_, AppState>, config: ServerConfig) -> Result<(), String> {
    let mut list = load_configs(&state).await;
    match list.iter_mut().find(|c| c.id == config.id) {
        Some(existing) => *existing = config,
        None => list.push(config),
    }
    store_configs(&state, &list).await
}

#[tauri::command]
pub async fn delete_server(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    // Stop it first if running (keep world files on disk).
    stop_server_inner(&app, state.inner(), &id).await;
    let list: Vec<ServerConfig> = load_configs(&state)
        .await
        .into_iter()
        .filter(|c| c.id != id)
        .collect();
    store_configs(&state, &list).await
}

#[tauri::command]
pub async fn servers_status(state: State<'_, AppState>) -> Result<Vec<ServerStatus>, String> {
    let map = state.servers.lock().await;
    let mut out = Vec::new();
    for p in map.values() {
        if p.running.load(Ordering::Relaxed) {
            out.push(ServerStatus {
                id: p.id.clone(),
                name: p.name.clone(),
                version: p.version.clone(),
                running: true,
                players: p.players.lock().await.len(),
                max_players: p.max_players,
                port: p.port,
                memory_mb: p.memory_mb.load(Ordering::Relaxed),
            });
        }
    }
    Ok(out)
}

#[tauri::command]
pub async fn server_start(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    use launcher_core::server;
    use std::process::Stdio;

    {
        let map = state.servers.lock().await;
        if map.get(&id).is_some_and(|p| p.running.load(Ordering::Relaxed)) {
            return Err("That server is already running".into());
        }
    }

    let cfg = load_configs(&state)
        .await
        .into_iter()
        .find(|c| c.id == id)
        .ok_or_else(|| "Server config not found".to_string())?;

    // Fail fast with a clear message if the port is taken (instead of letting
    // the server crash with a BindException).
    if std::net::TcpListener::bind(("0.0.0.0", cfg.port)).is_err() {
        return Err(format!(
            "Port {} is already in use — another server (or app) is using it. Stop it, or change this server's port with Edit.",
            cfg.port
        ));
    }

    let paths = state.paths.clone();
    let reporter = launcher_core::progress::noop();
    let emit_log = |line: String| {
        let _ = app.emit("server-log", serde_json::json!({ "id": &id, "line": line, "err": false }));
    };

    emit_log(format!("Preparing “{}” ({})…", cfg.name, cfg.version));
    let manifest = VersionManifest::fetch().await.map_err(err)?;
    let installer = Installer::new(paths.clone());
    let vj = installer.resolve_version(&manifest, &cfg.version).await.map_err(err)?;

    let dir = server::server_dir(&paths, &id);

    // Java first (the Forge installer needs it too).
    let major = vj.java_version.as_ref().map(|j| j.major_version).unwrap_or(21);
    emit_log(format!("Ensuring Java {major}…"));
    let java = launcher_core::java::ensure_java(&paths, major, &reporter).await.map_err(err)?;

    // Set up the chosen loader and compute how to launch it. We run with
    // cwd = dir and reference files by relative name so the working directory
    // can never break the path.
    let launch_args: Vec<String> = match cfg.loader.as_deref() {
        Some("fabric") => {
            emit_log("Installing Fabric server…".into());
            server::ensure_fabric_server_jar(&dir, &cfg.version, &reporter).await.map_err(err)?;
            vec!["-jar".into(), "server.jar".into()]
        }
        Some("forge") => {
            emit_log("Running the Forge installer — downloads libraries & runs processors, can take a minute…".into());
            let args_file = server::ensure_forge_server(&dir, &cfg.version, &java, &reporter).await.map_err(err)?;
            vec![format!("@{args_file}")]
        }
        Some("paper") => {
            emit_log("Downloading Paper…".into());
            server::ensure_paper_jar(&dir, &cfg.version, &reporter).await.map_err(err)?;
            vec!["-jar".into(), "server.jar".into()]
        }
        _ => {
            server::ensure_server_jar(&dir, &vj, &reporter).await.map_err(err)?;
            vec!["-jar".into(), "server.jar".into()]
        }
    };

    server::accept_eula(&dir).await.map_err(err)?;
    let motd = if cfg.description.is_empty() { &cfg.name } else { &cfg.description };
    server::write_properties(&dir, cfg.port, cfg.max_players, motd).await.map_err(err)?;

    emit_log("Starting server…".into());
    let mut cmd = tokio::process::Command::new(&java);
    cmd.current_dir(&dir)
        .arg(format!("-Xmx{}M", cfg.max_ram_mb))
        .arg(format!("-Xms{}M", (cfg.max_ram_mb / 2).max(512)));
    for a in &launch_args {
        cmd.arg(a);
    }
    cmd.arg("nogui")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW — the dashboard shows the console

    let mut child = cmd.spawn().map_err(|e| e.to_string())?;
    let pid = child.id().unwrap_or(0);
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let stdin = child.stdin.take().unwrap();

    let players = Arc::new(Mutex::new(HashSet::<String>::new()));
    let memory_mb = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let running = Arc::new(AtomicBool::new(true));

    let meta = ServerMeta {
        id: id.clone(),
        name: cfg.name.clone(),
        version: cfg.version.clone(),
        port: cfg.port,
        max_players: cfg.max_players,
    };

    spawn_log_pump(app.clone(), stdout, meta.clone(), players.clone(), memory_mb.clone(), false);
    spawn_log_pump(app.clone(), stderr, meta.clone(), players.clone(), memory_mb.clone(), true);
    spawn_ram_sampler(app.clone(), meta.clone(), pid, players.clone(), memory_mb.clone(), running.clone());

    // Wait-task owns the child so we can report its exit code. It exits when the
    // process ends naturally or when the kill one-shot fires.
    let (kill_tx, kill_rx) = tokio::sync::oneshot::channel::<()>();
    {
        let app = app.clone();
        let meta = meta.clone();
        let running = running.clone();
        tokio::spawn(async move {
            let status = tokio::select! {
                s = child.wait() => s.ok(),
                _ = kill_rx => {
                    let _ = child.start_kill();
                    child.wait().await.ok()
                }
            };
            running.store(false, Ordering::Relaxed);
            let code = status.and_then(|s| s.code());
            let (line, is_err) = match code {
                Some(0) | None => ("Server stopped.".to_string(), false),
                Some(c) => (
                    format!("Server process exited (code {c}). See the log above for the cause."),
                    true,
                ),
            };
            let _ = app.emit("server-log", serde_json::json!({ "id": meta.id, "line": line, "err": is_err }));
            emit_status(&app, &meta, 0, 0, false);
        });
    }

    state.servers.lock().await.insert(
        id.clone(),
        crate::state::ServerProc {
            id: id.clone(),
            name: cfg.name.clone(),
            version: cfg.version.clone(),
            port: cfg.port,
            max_players: cfg.max_players,
            pid,
            stdin,
            kill: Some(kill_tx),
            players,
            memory_mb,
            running,
        },
    );

    emit_status(&app, &meta, 0, 0, true);
    Ok(())
}

#[tauri::command]
pub async fn server_command(
    state: State<'_, AppState>,
    id: String,
    line: String,
) -> Result<(), String> {
    let mut map = state.servers.lock().await;
    if let Some(proc) = map.get_mut(&id) {
        proc.stdin.write_all(format!("{line}\n").as_bytes()).await.map_err(|e| e.to_string())?;
        proc.stdin.flush().await.map_err(|e| e.to_string())?;
        Ok(())
    } else {
        Err("That server is not running".into())
    }
}

#[tauri::command]
pub async fn server_stop(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    stop_server_inner(&app, state.inner(), &id).await;
    Ok(())
}

#[cfg(windows)]
fn kill_pid(pid: u32) {
    let _ = std::process::Command::new("taskkill")
        .args(["/F", "/T", "/PID", &pid.to_string()])
        .output();
}
#[cfg(not(windows))]
fn kill_pid(pid: u32) {
    let _ = std::process::Command::new("kill")
        .args(["-9", &pid.to_string()])
        .output();
}

/// Force-kill every hosted server. Called on launcher exit so server processes
/// never orphan and hold their ports.
pub fn kill_all_servers(app: &AppHandle) {
    let state = app.state::<AppState>();
    let map = state.servers.blocking_lock();
    for proc in map.values() {
        kill_pid(proc.pid);
    }
}

async fn stop_server_inner(app: &AppHandle, state: &AppState, id: &str) {
    let proc = state.servers.lock().await.remove(id);
    if let Some(mut proc) = proc {
        proc.running.store(false, Ordering::Relaxed);
        // Ask the server to save & stop gracefully…
        let _ = proc.stdin.write_all(b"stop\n").await;
        let _ = proc.stdin.flush().await;
        // …and force-kill via the wait-task if it lingers past 10s.
        if let Some(kill) = proc.kill.take() {
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_secs(10)).await;
                let _ = kill.send(());
            });
        }
    }
    let _ = app.emit(
        "server-status",
        ServerStatus {
            id: id.to_string(),
            name: String::new(),
            version: String::new(),
            running: false,
            players: 0,
            max_players: 0,
            port: 0,
            memory_mb: 0,
        },
    );
}

/// Open (or focus) a dashboard window for a specific server.
#[tauri::command]
pub fn open_server_console(app: AppHandle, id: String) -> Result<(), String> {
    let label = format!("console-{id}");
    if let Some(win) = app.get_webview_window(&label) {
        let _ = win.set_focus();
        return Ok(());
    }
    tauri::WebviewWindowBuilder::new(&app, &label, tauri::WebviewUrl::App("index.html".into()))
        .title("Server Dashboard")
        .inner_size(840.0, 600.0)
        .min_inner_size(560.0, 420.0)
        .decorations(false)
        .build()
        .map_err(err)?;
    Ok(())
}

fn emit_status(app: &AppHandle, meta: &ServerMeta, players: usize, mem: u64, running: bool) {
    let _ = app.emit(
        "server-status",
        ServerStatus {
            id: meta.id.clone(),
            name: meta.name.clone(),
            version: meta.version.clone(),
            running,
            players,
            max_players: meta.max_players,
            port: meta.port,
            memory_mb: mem,
        },
    );
}

fn spawn_log_pump<R>(
    app: AppHandle,
    reader: R,
    meta: ServerMeta,
    players: Arc<Mutex<HashSet<String>>>,
    memory_mb: Arc<std::sync::atomic::AtomicU64>,
    is_err: bool,
) where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut lines = BufReader::new(reader).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let _ = app.emit("server-log", serde_json::json!({ "id": meta.id, "line": line, "err": is_err }));
            if let Some((joined, name)) = launcher_core::server::parse_player_event(&line) {
                let count = {
                    let mut set = players.lock().await;
                    if joined {
                        set.insert(name);
                    } else {
                        set.remove(&name);
                    }
                    set.len()
                };
                emit_status(&app, &meta, count, memory_mb.load(Ordering::Relaxed), true);
            }
        }
        // Final "stopped"/exit-code status is emitted by the wait-task.
    });
}

fn spawn_ram_sampler(
    app: AppHandle,
    meta: ServerMeta,
    pid: u32,
    players: Arc<Mutex<HashSet<String>>>,
    memory_mb: Arc<std::sync::atomic::AtomicU64>,
    running: Arc<AtomicBool>,
) {
    if pid == 0 {
        return;
    }
    tokio::spawn(async move {
        use sysinfo::{Pid, ProcessesToUpdate, System};
        let mut sys = System::new();
        let p = Pid::from_u32(pid);
        while running.load(Ordering::Relaxed) {
            sys.refresh_processes(ProcessesToUpdate::Some(&[p]), true);
            match sys.process(p) {
                Some(proc) => {
                    let mb = proc.memory() / 1_048_576;
                    memory_mb.store(mb, Ordering::Relaxed);
                    let count = players.lock().await.len();
                    emit_status(&app, &meta, count, mb, true);
                }
                None => break,
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    });
}
