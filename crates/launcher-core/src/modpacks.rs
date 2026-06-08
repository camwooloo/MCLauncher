//! Modpack platforms beyond Modrinth: **FTB** (api.modpacks.ch, keyless),
//! **CurseForge** (api.curseforge.com, needs an API key), and **Technic**
//! (api.technicpack.net, zip-based, best-effort).
//!
//! Each `search` returns [`PackHit`]s; each `install` downloads the pack into an
//! instance directory and returns the Minecraft version + loader to configure.

use std::io::Read;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::download::{self, Download, DEFAULT_CONCURRENCY};
use crate::progress::SharedReporter;
use crate::{Error, Result};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackHit {
    /// Platform-specific id (stringified) used for install.
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub summary: String,
    pub icon: Option<String>,
    #[serde(default)]
    pub downloads: u64,
}

/// What to configure on the instance after installing a pack.
pub struct PackInstall {
    pub version: String,
    pub loader: String,
}

/// Normalise a platform loader name to our loader ids.
fn map_loader(name: &str) -> String {
    match name.to_lowercase().as_str() {
        "fabric" => "fabric",
        "quilt" => "quilt",
        "forge" => "forge",
        "neoforge" => "neoforge",
        _ => "vanilla",
    }
    .to_string()
}

/// Join a `/`-separated relative path onto a base, normalising + dropping `..`.
fn rel_join(base: &Path, rel: &str) -> std::path::PathBuf {
    let mut out = base.to_path_buf();
    for part in rel.split(['/', '\\']).filter(|p| !p.is_empty() && *p != "..") {
        out.push(part);
    }
    out
}

// =========================================================================
// FTB — api.modpacks.ch (keyless)
// =========================================================================
pub mod ftb {
    use super::*;

    const API: &str = "https://api.modpacks.ch/public";

    #[derive(Deserialize)]
    struct SearchResp {
        #[serde(default)]
        packs: Vec<i64>,
    }
    #[derive(Deserialize)]
    struct Art {
        url: String,
        #[serde(default, rename = "type")]
        kind: String,
    }
    #[derive(Deserialize)]
    struct VersionRef {
        id: i64,
        #[serde(default, rename = "type")]
        kind: String,
    }
    #[derive(Deserialize)]
    struct Pack {
        #[serde(default)]
        name: String,
        #[serde(default)]
        synopsis: String,
        #[serde(default)]
        art: Vec<Art>,
        #[serde(default)]
        versions: Vec<VersionRef>,
        #[serde(default)]
        installs: u64,
    }
    #[derive(Deserialize)]
    struct Target {
        #[serde(rename = "type")]
        kind: String,
        #[serde(default)]
        name: String,
        #[serde(default)]
        version: String,
    }
    #[derive(Deserialize)]
    struct VFile {
        #[serde(default)]
        path: String,
        name: String,
        #[serde(default)]
        url: String,
        #[serde(default)]
        sha1: String,
        #[serde(default)]
        serveronly: bool,
    }
    #[derive(Deserialize)]
    struct VersionDetail {
        #[serde(default)]
        files: Vec<VFile>,
        #[serde(default)]
        targets: Vec<Target>,
    }

    async fn pack(id: i64) -> Result<Pack> {
        Ok(crate::http::client()
            .get(format!("{API}/modpack/{id}"))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?)
    }

    pub async fn search(term: &str) -> Result<Vec<PackHit>> {
        let ids = if term.trim().is_empty() {
            crate::http::client()
                .get(format!("{API}/modpack/popular/installs/20"))
                .send()
                .await?
                .error_for_status()?
                .json::<SearchResp>()
                .await?
                .packs
        } else {
            crate::http::client()
                .get(format!("{API}/modpack/search/20"))
                .query(&[("term", term)])
                .send()
                .await?
                .error_for_status()?
                .json::<SearchResp>()
                .await?
                .packs
        };

        let mut hits = Vec::new();
        for id in ids.into_iter().take(20) {
            if let Ok(p) = pack(id).await {
                let icon = p
                    .art
                    .iter()
                    .find(|a| a.kind == "square")
                    .or_else(|| p.art.first())
                    .map(|a| a.url.clone());
                hits.push(PackHit {
                    id: id.to_string(),
                    name: p.name,
                    summary: p.synopsis,
                    icon,
                    downloads: p.installs,
                });
            }
        }
        Ok(hits)
    }

    pub async fn install(game_dir: &Path, pack_id: &str, reporter: &SharedReporter) -> Result<PackInstall> {
        let id: i64 = pack_id.parse().map_err(|_| Error::other("invalid FTB pack id"))?;
        let p = pack(id).await?;
        let ver = p
            .versions
            .iter()
            .filter(|v| v.kind == "release")
            .last()
            .or_else(|| p.versions.last())
            .ok_or_else(|| Error::other("FTB pack has no versions"))?;

        let detail: VersionDetail = crate::http::client()
            .get(format!("{API}/modpack/{id}/{}", ver.id))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        let version = detail
            .targets
            .iter()
            .find(|t| t.kind == "minecraft")
            .map(|t| t.version.clone())
            .ok_or_else(|| Error::other("FTB pack has no Minecraft version"))?;
        let loader = detail
            .targets
            .iter()
            .find(|t| t.kind == "modloader")
            .map(|t| map_loader(&t.name))
            .unwrap_or_else(|| "vanilla".to_string());

        crate::util::ensure_dir(game_dir).await?;
        let dls: Vec<Download> = detail
            .files
            .iter()
            .filter(|f| !f.serveronly && !f.url.is_empty())
            .map(|f| {
                let rel = format!("{}/{}", f.path.trim_matches(['.', '/']), f.name);
                let mut d = Download::new(f.url.clone(), rel_join(game_dir, &rel));
                if !f.sha1.is_empty() {
                    d = d.sha1(f.sha1.clone());
                }
                d
            })
            .collect();
        download::download_all(dls, DEFAULT_CONCURRENCY, reporter.clone()).await?;
        Ok(PackInstall { version, loader })
    }
}

// =========================================================================
// CurseForge — api.curseforge.com (needs API key)
// =========================================================================
pub mod curseforge {
    use super::*;

    const API: &str = "https://api.curseforge.com/v1";

    #[derive(Deserialize)]
    struct Logo {
        #[serde(default)]
        url: String,
    }
    #[derive(Deserialize)]
    struct CfMod {
        id: i64,
        name: String,
        #[serde(default)]
        summary: String,
        #[serde(default)]
        logo: Option<Logo>,
        #[serde(default, rename = "downloadCount")]
        downloads: f64,
    }
    #[derive(Deserialize)]
    struct SearchResp {
        data: Vec<CfMod>,
    }
    #[derive(Deserialize)]
    struct FileInfo {
        id: i64,
        #[serde(default, rename = "downloadUrl")]
        download_url: Option<String>,
    }
    #[derive(Deserialize)]
    struct ModResp {
        data: ModDetail,
    }
    #[derive(Deserialize)]
    struct ModDetail {
        #[serde(default, rename = "latestFiles")]
        latest_files: Vec<FileInfo>,
    }

    // .mrpack-style CurseForge manifest.
    #[derive(Deserialize)]
    struct Manifest {
        minecraft: MfMinecraft,
        #[serde(default)]
        files: Vec<MfFile>,
        #[serde(default)]
        overrides: String,
    }
    #[derive(Deserialize)]
    struct MfMinecraft {
        version: String,
        #[serde(default, rename = "modLoaders")]
        mod_loaders: Vec<MfLoader>,
    }
    #[derive(Deserialize)]
    struct MfLoader {
        id: String,
        #[serde(default)]
        primary: bool,
    }
    #[derive(Deserialize)]
    struct MfFile {
        #[serde(rename = "projectID")]
        project_id: i64,
        #[serde(rename = "fileID")]
        file_id: i64,
    }

    fn get(url: String, key: &str) -> reqwest::RequestBuilder {
        crate::http::client()
            .get(url)
            .header("x-api-key", key)
            .header("Accept", "application/json")
    }

    pub async fn search(query: &str, key: &str) -> Result<Vec<PackHit>> {
        let resp = get(format!("{API}/mods/search"), key)
            .query(&[
                ("gameId", "432"),
                ("classId", "4471"),
                ("searchFilter", query),
                ("sortField", "2"),
                ("sortOrder", "desc"),
                ("pageSize", "30"),
            ])
            .send()
            .await?
            .error_for_status()?
            .json::<SearchResp>()
            .await?;
        Ok(resp
            .data
            .into_iter()
            .map(|m| PackHit {
                id: m.id.to_string(),
                name: m.name,
                summary: m.summary,
                icon: m.logo.map(|l| l.url).filter(|u| !u.is_empty()),
                downloads: m.downloads as u64,
            })
            .collect())
    }

    #[derive(Deserialize)]
    struct ModFullResp {
        data: CfMod,
    }

    /// Fetch specific mods by id (used for the curated list, since `/mods/search`
    /// is forbidden on personal API keys). Failed/invalid ids are skipped.
    pub async fn by_ids(ids: &[i64], key: &str) -> Result<Vec<PackHit>> {
        let mut out = Vec::new();
        for &id in ids {
            let r = match get(format!("{API}/mods/{id}"), key).send().await {
                Ok(r) => r,
                Err(_) => continue,
            };
            let r = match r.error_for_status() {
                Ok(r) => r,
                Err(_) => continue,
            };
            if let Ok(m) = r.json::<ModFullResp>().await {
                let m = m.data;
                out.push(PackHit {
                    id: m.id.to_string(),
                    name: m.name,
                    summary: m.summary,
                    icon: m.logo.map(|l| l.url).filter(|u| !u.is_empty()),
                    downloads: m.downloads as u64,
                });
            }
        }
        Ok(out)
    }

    async fn file_url(project_id: i64, file_id: i64, key: &str) -> Option<String> {
        if let Ok(v) = get(format!("{API}/mods/{project_id}/files/{file_id}"), key)
            .send()
            .await
            .ok()?
            .json::<serde_json::Value>()
            .await
        {
            if let Some(u) = v["data"]["downloadUrl"].as_str() {
                if !u.is_empty() {
                    return Some(u.to_string());
                }
            }
        }
        // Fallback: explicit download-url endpoint.
        let v = get(format!("{API}/mods/{project_id}/files/{file_id}/download-url"), key)
            .send()
            .await
            .ok()?
            .json::<serde_json::Value>()
            .await
            .ok()?;
        v["data"].as_str().filter(|s| !s.is_empty()).map(|s| s.to_string())
    }

    pub async fn install(
        game_dir: &Path,
        mod_id: &str,
        key: &str,
        reporter: &SharedReporter,
    ) -> Result<PackInstall> {
        if key.trim().is_empty() {
            return Err(Error::other("Add your CurseForge API key in Settings first"));
        }
        let id: i64 = mod_id.parse().map_err(|_| Error::other("invalid CurseForge id"))?;
        let m: ModResp = get(format!("{API}/mods/{id}"), key)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        let file = m
            .data
            .latest_files
            .into_iter()
            .next()
            .ok_or_else(|| Error::other("CurseForge pack has no files"))?;
        let zip_url = match file.download_url {
            Some(u) if !u.is_empty() => u,
            _ => file_url(id, file.id, key)
                .await
                .ok_or_else(|| Error::other("CurseForge file isn't downloadable via the API"))?,
        };

        crate::util::ensure_dir(game_dir).await?;
        let zip_path = game_dir.join(".aurora-cf.zip");
        download::download_all(vec![Download::new(zip_url, zip_path.clone())], 2, reporter.clone()).await?;

        // Parse manifest + extract overrides on the blocking pool.
        let game2 = game_dir.to_path_buf();
        let zip2 = zip_path.clone();
        let (version, loader, files): (String, String, Vec<(i64, i64)>) =
            tokio::task::spawn_blocking(move || -> Result<_> {
                let f = std::fs::File::open(&zip2).map_err(|e| Error::io(&zip2, e))?;
                let mut z = zip::ZipArchive::new(f)?;
                let manifest: Manifest = {
                    let mut e = z
                        .by_name("manifest.json")
                        .map_err(|_| Error::other("CurseForge zip has no manifest.json"))?;
                    let mut s = String::new();
                    e.read_to_string(&mut s).map_err(Error::IoBare)?;
                    serde_json::from_str(&s)?
                };
                let loader = manifest
                    .minecraft
                    .mod_loaders
                    .iter()
                    .find(|l| l.primary)
                    .or_else(|| manifest.minecraft.mod_loaders.first())
                    .map(|l| map_loader(l.id.split('-').next().unwrap_or("")))
                    .unwrap_or_else(|| "vanilla".to_string());
                let ov = if manifest.overrides.is_empty() {
                    "overrides".to_string()
                } else {
                    manifest.overrides
                };
                let prefix = format!("{ov}/");
                for i in 0..z.len() {
                    let mut e = z.by_index(i)?;
                    if e.is_dir() {
                        continue;
                    }
                    let Some(name) = e.enclosed_name() else { continue };
                    let name = name.to_string_lossy().replace('\\', "/");
                    if let Some(rel) = name.strip_prefix(&prefix) {
                        let out = rel_join(&game2, rel);
                        if let Some(p) = out.parent() {
                            std::fs::create_dir_all(p).ok();
                        }
                        let mut of = std::fs::File::create(&out).map_err(|e| Error::io(&out, e))?;
                        std::io::copy(&mut e, &mut of).map_err(|e| Error::io(&out, e))?;
                    }
                }
                let files = manifest.files.iter().map(|f| (f.project_id, f.file_id)).collect();
                Ok((manifest.minecraft.version, loader, files))
            })
            .await
            .map_err(|e| Error::other(format!("CurseForge extract task panicked: {e}")))??;

        // Resolve + download each mod file into mods/.
        let mods_dir = game_dir.join("mods");
        crate::util::ensure_dir(&mods_dir).await?;
        let mut dls = Vec::new();
        for (pid, fid) in files {
            if let Some(url) = file_url(pid, fid, key).await {
                let fname = url.rsplit('/').next().unwrap_or("mod.jar").to_string();
                dls.push(Download::new(url, mods_dir.join(fname)));
            }
        }
        download::download_all(dls, DEFAULT_CONCURRENCY, reporter.clone()).await?;
        let _ = tokio::fs::remove_file(&zip_path).await;

        Ok(PackInstall { version, loader })
    }
}

// =========================================================================
// Technic — api.technicpack.net (zip-based, best-effort)
// =========================================================================
pub mod technic {
    use super::*;

    const API: &str = "https://api.technicpack.net";

    pub async fn search(query: &str) -> Result<Vec<PackHit>> {
        let v = crate::http::client()
            .get(format!("{API}/search"))
            .query(&[("q", query), ("build", "0.0.0")])
            .send()
            .await?
            .error_for_status()?
            .json::<serde_json::Value>()
            .await?;
        let mut hits = Vec::new();
        if let Some(arr) = v["results"].as_array() {
            for r in arr {
                let slug = r["slug"].as_str().unwrap_or("").to_string();
                if slug.is_empty() {
                    continue;
                }
                hits.push(PackHit {
                    id: slug.clone(),
                    name: r["displayName"]
                        .as_str()
                        .or_else(|| r["name"].as_str())
                        .unwrap_or(&slug)
                        .to_string(),
                    summary: r["description"].as_str().unwrap_or("").to_string(),
                    icon: r["iconUrl"].as_str().map(|s| s.to_string()),
                    downloads: r["downloads"].as_u64().unwrap_or(0),
                });
            }
        }
        Ok(hits)
    }

    pub async fn install(game_dir: &Path, slug: &str, reporter: &SharedReporter) -> Result<PackInstall> {
        let m = crate::http::client()
            .get(format!("{API}/modpack/{slug}"))
            .query(&[("build", "0.0.0")])
            .send()
            .await?
            .error_for_status()?
            .json::<serde_json::Value>()
            .await?;
        let version = m["minecraft"].as_str().unwrap_or("").to_string();
        let url = m["url"].as_str().unwrap_or("").to_string();
        if url.is_empty() {
            return Err(Error::other(
                "This Technic pack uses Solder (incremental downloads), which isn't supported yet",
            ));
        }
        if version.is_empty() {
            return Err(Error::other("Technic pack didn't report a Minecraft version"));
        }

        crate::util::ensure_dir(game_dir).await?;
        let zip_path = game_dir.join(".aurora-technic.zip");
        download::download_all(vec![Download::new(url, zip_path.clone())], 2, reporter.clone()).await?;

        let game2 = game_dir.to_path_buf();
        let zip2 = zip_path.clone();
        tokio::task::spawn_blocking(move || -> Result<()> {
            let f = std::fs::File::open(&zip2).map_err(|e| Error::io(&zip2, e))?;
            let mut z = zip::ZipArchive::new(f)?;
            for i in 0..z.len() {
                let mut e = z.by_index(i)?;
                if e.is_dir() {
                    continue;
                }
                let Some(name) = e.enclosed_name() else { continue };
                let out = game2.join(&name);
                if let Some(p) = out.parent() {
                    std::fs::create_dir_all(p).ok();
                }
                let mut of = std::fs::File::create(&out).map_err(|e| Error::io(&out, e))?;
                std::io::copy(&mut e, &mut of).map_err(|e| Error::io(&out, e))?;
            }
            Ok(())
        })
        .await
        .map_err(|e| Error::other(format!("Technic extract task panicked: {e}")))??;
        let _ = tokio::fs::remove_file(&zip_path).await;

        // Best-effort: Technic packs bundle their own loader jar; we launch via a
        // standard Forge install for this MC version, which may not match exactly.
        Ok(PackInstall { version, loader: "forge".to_string() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loader_mapping() {
        assert_eq!(map_loader("Forge"), "forge");
        assert_eq!(map_loader("fabric"), "fabric");
        assert_eq!(map_loader("NeoForge"), "neoforge");
        assert_eq!(map_loader("whatever"), "vanilla");
    }
}
