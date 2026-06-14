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

// --- Crash analyzer ------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CrashInfo {
    pub found: bool,
    /// Crash-report mtime (unix secs) — lets the UI ignore stale reports.
    pub when: u64,
    /// The crash report's "Description" line.
    pub title: String,
    /// Friendly name of the mod we think caused it.
    pub culprit_name: Option<String>,
    /// The jar in mods/ to disable (file name only).
    pub culprit_file: Option<String>,
    pub report_path: String,
}

/// Find the third-party mod token most likely responsible for a crash.
fn crash_culprit_token(text: &str) -> Option<String> {
    // 1) "Description: Failed to initialize Controlify" → "controlify".
    for line in text.lines() {
        if let Some(rest) = line.trim().strip_prefix("Description:") {
            if let Some(name) = rest.trim().strip_prefix("Failed to initialize ") {
                let tok = name.trim().split_whitespace().next().unwrap_or("").to_lowercase();
                if !tok.is_empty() {
                    return Some(tok);
                }
            }
        }
    }
    // 2) First stack frame in a third-party package (skip vanilla/loader/jdk).
    for line in text.lines() {
        let l = line.trim();
        if !l.starts_with("at ") {
            continue;
        }
        let pkg = l.trim_start_matches("at ").split("//").last().unwrap_or("");
        let segs: Vec<&str> = pkg.split('.').collect();
        if segs.len() < 3 {
            continue;
        }
        let (root, second) = (segs[0], segs[1]);
        let skip = matches!(root, "java" | "javax" | "sun" | "jdk" | "kotlin" | "knot")
            || (root == "net" && matches!(second, "minecraft" | "fabricmc"))
            || (root == "com" && second == "mojang");
        if matches!(root, "dev" | "com" | "io" | "me" | "org" | "net" | "gg" | "fr") && !skip {
            // e.g. dev.isxander.controlify → "controlify"; com.x.coolmod → "coolmod"
            let tok = segs[2].to_lowercase();
            if tok.len() > 2 {
                return Some(tok);
            }
        }
    }
    None
}

#[tauri::command]
pub fn analyze_crash(state: State<'_, AppState>, id: String) -> Result<CrashInfo, String> {
    let inst = instance_dir(&state, &id);
    let dir = inst.join("crash-reports");
    let newest = std::fs::read_dir(&dir)
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().map(|x| x == "txt").unwrap_or(false))
        .max_by_key(|e| e.metadata().and_then(|m| m.modified()).ok());

    let empty = CrashInfo {
        found: false,
        when: 0,
        title: String::new(),
        culprit_name: None,
        culprit_file: None,
        report_path: String::new(),
    };
    let Some(entry) = newest else { return Ok(empty) };

    let path = entry.path();
    let when = entry
        .metadata()
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let title = text
        .lines()
        .find_map(|l| l.trim().strip_prefix("Description:").map(|s| s.trim().to_string()))
        .unwrap_or_else(|| "Minecraft crashed".into());

    let token = crash_culprit_token(&text);
    let (culprit_name, culprit_file) = match token {
        Some(tok) => {
            let jar = std::fs::read_dir(inst.join("mods"))
                .ok()
                .into_iter()
                .flatten()
                .flatten()
                .filter_map(|e| e.file_name().to_str().map(String::from))
                .find(|f| f.to_lowercase().ends_with(".jar") && f.to_lowercase().contains(&tok));
            // Friendly name: prefer the word from the Description, else the token.
            let name = title
                .strip_prefix("Failed to initialize ")
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|| {
                    let mut c = tok.chars();
                    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or(tok.clone())
                });
            (Some(name), jar)
        }
        None => (None, None),
    };

    Ok(CrashInfo {
        found: true,
        when,
        title,
        culprit_name,
        culprit_file,
        report_path: path.to_string_lossy().into_owned(),
    })
}

/// Disable a mod by renaming its jar to `.jar.disabled` (Fabric ignores it).
#[tauri::command]
pub fn disable_mod(state: State<'_, AppState>, id: String, file: String) -> Result<(), String> {
    if file.contains('/') || file.contains('\\') || file.contains("..") {
        return Err("Invalid file name".into());
    }
    let mods = instance_dir(&state, &id).join("mods");
    std::fs::rename(mods.join(&file), mods.join(format!("{file}.disabled"))).map_err(err)
}

// --- Import / export instances -------------------------------------------

/// Zip an instance's content (mods/config/packs) to `<data>/exports/<name>.zip`
/// and reveal the folder. Returns the zip path.
#[tauri::command]
pub async fn export_instance(state: State<'_, AppState>, id: String) -> Result<String, String> {
    let cfg = load_instances(&state)
        .await
        .into_iter()
        .find(|c| c.id == id)
        .ok_or("Instance not found")?;
    let dir = instance_dir(&state, &id);
    let safe: String = cfg
        .name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect();
    let exports = state.paths.data_dir.join("exports");
    let dest = exports.join(format!("{safe}.zip"));
    let dest2 = dest.clone();
    let added = tokio::task::spawn_blocking(move || {
        launcher_core::backup::create(&dir, &["config", "mods", "resourcepacks", "shaderpacks"], &dest2)
    })
    .await
    .map_err(err)?
    .map_err(err)?;
    if added == 0 {
        let _ = std::fs::remove_file(&dest);
        return Err("Nothing to export yet — add some mods/config first.".into());
    }
    let _ = open::that(&exports);
    Ok(dest.to_string_lossy().into_owned())
}

/// Create a new instance from a local `.mrpack` file's bytes.
#[tauri::command]
pub async fn import_mrpack(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
    bytes: Vec<u8>,
) -> Result<InstanceConfig, String> {
    let stem = std::path::Path::new(&name)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Imported pack".into());
    let safe_id: String = stem
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect();
    let id = format!("import-{safe_id}");
    let inst = instance_dir(&state, &id);
    let tmp = std::env::temp_dir().join("aurora-import.mrpack");
    tokio::fs::write(&tmp, &bytes).await.map_err(err)?;

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
    reporter.stage("Importing modpack");

    let meta = modrinth::install_mrpack_path(&tmp, &inst, &rep).await;
    done.store(true, Ordering::Relaxed);
    let _ = app.emit("mc-progress", reporter.snapshot());
    let _ = tokio::fs::remove_file(&tmp).await;
    let meta = meta.map_err(err)?;

    let cfg = InstanceConfig {
        id,
        name: stem,
        version: if meta.minecraft.is_empty() { "1.21.1".into() } else { meta.minecraft },
        loader: meta.loader,
        max_ram_mb: 6144,
        icon: None,
    };
    let mut list = load_instances(&state).await;
    match list.iter_mut().find(|c| c.id == cfg.id) {
        Some(e) => *e = cfg.clone(),
        None => list.push(cfg.clone()),
    }
    store_instances(&state, &list).await?;
    let _ = app.emit("mc-done", serde_json::json!({ "message": format!("Imported {}", cfg.name) }));
    Ok(cfg)
}

// --- Server whitelist / ops manager --------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessMember {
    pub name: String,
    pub uuid: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerAccess {
    pub whitelist: Vec<AccessMember>,
    pub ops: Vec<AccessMember>,
}

fn load_raw(path: &std::path::Path) -> Vec<serde_json::Value> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn read_members(path: &std::path::Path) -> Vec<AccessMember> {
    load_raw(path)
        .iter()
        .filter_map(|v| {
            Some(AccessMember {
                name: v.get("name")?.as_str()?.to_string(),
                uuid: v.get("uuid").and_then(|x| x.as_str()).unwrap_or("").to_string(),
            })
        })
        .collect()
}

fn dash_uuid(id: &str) -> String {
    if id.len() == 32 {
        format!("{}-{}-{}-{}-{}", &id[0..8], &id[8..12], &id[12..16], &id[16..20], &id[20..32])
    } else {
        id.to_string()
    }
}

/// Resolve a Minecraft username → (dashed uuid, canonical name) via Mojang.
async fn resolve_uuid(name: &str) -> Result<(String, String), String> {
    let resp = launcher_core::http::client()
        .get(format!("https://api.mojang.com/users/profiles/minecraft/{name}"))
        .send()
        .await
        .map_err(err)?;
    if matches!(
        resp.status(),
        reqwest::StatusCode::NO_CONTENT | reqwest::StatusCode::NOT_FOUND
    ) {
        return Err(format!("No Minecraft account named \"{name}\""));
    }
    let v: serde_json::Value = resp.error_for_status().map_err(err)?.json().await.map_err(err)?;
    let id = v.get("id").and_then(|x| x.as_str()).ok_or("Mojang returned no id")?;
    let canon = v.get("name").and_then(|x| x.as_str()).unwrap_or(name).to_string();
    Ok((dash_uuid(id), canon))
}

#[tauri::command]
pub fn server_access(state: State<'_, AppState>, id: String) -> Result<ServerAccess, String> {
    let dir = launcher_core::server::server_dir(&state.paths, &id);
    Ok(ServerAccess {
        whitelist: read_members(&dir.join("whitelist.json")),
        ops: read_members(&dir.join("ops.json")),
    })
}

fn save_raw(path: &std::path::Path, arr: &[serde_json::Value]) -> Result<(), String> {
    if let Some(p) = path.parent() {
        let _ = std::fs::create_dir_all(p);
    }
    std::fs::write(path, serde_json::to_vec_pretty(arr).map_err(err)?).map_err(err)
}

#[tauri::command]
pub async fn access_add(
    state: State<'_, AppState>,
    id: String,
    list: String, // "whitelist" | "ops"
    name: String,
) -> Result<AccessMember, String> {
    let dir = launcher_core::server::server_dir(&state.paths, &id);
    let (uuid, canon) = resolve_uuid(name.trim()).await?;
    let file = if list == "ops" { "ops.json" } else { "whitelist.json" };
    let path = dir.join(file);
    let mut arr = load_raw(&path);
    if !arr.iter().any(|v| v.get("uuid").and_then(|x| x.as_str()) == Some(uuid.as_str())) {
        arr.push(if list == "ops" {
            serde_json::json!({ "uuid": uuid, "name": canon, "level": 4, "bypassesPlayerLimit": false })
        } else {
            serde_json::json!({ "uuid": uuid, "name": canon })
        });
        save_raw(&path, &arr)?;
    }
    Ok(AccessMember { name: canon, uuid })
}

#[tauri::command]
pub fn access_remove(
    state: State<'_, AppState>,
    id: String,
    list: String,
    uuid: String,
) -> Result<(), String> {
    let dir = launcher_core::server::server_dir(&state.paths, &id);
    let file = if list == "ops" { "ops.json" } else { "whitelist.json" };
    let path = dir.join(file);
    let mut arr = load_raw(&path);
    arr.retain(|v| v.get("uuid").and_then(|x| x.as_str()) != Some(uuid.as_str()));
    save_raw(&path, &arr)
}

// --- Built-in config / text editor --------------------------------------

const TEXT_EXTS: &[&str] = &[
    "json", "json5", "yaml", "yml", "toml", "properties", "cfg", "conf", "config", "ini", "txt",
    "js", "mjs", "cjs", "ts", "jsx", "tsx", "py", "lua", "md", "snbt", "xml", "html", "css", "sh",
    "bat", "ps1", "csv", "log", "env", "mcmeta",
];

fn is_text_file(p: &std::path::Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .map(|e| TEXT_EXTS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

/// The directory whose text files the editor may browse/edit.
fn editor_root(state: &AppState, kind: &str, id: &str) -> PathBuf {
    if kind == "server" {
        launcher_core::server::server_dir(&state.paths, id)
    } else {
        instance_dir(state, id)
    }
}

fn walk_text(root: &std::path::Path, dir: &std::path::Path, out: &mut Vec<String>, depth: usize) {
    if depth > 7 || out.len() > 1500 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let name = e.file_name().to_string_lossy().to_string();
        if p.is_dir() {
            // Skip heavy/binary trees (world region data, vcs, extracted natives).
            if matches!(name.as_str(), "saves" | ".git" | "natives" | "crash-reports") {
                continue;
            }
            walk_text(root, &p, out, depth + 1);
        } else if is_text_file(&p) {
            if e.metadata().map(|m| m.len() > 4_000_000).unwrap_or(false) {
                continue; // don't try to edit multi-MB files in the browser
            }
            if let Ok(rel) = p.strip_prefix(root) {
                out.push(rel.to_string_lossy().replace('\\', "/"));
            }
        }
    }
}

/// List editable text/config files (relative paths) under an instance/server.
#[tauri::command]
pub fn list_config_files(state: State<'_, AppState>, kind: String, id: String) -> Result<Vec<String>, String> {
    let root = editor_root(&state, &kind, &id);
    let mut out = Vec::new();
    walk_text(&root, &root, &mut out, 0);
    out.sort();
    Ok(out)
}

/// Resolve `rel` under `root`, refusing anything that escapes the folder.
fn resolve_in(root: &std::path::Path, rel: &str) -> Result<PathBuf, String> {
    let full = root.join(rel);
    let canon_root = std::fs::canonicalize(root).map_err(err)?;
    let canon_full = std::fs::canonicalize(&full).map_err(|_| "File not found".to_string())?;
    if canon_full.starts_with(&canon_root) {
        Ok(canon_full)
    } else {
        Err("That path is outside the allowed folder".into())
    }
}

#[tauri::command]
pub fn read_config_file(
    state: State<'_, AppState>,
    kind: String,
    id: String,
    path: String,
) -> Result<String, String> {
    let root = editor_root(&state, &kind, &id);
    std::fs::read_to_string(resolve_in(&root, &path)?).map_err(err)
}

#[tauri::command]
pub fn write_config_file(
    state: State<'_, AppState>,
    kind: String,
    id: String,
    path: String,
    content: String,
) -> Result<(), String> {
    let root = editor_root(&state, &kind, &id);
    std::fs::write(resolve_in(&root, &path)?, content).map_err(err)
}

// --- World backups -------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupInfo {
    /// File name (also the id used to restore/delete).
    pub file: String,
    pub size: u64,
    /// Created time (unix seconds), parsed from the filename.
    pub created: u64,
}

/// `<data>/backups/<kind>-<id>/`
fn backups_dir(state: &AppState, kind: &str, id: &str) -> PathBuf {
    state.paths.data_dir.join("backups").join(format!("{kind}-{id}"))
}

/// The directory whose worlds we back up, and which subfolders hold worlds.
fn backup_target(state: &AppState, kind: &str, id: &str) -> (PathBuf, Vec<&'static str>) {
    if kind == "server" {
        (
            launcher_core::server::server_dir(&state.paths, id),
            vec!["world", "world_nether", "world_the_end"],
        )
    } else {
        (instance_dir(state, id), vec!["saves"])
    }
}

#[tauri::command]
pub async fn list_backups(
    state: State<'_, AppState>,
    kind: String,
    id: String,
) -> Result<Vec<BackupInfo>, String> {
    let dir = backups_dir(&state, &kind, &id);
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if !name.ends_with(".zip") {
                continue;
            }
            let size = e.metadata().map(|m| m.len()).unwrap_or(0);
            // filename: backup-<epoch>.zip
            let created = name
                .trim_start_matches("backup-")
                .trim_end_matches(".zip")
                .parse()
                .unwrap_or(0);
            out.push(BackupInfo { file: name, size, created });
        }
    }
    out.sort_by(|a, b| b.created.cmp(&a.created));
    Ok(out)
}

#[tauri::command]
pub async fn create_backup(
    state: State<'_, AppState>,
    kind: String,
    id: String,
) -> Result<BackupInfo, String> {
    let (root, include) = backup_target(&state, &kind, &id);
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let file = format!("backup-{secs}.zip");
    let dest = backups_dir(&state, &kind, &id).join(&file);
    let dest2 = dest.clone();
    let added = tokio::task::spawn_blocking(move || launcher_core::backup::create(&root, &include, &dest2))
        .await
        .map_err(err)?
        .map_err(err)?;
    if added == 0 {
        let _ = std::fs::remove_file(&dest);
        return Err("No worlds found to back up yet — play once first.".into());
    }
    let size = std::fs::metadata(&dest).map(|m| m.len()).unwrap_or(0);
    Ok(BackupInfo { file, size, created: secs })
}

#[tauri::command]
pub async fn restore_backup(
    state: State<'_, AppState>,
    kind: String,
    id: String,
    file: String,
) -> Result<(), String> {
    let (root, _) = backup_target(&state, &kind, &id);
    let zip = backups_dir(&state, &kind, &id).join(&file);
    if !zip.exists() {
        return Err("That backup no longer exists".into());
    }
    tokio::task::spawn_blocking(move || launcher_core::backup::restore(&zip, &root))
        .await
        .map_err(err)?
        .map_err(err)
}

#[tauri::command]
pub async fn delete_backup(
    state: State<'_, AppState>,
    kind: String,
    id: String,
    file: String,
) -> Result<(), String> {
    let zip = backups_dir(&state, &kind, &id).join(&file);
    std::fs::remove_file(&zip).map_err(err)
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

    // Always start the mods folder clean. Re-running a pack install (a repair,
    // or an upgrade) must never leave stale mods from a different Minecraft
    // version behind — that mix is what crashes mods like Controlify.
    let _ = std::fs::remove_dir_all(inst.join("mods"));

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
        auto_start: false,
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
    // Optional `host:port` to join directly via Quick Play (1.20+).
    server: Option<String>,
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
        let mut options = LaunchOptions {
            max_memory_mb: cfg.max_ram_mb,
            game_directory: Some(inst_dir.clone()),
            launcher_name: "Aurora Launcher".to_string(),
            ..Default::default()
        };
        if let Some(addr) = server.as_deref().filter(|s| !s.is_empty()) {
            // Quick Play straight into the server, skipping the menus.
            options.extra_game_args = vec!["--quickPlayMultiplayer".into(), addr.to_string()];
        }
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
            crate::discord::set_playing(&format!("Playing {}", cfg.name), "Minecraft · via Aurora");
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
