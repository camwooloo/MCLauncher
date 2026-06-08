//! Modrinth content, scoped to a specific **instance or server**.
//!
//! Every content op takes a `target_kind` ("instance" | "server") + `target_id`.
//! The backend resolves that target's directory, Minecraft version, and loader,
//! installs into that directory, and tracks installed content in
//! `<target_dir>/aurora-content.json`. This keeps mods/shaders/resource packs
//! isolated per instance/server and lets the update checker compare against the
//! right version.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::State;

use launcher_core::modrinth;

use crate::state::AppState;

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledItem {
    pub project_id: String,
    pub project_type: String,
    pub title: String,
    pub version_id: String,
    pub version_number: String,
    pub file_name: String,
    pub game_version: String,
    pub loader: Option<String>,
}

#[derive(Default, Serialize, Deserialize)]
struct ContentManifest {
    #[serde(default)]
    items: Vec<InstalledItem>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateResult {
    pub item: InstalledItem,
    /// "update" | "current" | "incompatible"
    pub status: String,
    pub new_version_number: Option<String>,
}

/// Resolve (base dir, version, loader) for a content target.
async fn resolve_target(
    state: &AppState,
    target_kind: &str,
    target_id: &str,
) -> Result<(PathBuf, String, Option<String>), String> {
    let resolved = match target_kind {
        "instance" => crate::instances::instance_content_target(state, target_id).await,
        "server" => crate::commands::server_content_target(state, target_id).await,
        _ => None,
    };
    resolved.ok_or_else(|| "Target not found".to_string())
}

fn content_subdir(base: &Path, project_type: &str) -> Option<PathBuf> {
    match project_type {
        "mod" => Some(base.join("mods")),
        "shader" => Some(base.join("shaderpacks")),
        "resourcepack" => Some(base.join("resourcepacks")),
        "plugin" => Some(base.join("plugins")),
        _ => None,
    }
}

fn manifest_path(base: &Path) -> PathBuf {
    base.join("aurora-content.json")
}

async fn load_manifest(base: &Path) -> ContentManifest {
    match tokio::fs::read(manifest_path(base)).await {
        Ok(b) => serde_json::from_slice(&b).unwrap_or_default(),
        Err(_) => ContentManifest::default(),
    }
}

async fn store_manifest(base: &Path, m: &ContentManifest) -> Result<(), String> {
    let path = manifest_path(base);
    if let Some(p) = path.parent() {
        let _ = tokio::fs::create_dir_all(p).await;
    }
    let bytes = serde_json::to_vec_pretty(m).map_err(err)?;
    tokio::fs::write(&path, bytes).await.map_err(|e| e.to_string())
}

fn upsert_item(m: &mut ContentManifest, item: InstalledItem) {
    match m.items.iter_mut().find(|i| i.project_id == item.project_id) {
        Some(existing) => *existing = item,
        None => m.items.push(item),
    }
}

/// Search is global (filtered by version/loader the UI passes from the target).
#[tauri::command]
pub async fn modrinth_search(
    query: String,
    kind: String,
    game_version: Option<String>,
    loader: Option<String>,
) -> Result<Vec<modrinth::SearchHit>, String> {
    // Plugins are Modrinth "mod" projects filtered by a server loader (paper/…).
    let project_type = if kind == "plugin" { "mod" } else { kind.as_str() };
    let loader = if kind == "mod" || kind == "plugin" { loader.as_deref() } else { None };
    modrinth::search(&query, project_type, game_version.as_deref(), loader, 30)
        .await
        .map_err(err)
}

/// Loaders that a Modrinth version must be filtered by for this project type.
fn loader_filter<'a>(project_type: &str, loader: &'a Option<String>) -> Option<&'a str> {
    if project_type == "mod" || project_type == "plugin" {
        loader.as_deref()
    } else {
        None
    }
}

#[tauri::command]
pub async fn content_install(
    state: State<'_, AppState>,
    target_kind: String,
    target_id: String,
    project_id: String,
    project_type: String,
    title: String,
) -> Result<InstalledItem, String> {
    let (base, version, loader) = resolve_target(&state, &target_kind, &target_id).await?;
    let reporter = launcher_core::progress::noop();
    let lo = loader_filter(&project_type, &loader);

    // Modpacks expand into many files + overrides under the target dir.
    if project_type == "modpack" {
        let v = modrinth::latest_version(&project_id, &version, None)
            .await
            .map_err(err)?
            .ok_or_else(|| format!("No build of \"{title}\" for Minecraft {version}"))?;
        modrinth::install_modpack(&base, &v, &reporter).await.map_err(err)?;
        let item = InstalledItem {
            project_id,
            project_type,
            title,
            version_id: v.id.clone(),
            version_number: v.version_number.clone(),
            file_name: v.primary_file().map(|f| f.filename.clone()).unwrap_or_default(),
            game_version: version,
            loader: None,
        };
        let mut manifest = load_manifest(&base).await;
        upsert_item(&mut manifest, item.clone());
        store_manifest(&base, &manifest).await?;
        return Ok(item);
    }

    let dir = content_subdir(&base, &project_type).ok_or_else(|| "Unsupported content type".to_string())?;
    let v = modrinth::latest_version(&project_id, &version, lo)
        .await
        .map_err(err)?
        .ok_or_else(|| format!("No build of \"{title}\" for Minecraft {version}"))?;
    let filename = modrinth::install_file(&dir, &v, &reporter).await.map_err(err)?;

    let item = InstalledItem {
        project_id: project_id.clone(),
        project_type: project_type.clone(),
        title,
        version_id: v.id.clone(),
        version_number: v.version_number.clone(),
        file_name: filename,
        game_version: version.clone(),
        loader: lo.map(|s| s.to_string()),
    };
    let mut manifest = load_manifest(&base).await;
    upsert_item(&mut manifest, item.clone());

    // Required dependencies (one level) for mods.
    if project_type == "mod" {
        for dep in &v.dependencies {
            if dep.dependency_type != "required" {
                continue;
            }
            let Some(pid) = &dep.project_id else { continue };
            if manifest.items.iter().any(|i| &i.project_id == pid) {
                continue;
            }
            if let Ok(Some(dv)) = modrinth::latest_version(pid, &version, lo).await {
                if let Ok(fname) = modrinth::install_file(&dir, &dv, &reporter).await {
                    upsert_item(
                        &mut manifest,
                        InstalledItem {
                            project_id: pid.clone(),
                            project_type: "mod".into(),
                            title: dv.name.clone(),
                            version_id: dv.id.clone(),
                            version_number: dv.version_number.clone(),
                            file_name: fname,
                            game_version: version.clone(),
                            loader: lo.map(|s| s.to_string()),
                        },
                    );
                }
            }
        }
    }

    store_manifest(&base, &manifest).await?;
    Ok(item)
}

#[tauri::command]
pub async fn list_installed(
    state: State<'_, AppState>,
    target_kind: String,
    target_id: String,
) -> Result<Vec<InstalledItem>, String> {
    let (base, _, _) = resolve_target(&state, &target_kind, &target_id).await?;
    Ok(load_manifest(&base).await.items)
}

#[tauri::command]
pub async fn content_remove(
    state: State<'_, AppState>,
    target_kind: String,
    target_id: String,
    project_id: String,
) -> Result<(), String> {
    let (base, _, _) = resolve_target(&state, &target_kind, &target_id).await?;
    let mut manifest = load_manifest(&base).await;
    if let Some(pos) = manifest.items.iter().position(|i| i.project_id == project_id) {
        let item = manifest.items.remove(pos);
        if let Some(dir) = content_subdir(&base, &item.project_type) {
            let _ = tokio::fs::remove_file(dir.join(&item.file_name)).await;
        }
        store_manifest(&base, &manifest).await?;
    }
    Ok(())
}

#[tauri::command]
pub async fn check_updates(
    state: State<'_, AppState>,
    target_kind: String,
    target_id: String,
    target_version: String,
) -> Result<Vec<UpdateResult>, String> {
    let (base, _, _) = resolve_target(&state, &target_kind, &target_id).await?;
    let manifest = load_manifest(&base).await;
    let mut out = Vec::new();
    for item in manifest.items {
        let lo = loader_filter(&item.project_type, &item.loader);
        match modrinth::latest_version(&item.project_id, &target_version, lo).await {
            Ok(Some(v)) => {
                let updatable = v.id != item.version_id;
                out.push(UpdateResult {
                    new_version_number: if updatable { Some(v.version_number) } else { None },
                    status: if updatable { "update" } else { "current" }.into(),
                    item,
                });
            }
            _ => out.push(UpdateResult {
                item,
                status: "incompatible".into(),
                new_version_number: None,
            }),
        }
    }
    Ok(out)
}

#[tauri::command]
pub async fn apply_update(
    state: State<'_, AppState>,
    target_kind: String,
    target_id: String,
    project_id: String,
    target_version: String,
) -> Result<InstalledItem, String> {
    let (base, _, _) = resolve_target(&state, &target_kind, &target_id).await?;
    let mut manifest = load_manifest(&base).await;
    let idx = manifest
        .items
        .iter()
        .position(|i| i.project_id == project_id)
        .ok_or_else(|| "Not installed".to_string())?;
    let item = manifest.items[idx].clone();
    let lo = loader_filter(&item.project_type, &item.loader);
    let reporter = launcher_core::progress::noop();

    let v = modrinth::latest_version(&project_id, &target_version, lo)
        .await
        .map_err(err)?
        .ok_or_else(|| format!("No build of \"{}\" for Minecraft {target_version}", item.title))?;

    let fname = if item.project_type == "modpack" {
        modrinth::install_modpack(&base, &v, &reporter).await.map_err(err)?;
        v.primary_file().map(|f| f.filename.clone()).unwrap_or_default()
    } else {
        let dir = content_subdir(&base, &item.project_type).ok_or_else(|| "Unsupported type".to_string())?;
        let _ = tokio::fs::remove_file(dir.join(&item.file_name)).await;
        modrinth::install_file(&dir, &v, &reporter).await.map_err(err)?
    };

    let entry = &mut manifest.items[idx];
    entry.version_id = v.id;
    entry.version_number = v.version_number;
    entry.file_name = fname;
    entry.game_version = target_version;
    let updated = entry.clone();
    store_manifest(&base, &manifest).await?;
    Ok(updated)
}

/// Upload and apply a skin (PNG bytes) for the active Microsoft account.
#[tauri::command]
pub async fn set_skin(
    state: State<'_, AppState>,
    variant: String,
    png: Vec<u8>,
) -> Result<(), String> {
    use launcher_core::account::AccountStore;

    let store = AccountStore::load(&state.paths.accounts_file()).await.map_err(err)?;
    let acct = store.active().ok_or_else(|| "No active account".to_string())?;
    if acct.account.user_type != "msa" || acct.account.access_token == "0" {
        return Err("Sign in with a Microsoft account to change your skin".into());
    }

    let model = if variant == "slim" { "slim" } else { "classic" };
    let form = reqwest::multipart::Form::new().text("variant", model).part(
        "file",
        reqwest::multipart::Part::bytes(png)
            .file_name("skin.png")
            .mime_str("image/png")
            .map_err(err)?,
    );

    let resp = launcher_core::http::client()
        .post("https://api.minecraftservices.com/minecraft/profile/skins")
        .bearer_auth(&acct.account.access_token)
        .multipart(form)
        .send()
        .await
        .map_err(err)?;
    if !resp.status().is_success() {
        return Err(format!("Skin upload failed ({})", resp.status()));
    }
    Ok(())
}
