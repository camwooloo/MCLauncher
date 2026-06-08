//! Modrinth API client — content discovery & install (mods, shaders, resource
//! packs, modpacks).
//!
//! Modrinth's v2 API is free and keyless. We expose search, "latest compatible
//! version" resolution (used for both install and update checks), and a helper
//! that downloads a version's primary file into a target directory.

use serde::{Deserialize, Serialize};

use crate::download::{self, Download};
use crate::progress::SharedReporter;
use crate::Result;

const API: &str = "https://api.modrinth.com/v2";

/// One search result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    pub project_id: String,
    pub slug: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub downloads: u64,
    #[serde(default)]
    pub icon_url: Option<String>,
    pub project_type: String,
}

#[derive(Deserialize)]
struct SearchResponse {
    hits: Vec<SearchHit>,
}

/// A published version of a project.
#[derive(Debug, Clone, Deserialize)]
pub struct Version {
    pub id: String,
    #[serde(default)]
    pub name: String,
    pub version_number: String,
    #[serde(default)]
    pub game_versions: Vec<String>,
    #[serde(default)]
    pub loaders: Vec<String>,
    pub files: Vec<VersionFile>,
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
    #[serde(default)]
    pub date_published: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VersionFile {
    pub url: String,
    pub filename: String,
    #[serde(default)]
    pub primary: bool,
    #[serde(default)]
    pub hashes: Hashes,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Hashes {
    pub sha1: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Dependency {
    pub project_id: Option<String>,
    pub dependency_type: String,
}

impl Version {
    /// The file to download (the primary, or the first).
    pub fn primary_file(&self) -> Option<&VersionFile> {
        self.files.iter().find(|f| f.primary).or_else(|| self.files.first())
    }
}

/// Search Modrinth for `project_type` ("mod" | "shader" | "resourcepack" |
/// "modpack"), optionally filtered to a game version and loader.
pub async fn search(
    query: &str,
    project_type: &str,
    game_version: Option<&str>,
    loader: Option<&str>,
    limit: u32,
) -> Result<Vec<SearchHit>> {
    let mut facets = vec![format!("[\"project_type:{project_type}\"]")];
    if let Some(v) = game_version {
        facets.push(format!("[\"versions:{v}\"]"));
    }
    if let Some(l) = loader {
        facets.push(format!("[\"categories:{l}\"]"));
    }
    let facets_json = format!("[{}]", facets.join(","));

    let resp = crate::http::client()
        .get(format!("{API}/search"))
        .query(&[
            ("limit", limit.to_string()),
            ("index", "relevance".to_string()),
            ("query", query.to_string()),
            ("facets", facets_json),
        ])
        .send()
        .await?
        .error_for_status()?
        .json::<SearchResponse>()
        .await?;
    Ok(resp.hits)
}

/// Most-downloaded projects of a type (e.g. popular modpacks), newest builds
/// aside — sorted by total downloads.
pub async fn popular(project_type: &str, limit: u32) -> Result<Vec<SearchHit>> {
    let resp = crate::http::client()
        .get(format!("{API}/search"))
        .query(&[
            ("limit", limit.to_string()),
            ("index", "downloads".to_string()),
            ("facets", format!("[[\"project_type:{project_type}\"]]")),
        ])
        .send()
        .await?
        .error_for_status()?
        .json::<SearchResponse>()
        .await?;
    Ok(resp.hits)
}

/// The newest published version of a project, regardless of game version —
/// used to derive a modpack's Minecraft version + loader when creating from it.
pub async fn latest_any(project: &str) -> Result<Option<Version>> {
    let mut versions = crate::http::client()
        .get(format!("{API}/project/{project}/version"))
        .send()
        .await?
        .error_for_status()?
        .json::<Vec<Version>>()
        .await?;
    versions.sort_by(|a, b| b.date_published.cmp(&a.date_published));
    Ok(versions.into_iter().next())
}

/// The latest version of a project compatible with `game_version` (and `loader`
/// if given). Returns `None` when nothing matches — the signal a piece of
/// content has no build for that Minecraft version.
pub async fn latest_version(
    project: &str,
    game_version: &str,
    loader: Option<&str>,
) -> Result<Option<Version>> {
    let mut query = vec![("game_versions", format!("[\"{game_version}\"]"))];
    if let Some(l) = loader {
        query.push(("loaders", format!("[\"{l}\"]")));
    }
    let mut versions = crate::http::client()
        .get(format!("{API}/project/{project}/version"))
        .query(&query)
        .send()
        .await?
        .error_for_status()?
        .json::<Vec<Version>>()
        .await?;

    // Newest first.
    versions.sort_by(|a, b| b.date_published.cmp(&a.date_published));
    Ok(versions.into_iter().next())
}

/// Download a version's primary file into `dir`; returns the filename written.
pub async fn install_file(
    dir: &std::path::Path,
    version: &Version,
    reporter: &SharedReporter,
) -> Result<String> {
    let file = version
        .primary_file()
        .ok_or_else(|| crate::Error::other("Modrinth version has no downloadable file"))?;
    crate::util::ensure_dir(dir).await?;
    let dest = dir.join(&file.filename);
    let mut dl = Download::new(file.url.clone(), dest);
    if let Some(sha1) = &file.hashes.sha1 {
        dl = dl.sha1(sha1.clone());
    }
    download::download_all(vec![dl], 4, reporter.clone()).await?;
    Ok(file.filename.clone())
}

// --- Modpacks (.mrpack) --------------------------------------------------

#[derive(Deserialize)]
struct MrpackIndex {
    #[serde(default)]
    files: Vec<MrpackFile>,
}

#[derive(Deserialize)]
struct MrpackFile {
    path: String,
    #[serde(default)]
    downloads: Vec<String>,
    #[serde(default)]
    hashes: Hashes,
    #[serde(default)]
    env: Option<MrpackEnv>,
}

#[derive(Deserialize)]
struct MrpackEnv {
    #[serde(default)]
    client: Option<String>,
}

fn rel_join(base: &std::path::Path, rel: &str) -> std::path::PathBuf {
    let mut out = base.to_path_buf();
    for part in rel.split('/').filter(|p| !p.is_empty() && *p != "..") {
        out.push(part);
    }
    out
}

/// Install a Modrinth modpack: download the `.mrpack`, fetch every listed file
/// to its path under `game_dir`, and copy the pack's `overrides/` into place.
pub async fn install_modpack(
    game_dir: &std::path::Path,
    version: &Version,
    reporter: &SharedReporter,
) -> Result<()> {
    use std::io::Read;

    let file = version
        .primary_file()
        .ok_or_else(|| crate::Error::other("modpack has no downloadable file"))?;
    crate::util::ensure_dir(game_dir).await?;
    let mrpack = game_dir.join(".aurora-modpack.mrpack");
    let mut dl = Download::new(file.url.clone(), mrpack.clone());
    if let Some(sha1) = &file.hashes.sha1 {
        dl = dl.sha1(sha1.clone());
    }
    download::download_all(vec![dl], 2, reporter.clone()).await?;

    // Parse the index and extract overrides on the blocking pool (zip is sync).
    let mrpack2 = mrpack.clone();
    let game2 = game_dir.to_path_buf();
    let listed: Vec<(String, Vec<String>, Option<String>)> = tokio::task::spawn_blocking(
        move || -> Result<Vec<(String, Vec<String>, Option<String>)>> {
            let f = std::fs::File::open(&mrpack2).map_err(|e| crate::Error::io(&mrpack2, e))?;
            let mut zip = zip::ZipArchive::new(f)?;

            let index: MrpackIndex = {
                let mut entry = zip
                    .by_name("modrinth.index.json")
                    .map_err(|_| crate::Error::other("modrinth.index.json missing from .mrpack"))?;
                let mut s = String::new();
                entry.read_to_string(&mut s).map_err(crate::Error::IoBare)?;
                serde_json::from_str(&s)?
            };

            for i in 0..zip.len() {
                let mut e = zip.by_index(i)?;
                if e.is_dir() {
                    continue;
                }
                let Some(name) = e.enclosed_name() else { continue };
                let name = name.to_string_lossy().replace('\\', "/");
                let rel = name
                    .strip_prefix("overrides/")
                    .or_else(|| name.strip_prefix("client-overrides/"));
                if let Some(rel) = rel {
                    if rel.is_empty() {
                        continue;
                    }
                    let out = rel_join(&game2, rel);
                    if let Some(p) = out.parent() {
                        std::fs::create_dir_all(p).ok();
                    }
                    let mut of = std::fs::File::create(&out).map_err(|e| crate::Error::io(&out, e))?;
                    std::io::copy(&mut e, &mut of).map_err(|e| crate::Error::io(&out, e))?;
                }
            }

            let mut out = Vec::new();
            for mf in index.files {
                let client_ok = mf
                    .env
                    .as_ref()
                    .and_then(|e| e.client.as_deref())
                    .map(|c| c != "unsupported")
                    .unwrap_or(true);
                if client_ok && !mf.downloads.is_empty() {
                    out.push((mf.path, mf.downloads, mf.hashes.sha1));
                }
            }
            Ok(out)
        },
    )
    .await
    .map_err(|e| crate::Error::other(format!("modpack extraction task panicked: {e}")))??;

    let downloads: Vec<Download> = listed
        .into_iter()
        .map(|(path, urls, sha1)| {
            let dest = rel_join(game_dir, &path);
            let mut d = Download::new(urls[0].clone(), dest);
            if let Some(s) = sha1 {
                d = d.sha1(s);
            }
            d
        })
        .collect();
    download::download_all(downloads, crate::download::DEFAULT_CONCURRENCY, reporter.clone()).await?;

    let _ = tokio::fs::remove_file(&mrpack).await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_version_and_picks_primary() {
        let json = r#"[{
            "id":"abc","name":"v1","version_number":"1.0.0",
            "game_versions":["1.21.1"],"loaders":["fabric"],
            "files":[
              {"url":"https://x/a.jar","filename":"a.jar","primary":false,"hashes":{"sha1":"aa"}},
              {"url":"https://x/b.jar","filename":"b.jar","primary":true,"hashes":{"sha1":"bb"}}
            ],
            "dependencies":[{"project_id":"dep1","dependency_type":"required"}],
            "date_published":"2024-08-01T00:00:00Z"
        }]"#;
        let versions: Vec<Version> = serde_json::from_str(json).unwrap();
        assert_eq!(versions[0].primary_file().unwrap().filename, "b.jar");
        assert_eq!(versions[0].dependencies[0].project_id.as_deref(), Some("dep1"));
    }
}
