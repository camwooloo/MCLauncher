//! One-click installers for game mods distributed as **GitHub release** archives
//! (Seamless Co-op, SKSE, Mod Engine 2, Cyber Engine Tweaks, CyberpunkMP…).
//!
//! Flow: resolve the latest release's matching asset via the GitHub API,
//! download it, extract (`.zip` or `.7z`) into the destination, optionally
//! stripping a single wrapper directory (e.g. `ModEngine-2.1.0.0-win64/`).
//! Skyrim Together Reborn is Nexus-only (no API downloads for free accounts),
//! so for it we instead extract a user-downloaded zip via
//! [`extract_archive_into`] + [`merge_move`].

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::download::{self, Download};
use crate::progress::SharedReporter;
use crate::{Error, Result};

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
}

/// Resolve the latest release of `repo` and pick the first asset whose
/// lowercase name passes `pick`. Returns `(tag, asset_name, download_url)`.
pub async fn github_latest_asset(
    repo: &str,
    pick: impl Fn(&str) -> bool,
) -> Result<(String, String, String)> {
    let rel: Release = crate::http::client()
        .get(format!("https://api.github.com/repos/{repo}/releases/latest"))
        .header("Accept", "application/vnd.github+json")
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let asset = rel
        .assets
        .iter()
        .find(|a| pick(&a.name.to_lowercase()))
        .ok_or_else(|| Error::other(format!("no matching download in the latest {repo} release")))?;
    Ok((rel.tag_name, asset.name.clone(), asset.browser_download_url.clone()))
}

/// Extract a `.zip` or `.7z` archive into `dest` (created if missing).
/// Blocking — call from `spawn_blocking`.
pub fn extract_archive_into(archive: &Path, dest: &Path) -> Result<()> {
    std::fs::create_dir_all(dest).map_err(|e| Error::io(dest, e))?;
    let ext = archive
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    match ext.as_str() {
        "7z" => sevenz_rust::decompress_file(archive, dest)
            .map_err(|e| Error::other(format!("7z extraction failed: {e}"))),
        _ => {
            let f = std::fs::File::open(archive).map_err(|e| Error::io(archive, e))?;
            let mut z = zip::ZipArchive::new(f)?;
            for i in 0..z.len() {
                let mut entry = z.by_index(i)?;
                if entry.is_dir() {
                    continue;
                }
                let Some(rel) = entry.enclosed_name() else { continue };
                let out = dest.join(rel);
                if let Some(p) = out.parent() {
                    std::fs::create_dir_all(p).ok();
                }
                let mut of = std::fs::File::create(&out).map_err(|e| Error::io(&out, e))?;
                std::io::copy(&mut entry, &mut of).map_err(|e| Error::io(&out, e))?;
            }
            Ok(())
        }
    }
}

/// Recursively move the *contents* of `src` into `dest`, overwriting existing
/// files (a merge, not a replace — other files in `dest` are untouched).
pub fn merge_move(src: &Path, dest: &Path) -> Result<()> {
    std::fs::create_dir_all(dest).map_err(|e| Error::io(dest, e))?;
    for entry in std::fs::read_dir(src).map_err(|e| Error::io(src, e))? {
        let entry = entry.map_err(|e| Error::io(src, e))?;
        let from = entry.path();
        let to = dest.join(entry.file_name());
        if from.is_dir() {
            merge_move(&from, &to)?;
        } else {
            if to.exists() {
                std::fs::remove_file(&to).map_err(|e| Error::io(&to, e))?;
            }
            if std::fs::rename(&from, &to).is_err() {
                std::fs::copy(&from, &to).map_err(|e| Error::io(&to, e))?;
            }
        }
    }
    Ok(())
}

/// Find a file named `needle` anywhere under `root` (case-insensitive),
/// returning its full path. Used to locate e.g. `SkyrimTogetherReborn.exe`
/// inside a freshly extracted archive whatever its wrapper folders are.
pub fn find_file(root: &Path, needle: &str) -> Option<PathBuf> {
    let needle = needle.to_lowercase();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for entry in rd.flatten() {
            let p = entry.path();
            if p.is_dir() {
                stack.push(p);
            } else if p
                .file_name()
                .map(|n| n.to_string_lossy().to_lowercase() == needle)
                .unwrap_or(false)
            {
                return Some(p);
            }
        }
    }
    None
}

/// Newest `.zip` in the user's Downloads folder whose lowercase name passes
/// `pred` — backs the "I downloaded it — install" guided flows for mods that
/// can't be fetched automatically (Nexus).
pub fn find_downloaded_zip(pred: impl Fn(&str) -> bool) -> Option<PathBuf> {
    let downloads =
        dirs::download_dir().or_else(|| dirs::home_dir().map(|h| h.join("Downloads")))?;
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    for entry in std::fs::read_dir(downloads).ok()?.flatten() {
        let p = entry.path();
        let Some(name) = p.file_name().map(|n| n.to_string_lossy().to_lowercase()) else { continue };
        if name.ends_with(".zip") && pred(&name) {
            let Some(modified) = entry.metadata().ok().and_then(|m| m.modified().ok()) else { continue };
            if best.as_ref().map(|(t, _)| modified > *t).unwrap_or(true) {
                best = Some((modified, p));
            }
        }
    }
    best.map(|(_, p)| p)
}

/// Download the latest matching GitHub release asset and extract it into
/// `dest`. With `strip_top`, a single wrapper directory in the archive is
/// flattened away (its contents land directly in `dest`). Returns the tag.
pub async fn install_github_archive(
    repo: &str,
    pick: impl Fn(&str) -> bool,
    dest: &Path,
    strip_top: bool,
    reporter: &SharedReporter,
) -> Result<String> {
    let (tag, name, url) = github_latest_asset(repo, pick).await?;

    tokio::fs::create_dir_all(dest)
        .await
        .map_err(|e| Error::io(dest, e))?;
    let tmp_archive = dest.join(format!(".aurora-dl-{name}"));
    download::download_all(
        vec![Download::new(url, tmp_archive.clone())],
        2,
        reporter.clone(),
    )
    .await?;

    let staging = dest.join(".aurora-extract");
    let _ = std::fs::remove_dir_all(&staging);

    let tmp2 = tmp_archive.clone();
    let staging2 = staging.clone();
    let dest2 = dest.to_path_buf();
    tokio::task::spawn_blocking(move || -> Result<()> {
        extract_archive_into(&tmp2, &staging2)?;

        // Decide what to move: the lone wrapper dir's contents, or everything.
        let entries: Vec<_> = std::fs::read_dir(&staging2)
            .map_err(|e| Error::io(&staging2, e))?
            .flatten()
            .collect();
        let src = if strip_top && entries.len() == 1 && entries[0].path().is_dir() {
            entries[0].path()
        } else {
            staging2.clone()
        };
        merge_move(&src, &dest2)?;
        let _ = std::fs::remove_dir_all(&staging2);
        Ok(())
    })
    .await
    .map_err(|e| Error::other(format!("extract task panicked: {e}")))??;

    let _ = tokio::fs::remove_file(&tmp_archive).await;
    Ok(tag)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_move_merges_and_overwrites() {
        let base = std::env::temp_dir().join(format!("aurora-mm-{}", std::process::id()));
        let src = base.join("src");
        let dest = base.join("dest");
        std::fs::create_dir_all(src.join("sub")).unwrap();
        std::fs::create_dir_all(&dest).unwrap();
        std::fs::write(src.join("a.txt"), "new").unwrap();
        std::fs::write(src.join("sub/b.txt"), "b").unwrap();
        std::fs::write(dest.join("a.txt"), "old").unwrap();
        std::fs::write(dest.join("keep.txt"), "keep").unwrap();

        merge_move(&src, &dest).unwrap();
        assert_eq!(std::fs::read_to_string(dest.join("a.txt")).unwrap(), "new");
        assert_eq!(std::fs::read_to_string(dest.join("sub/b.txt")).unwrap(), "b");
        assert_eq!(std::fs::read_to_string(dest.join("keep.txt")).unwrap(), "keep");
        let _ = std::fs::remove_dir_all(&base);
    }
}
