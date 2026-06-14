//! Minecraft *instances* — multiple isolated profiles (different versions,
//! loaders, modpacks), mirroring how servers work.
//!
//! The install (versions/libraries/assets) is shared in the main game dir; each
//! instance gets its own directory under `<data>/instances/<id>` used as the
//! game's `--gameDir`, so saves/mods/config/resourcepacks/shaderpacks are
//! isolated per instance.

use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use launcher_core::account::AccountStore;
use launcher_core::auth::Auth;
use launcher_core::launch::{self, LaunchOptions};
use launcher_core::manifest::VersionManifest;
use launcher_core::modloader::{fabric, forge, neoforge, quilt};
use launcher_core::modpacks;
use launcher_core::modrinth;
use launcher_core::platform::Environment;
use launcher_core::progress::{Reporter, SharedReporter};
use launcher_core::{java, Installer};

use crate::progress::EventReporter;
use crate::state::AppState;

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

/// Read the last `n` non-empty lines of a log file (best-effort), for surfacing
/// the reason a freshly-launched game crashed on startup.
fn tail_of(path: &std::path::Path, n: usize) -> String {
    match std::fs::read_to_string(path) {
        Ok(text) => {
            let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
            let start = lines.len().saturating_sub(n);
            lines[start..].join("\n")
        }
        Err(_) => "(no log captured)".to_string(),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceConfig {
    pub id: String,
    pub name: String,
    pub version: String,
    /// "vanilla" | "fabric" | "quilt"
    #[serde(default)]
    pub loader: String,
    pub max_ram_mb: u32,
    /// Optional icon (URL or data URI) shown on the instance card.
    #[serde(default)]
    pub icon: Option<String>,
}

#[derive(Default, Serialize, Deserialize)]
struct InstancesFile {
    #[serde(default)]
    instances: Vec<InstanceConfig>,
}

fn instances_path(state: &AppState) -> PathBuf {
    state.paths.data_dir.join("instances.json")
}

/// The per-instance game directory (its `--gameDir`).
pub fn instance_dir(state: &AppState, id: &str) -> PathBuf {
    state.paths.data_dir.join("instances").join(id)
}

/// Resolve an instance to (content dir, game version, loader for mods).
/// `loader` is `None` for vanilla (mods don't apply).
pub async fn instance_content_target(
    state: &AppState,
    id: &str,
) -> Option<(PathBuf, String, Option<String>)> {
    let cfg = load_instances(state).await.into_iter().find(|c| c.id == id)?;
    let loader = match cfg.loader.as_str() {
        "fabric" => Some("fabric".to_string()),
        "quilt" => Some("quilt".to_string()),
        "forge" => Some("forge".to_string()),
        "neoforge" => Some("neoforge".to_string()),
        _ => None,
    };
    Some((instance_dir(state, id), cfg.version, loader))
}

async fn load_instances(state: &AppState) -> Vec<InstanceConfig> {
    match tokio::fs::read(instances_path(state)).await {
        Ok(b) => serde_json::from_slice::<InstancesFile>(&b)
            .map(|f| f.instances)
            .unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

async fn store_instances(state: &AppState, instances: &[InstanceConfig]) -> Result<(), String> {
    let path = instances_path(state);
    if let Some(p) = path.parent() {
        let _ = tokio::fs::create_dir_all(p).await;
    }
    let bytes = serde_json::to_vec_pretty(&InstancesFile {
        instances: instances.to_vec(),
    })
    .map_err(err)?;
    tokio::fs::write(&path, bytes).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_instances(state: State<'_, AppState>) -> Result<Vec<InstanceConfig>, String> {
    Ok(load_instances(&state).await)
}

#[tauri::command]
pub async fn save_instance(state: State<'_, AppState>, config: InstanceConfig) -> Result<(), String> {
    let mut list = load_instances(&state).await;
    match list.iter_mut().find(|c| c.id == config.id) {
        Some(existing) => *existing = config,
        None => list.push(config),
    }
    store_instances(&state, &list).await
}

#[tauri::command]
pub async fn delete_instance(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let list: Vec<InstanceConfig> = load_instances(&state)
        .await
        .into_iter()
        .filter(|c| c.id != id)
        .collect();
    store_instances(&state, &list).await
    // Instance files are left on disk so worlds aren't destroyed by accident.
}

#[tauri::command]
pub fn open_instance_folder(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let dir = instance_dir(&state, &id);
    let _ = std::fs::create_dir_all(&dir);
    open::that(dir).map_err(err)
}

#[tauri::command]
pub fn open_server_folder(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let dir = launcher_core::server::server_dir(&state.paths, &id);
    let _ = std::fs::create_dir_all(&dir);
    open::that(dir).map_err(err)
}

/// Most-downloaded modpacks (for quick-create).
#[tauri::command]
pub async fn popular_modpacks() -> Result<Vec<modrinth::SearchHit>, String> {
    modrinth::popular("modpack", 30).await.map_err(err)
}

/// Create a new instance from a Modrinth modpack: derive its version + loader,
/// then install the pack into the instance directory.
#[tauri::command]
pub async fn create_instance_from_modpack(
    app: AppHandle,
    state: State<'_, AppState>,
    project_id: String,
    title: String,
) -> Result<InstanceConfig, String> {
    let v = modrinth::latest_any(&project_id)
        .await
        .map_err(err)?
        .ok_or_else(|| "That modpack has no published versions".to_string())?;
    let version = v
        .game_versions
        .first()
        .cloned()
        .ok_or_else(|| "Modpack has no Minecraft version".to_string())?;
    let loader = v
        .loaders
        .iter()
        .find(|l| matches!(l.as_str(), "fabric" | "quilt" | "forge" | "neoforge"))
        .cloned()
        .unwrap_or_else(|| "vanilla".to_string());

    let cfg = InstanceConfig {
        id: format!("mp-{project_id}"),
        name: title,
        version,
        loader,
        max_ram_mb: 6144,
        icon: None,
    };

    let mut list = load_instances(&state).await;
    match list.iter_mut().find(|c| c.id == cfg.id) {
        Some(e) => *e = cfg.clone(),
        None => list.push(cfg.clone()),
    }
    store_instances(&state, &list).await?;

    let inst = instance_dir(&state, &cfg.id);
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
    reporter.stage("Installing modpack");
    let res = modrinth::install_modpack(&inst, &v, &rep).await;
    done.store(true, Ordering::Relaxed);
    let _ = app.emit("mc-progress", reporter.snapshot());
    res.map_err(err)?;
    let _ = app.emit("mc-done", serde_json::json!({ "message": format!("Installed {}", cfg.name) }));
    Ok(cfg)
}

/// Search any modpack platform: "modrinth" | "ftb" | "curseforge" | "technic".
#[tauri::command]
pub async fn pack_search(source: String, query: String) -> Result<Vec<modpacks::PackHit>, String> {
    use launcher_core::modpacks::{curseforge, ftb, technic};
    match source.as_str() {
        "ftb" => ftb::search(&query).await.map_err(err),
        "technic" => technic::search(&query).await.map_err(err),
        "curseforge" => {
            // CurseForge blocks /mods/search on personal keys, so we surface a
            // curated set (or a typed Project ID) via the allowed /mods/{id}.
            const CURATED: &[i64] = &[925200, 715572, 520914, 285109];
            let ids: Vec<i64> = match query.trim().parse::<i64>() {
                Ok(id) => vec![id],
                Err(_) => CURATED.to_vec(),
            };
            curseforge::by_ids(&ids, crate::secrets::CURSEFORGE_API_KEY).await.map_err(err)
        }
        _ => {
            let hits = if query.trim().is_empty() {
                modrinth::popular("modpack", 30).await
            } else {
                modrinth::search(&query, "modpack", None, None, 30).await
            }
            .map_err(err)?;
            Ok(hits
                .into_iter()
                .map(|h| modpacks::PackHit {
                    id: h.project_id,
                    name: h.title,
                    summary: h.description,
                    icon: h.icon_url,
                    downloads: h.downloads,
                })
                .collect())
        }
    }
}

/// Create an instance from a modpack on any platform, installing into its dir.
#[tauri::command]
pub async fn create_instance_from_pack(
    app: AppHandle,
    state: State<'_, AppState>,
    source: String,
    project_id: String,
    title: String,
    icon: Option<String>,
) -> Result<InstanceConfig, String> {
    use launcher_core::modpacks::{curseforge, ftb, technic};

    let safe_id: String = project_id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect();
    let id = format!("{source}-{safe_id}");
    let inst = instance_dir(&state, &id);

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
    reporter.stage("Installing modpack");

    let outcome: std::result::Result<(String, String), launcher_core::Error> = async {
        match source.as_str() {
            "ftb" => {
                let p = ftb::install(&inst, &project_id, &rep).await?;
                Ok((p.version, p.loader))
            }
            "technic" => {
                let p = technic::install(&inst, &project_id, &rep).await?;
                Ok((p.version, p.loader))
            }
            "curseforge" => {
                let p = curseforge::install(&inst, &project_id, crate::secrets::CURSEFORGE_API_KEY, &rep).await?;
                Ok((p.version, p.loader))
            }
            _ => {
                let v = modrinth::latest_any(&project_id)
                    .await?
                    .ok_or_else(|| launcher_core::Error::other("That modpack has no published versions"))?;
                let version = v
                    .game_versions
                    .first()
                    .cloned()
                    .ok_or_else(|| launcher_core::Error::other("Modpack has no Minecraft version"))?;
                let loader = v
                    .loaders
                    .iter()
                    .find(|l| matches!(l.as_str(), "fabric" | "quilt" | "forge" | "neoforge"))
                    .cloned()
                    .unwrap_or_else(|| "vanilla".to_string());
                modrinth::install_modpack(&inst, &v, &rep).await?;
                Ok((version, loader))
            }
        }
    }
    .await;

    done.store(true, Ordering::Relaxed);
    let _ = app.emit("mc-progress", reporter.snapshot());
    let (version, loader) = outcome.map_err(err)?;

    let cfg = InstanceConfig { id, name: title, version, loader, max_ram_mb: 6144, icon };
    let mut list = load_instances(&state).await;
    match list.iter_mut().find(|c| c.id == cfg.id) {
        Some(e) => *e = cfg.clone(),
        None => list.push(cfg.clone()),
    }
    store_instances(&state, &list).await?;
    let _ = app.emit("mc-done", serde_json::json!({ "message": format!("Installed {}", cfg.name) }));
    Ok(cfg)
}

/// Create a hosted server from a modpack, installing its mods into the server dir.
#[tauri::command]
pub async fn create_server_from_pack(
    app: AppHandle,
    state: State<'_, AppState>,
    source: String,
    project_id: String,
    title: String,
    icon: Option<String>,
) -> Result<crate::commands::ServerConfig, String> {
    use launcher_core::modpacks::{curseforge, ftb, technic};

    let safe_id: String = project_id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect();
    let id = format!("srv-{source}-{safe_id}");
    let dir = launcher_core::server::server_dir(&state.paths, &id);

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
    reporter.stage("Installing modpack");

    let outcome: std::result::Result<(String, String), launcher_core::Error> = async {
        match source.as_str() {
            "ftb" => {
                let p = ftb::install(&dir, &project_id, &rep).await?;
                Ok((p.version, p.loader))
            }
            "technic" => {
                let p = technic::install(&dir, &project_id, &rep).await?;
                Ok((p.version, p.loader))
            }
            "curseforge" => {
                let p = curseforge::install(&dir, &project_id, crate::secrets::CURSEFORGE_API_KEY, &rep).await?;
                Ok((p.version, p.loader))
            }
            _ => {
                let v = modrinth::latest_any(&project_id)
                    .await?
                    .ok_or_else(|| launcher_core::Error::other("That modpack has no published versions"))?;
                let version = v.game_versions.first().cloned().ok_or_else(|| launcher_core::Error::other("Modpack has no Minecraft version"))?;
                let loader = v
                    .loaders
                    .iter()
                    .find(|l| matches!(l.as_str(), "fabric" | "quilt" | "forge" | "neoforge"))
                    .cloned()
                    .unwrap_or_else(|| "vanilla".to_string());
                modrinth::install_modpack(&dir, &v, &rep).await?;
                Ok((version, loader))
            }
        }
    }
    .await;

    done.store(true, Ordering::Relaxed);
    let _ = app.emit("mc-progress", reporter.snapshot());
    let (version, loader) = outcome.map_err(err)?;

    // Server hosting supports Vanilla / Fabric / Forge (+ Paper). Reject loaders
    // we can't run rather than silently launching the wrong jar.
    let server_loader = match loader.as_str() {
        "forge" => Some("forge".to_string()),
        "fabric" => Some("fabric".to_string()),
        "vanilla" => None,
        other => {
            return Err(format!(
                "This pack uses {other}, which isn't supported for hosting yet (the mods downloaded to the server folder, but Aurora can only run Vanilla, Fabric or Forge servers)."
            ))
        }
    };

    let existing = crate::commands::load_configs(&state).await;
    let port = 25565 + existing.len() as u16;
    let cfg = crate::commands::ServerConfig {
        id,
        name: title,
        description: String::new(),
        version,
        port,
        max_players: 20,
        max_ram_mb: 6144,
        loader: server_loader,
        icon,
    };
    let mut list = existing;
    match list.iter_mut().find(|c| c.id == cfg.id) {
        Some(e) => *e = cfg.clone(),
        None => list.push(cfg.clone()),
    }
    crate::commands::store_configs(&state, &list).await?;
    let _ = app.emit("mc-done", serde_json::json!({ "message": format!("Installed {}", cfg.name) }));
    Ok(cfg)
}

#[tauri::command]
pub async fn instance_play(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<String, String> {
    let cfg = load_instances(&state)
        .await
        .into_iter()
        .find(|c| c.id == id)
        .ok_or_else(|| "Instance not found".to_string())?;

    let paths = state.paths.clone();
    let inst_dir = instance_dir(&state, &id);

    let accounts_path = paths.accounts_file();
    let mut store = AccountStore::load(&accounts_path).await.map_err(err)?;
    let active = store
        .active()
        .cloned()
        .ok_or_else(|| "Add an account first (top-right menu)".to_string())?;

    // For Microsoft accounts, silently refresh the access token before
    // launching so the session is genuinely *online*: multiplayer works and
    // mods like Fabulously Optimized's Crash Assistant won't flag "offline
    // mode / unsupported launcher". Tokens expire after ~24h. A refresh failure
    // is non-fatal — we fall back to the stored token (the game still runs;
    // online features just may not).
    let account = if active.account.is_online() && !active.refresh_token.is_empty() {
        match Auth::new(crate::secrets::AZURE_CLIENT_ID.to_string())
            .login_with_refresh(&active.refresh_token)
            .await
        {
            Ok(res) => {
                store.upsert(res.account.clone(), res.refresh_token);
                let _ = store.save(&accounts_path).await;
                res.account
            }
            Err(e) => {
                eprintln!("[instance_play] token refresh failed, using stored token: {e}");
                active.account.clone()
            }
        }
    } else {
        active.account.clone()
    };

    // Progress pump → "mc-progress" events (shared with the UI's existing bar).
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
        let manifest = VersionManifest::fetch().await?;
        let installer = Installer::new(paths.clone());

        // Java first — the Forge/NeoForge client installer needs it.
        let vanilla = installer.resolve_version(&manifest, &cfg.version).await?;
        let major = vanilla
            .java_version
            .as_ref()
            .map(|j| j.major_version)
            .unwrap_or(21);
        let java = java::ensure_java(&paths, major, &rep).await?;

        // Resolve loader → launchable version id (shared install).
        let version_id = match cfg.loader.as_str() {
            "fabric" => {
                let lv = fabric::latest_stable(&cfg.version).await?;
                fabric::install(&paths, &cfg.version, &lv).await?
            }
            "quilt" => {
                let lv = quilt::latest_stable(&cfg.version).await?;
                quilt::install(&paths, &cfg.version, &lv).await?
            }
            "forge" => {
                reporter.stage("Installing Forge");
                forge::install_client(&paths.game_dir, &cfg.version, &java, &rep).await?
            }
            "neoforge" => {
                reporter.stage("Installing NeoForge");
                neoforge::install_client(&paths.game_dir, &cfg.version, &java, &rep).await?
            }
            _ => cfg.version.clone(),
        };

        let version = installer.resolve_version(&manifest, &version_id).await?;
        let installed = installer.install(&version, rep.clone()).await?;

        // Make sure the instance dir + mods folder exist.
        let _ = tokio::fs::create_dir_all(inst_dir.join("mods")).await;

        reporter.stage("Launching");
        let options = LaunchOptions {
            max_memory_mb: cfg.max_ram_mb,
            game_directory: Some(inst_dir.clone()),
            launcher_name: "Aurora Launcher".to_string(),
            ..Default::default()
        };
        let env = Environment::detect();

        // Capture the game's stdout/stderr to a log file in the instance dir.
        // The Tauri app has no console (`windows_subsystem = "windows"`), so
        // without this an early crash would vanish silently and we'd falsely
        // report "Launched". The file also lets the user inspect crashes later.
        let log_path = inst_dir.join("aurora-launch.log");
        let mut command = launch::build_command(&installed, &paths, &java, &account, &options, &env)?;
        if let Ok(file) = std::fs::File::create(&log_path) {
            if let Ok(file2) = file.try_clone() {
                command.stdout(Stdio::from(file));
                command.stderr(Stdio::from(file2));
            }
        }

        let mut child = command
            .spawn()
            .map_err(|e| launcher_core::Error::other(format!("failed to start Java: {e}")))?;
        let pid = child.id();

        // Give the JVM a moment to fall over (bad classpath, missing natives,
        // an incompatible mod). If it exits non-zero almost immediately, the
        // launch really failed — surface the tail of the log instead of lying.
        match tokio::time::timeout(Duration::from_millis(4000), child.wait()).await {
            Ok(Ok(status)) if !status.success() => {
                let tail = tail_of(&log_path, 24);
                let code = status.code().map(|c| c.to_string()).unwrap_or_else(|| "signal".into());
                return Err(launcher_core::Error::other(format!(
                    "Minecraft exited immediately (code {code}). Last output:\n{tail}"
                )));
            }
            // Still running after the grace window (the normal case) — detach
            // and let it run. Dropping a tokio Child does not kill it.
            Err(_) => drop(child),
            // Exited cleanly/instantly, or wait errored — treat as launched.
            _ => {}
        }
        Ok::<_, launcher_core::Error>(pid)
    };

    let outcome = run.await;
    done.store(true, Ordering::Relaxed);
    let _ = app.emit("mc-progress", reporter.snapshot());

    match outcome {
        Ok(pid) => {
            crate::stats::record_session(
                &paths.data_dir,
                &format!("instance:{}", cfg.id),
                &cfg.name,
                "instance",
                cfg.icon.clone(),
                pid.unwrap_or(0),
            );
            let msg = format!("Launched {}", cfg.name);
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
