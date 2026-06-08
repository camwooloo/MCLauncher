//! Modloader support: Fabric, Quilt, and (scaffolded) Forge.
//!
//! Fabric and Quilt are wonderfully launcher-friendly: their meta APIs serve a
//! complete *launcher profile JSON* with `inheritsFrom` pointing at the vanilla
//! version and libraries expressed as Maven coordinate + repo URL. We already
//! parse and merge exactly that shape (see [`crate::version`] and
//! [`crate::install::Installer::resolve_version`]), so "installing" a loader is
//! just: fetch the profile JSON, write it to `versions/<id>/<id>.json`, and
//! then drive the normal install/launch path with that id.
//!
//! Forge is different — it ships an *installer jar* with binpatch/deobfuscation
//! "processors" that must run locally. We expose Forge version discovery here;
//! full installer execution is the remaining piece (see [`forge`]).

use serde::Deserialize;

use crate::paths::Paths;
use crate::version::VersionJson;
use crate::{Error, Result};

/// Which loader a profile uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Loader {
    Vanilla,
    Fabric,
    Quilt,
    Forge,
}

/// A discovered loader version.
#[derive(Debug, Clone)]
pub struct LoaderVersion {
    pub version: String,
    pub stable: bool,
}

/// Fabric and Quilt share the same v2/v3 meta API shape, so one implementation
/// parameterised by base URL serves both.
mod fabriclike {
    use super::*;

    #[derive(Deserialize)]
    pub(super) struct LoaderEntry {
        pub loader: LoaderInfo,
    }

    #[derive(Deserialize)]
    pub(super) struct LoaderInfo {
        pub version: String,
        #[serde(default)]
        pub stable: bool,
    }

    /// List loader versions available for a game version, newest first.
    pub(super) async fn loader_versions(
        base: &str,
        game_version: &str,
    ) -> Result<Vec<LoaderVersion>> {
        let url = format!("{base}/versions/loader/{game_version}");
        let entries = crate::http::client()
            .get(&url)
            .send()
            .await?
            .error_for_status()?
            .json::<Vec<LoaderEntry>>()
            .await?;
        Ok(entries
            .into_iter()
            .map(|e| LoaderVersion {
                version: e.loader.version,
                stable: e.loader.stable,
            })
            .collect())
    }

    /// Fetch the profile JSON, write it to disk, and return its version id.
    pub(super) async fn install_profile(
        base: &str,
        paths: &Paths,
        game_version: &str,
        loader_version: &str,
    ) -> Result<String> {
        let url =
            format!("{base}/versions/loader/{game_version}/{loader_version}/profile/json");
        let text = crate::http::client()
            .get(&url)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;

        // Parse just to learn the generated id (e.g. "fabric-loader-0.16.0-1.21").
        let profile = VersionJson::parse(&text)?;
        let path = paths.version_json(&profile.id);
        crate::util::ensure_parent(&path).await?;
        tokio::fs::write(&path, text)
            .await
            .map_err(|e| Error::io(&path, e))?;
        Ok(profile.id)
    }
}

/// Fabric loader integration.
pub mod fabric {
    use super::*;

    pub const META: &str = "https://meta.fabricmc.net/v2";

    pub async fn loader_versions(game_version: &str) -> Result<Vec<LoaderVersion>> {
        fabriclike::loader_versions(META, game_version).await
    }

    /// Latest stable loader for a game version.
    pub async fn latest_stable(game_version: &str) -> Result<String> {
        let versions = loader_versions(game_version).await?;
        versions
            .iter()
            .find(|v| v.stable)
            .or_else(|| versions.first())
            .map(|v| v.version.clone())
            .ok_or_else(|| Error::other(format!("no Fabric loader for {game_version}")))
    }

    /// Install a Fabric profile; returns the launchable version id.
    pub async fn install(
        paths: &Paths,
        game_version: &str,
        loader_version: &str,
    ) -> Result<String> {
        fabriclike::install_profile(META, paths, game_version, loader_version).await
    }

    #[derive(Deserialize)]
    struct InstallerEntry {
        version: String,
        #[serde(default)]
        stable: bool,
    }

    /// Latest Fabric *installer* version (needed to build the server-jar URL).
    pub async fn latest_installer() -> Result<String> {
        let list = crate::http::client()
            .get(format!("{META}/versions/installer"))
            .send()
            .await?
            .error_for_status()?
            .json::<Vec<InstallerEntry>>()
            .await?;
        list.iter()
            .find(|i| i.stable)
            .or_else(|| list.first())
            .map(|i| i.version.clone())
            .ok_or_else(|| Error::other("no Fabric installer available"))
    }

    /// URL of the runnable Fabric *server* launcher jar.
    pub fn server_launcher_url(game_version: &str, loader: &str, installer: &str) -> String {
        format!("{META}/versions/loader/{game_version}/{loader}/{installer}/server/jar")
    }
}

/// Quilt loader integration (Fabric-compatible API).
pub mod quilt {
    use super::*;

    pub const META: &str = "https://meta.quiltmc.org/v3";

    pub async fn loader_versions(game_version: &str) -> Result<Vec<LoaderVersion>> {
        fabriclike::loader_versions(META, game_version).await
    }

    pub async fn latest_stable(game_version: &str) -> Result<String> {
        let versions = loader_versions(game_version).await?;
        // Quilt marks pre-releases via a "-beta"/"-pre" suffix; prefer those
        // without one when no explicit stable flag is set.
        versions
            .iter()
            .find(|v| v.stable)
            .or_else(|| versions.iter().find(|v| !v.version.contains('-')))
            .or_else(|| versions.first())
            .map(|v| v.version.clone())
            .ok_or_else(|| Error::other(format!("no Quilt loader for {game_version}")))
    }

    pub async fn install(
        paths: &Paths,
        game_version: &str,
        loader_version: &str,
    ) -> Result<String> {
        fabriclike::install_profile(META, paths, game_version, loader_version).await
    }
}

/// Forge integration — version discovery is implemented; running the Forge
/// installer's processors is the remaining work.
pub mod forge {
    use super::*;
    use std::collections::HashMap;

    const PROMOTIONS: &str =
        "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json";

    #[derive(Deserialize)]
    struct Promotions {
        promos: HashMap<String, String>,
    }

    /// The recommended (falling back to latest) Forge version for a game
    /// version, e.g. "47.3.0" for "1.20.1".
    pub async fn recommended_version(game_version: &str) -> Result<Option<String>> {
        let promos = crate::http::client()
            .get(PROMOTIONS)
            .send()
            .await?
            .error_for_status()?
            .json::<Promotions>()
            .await?
            .promos;

        Ok(promos
            .get(&format!("{game_version}-recommended"))
            .or_else(|| promos.get(&format!("{game_version}-latest")))
            .cloned())
    }

    /// Install the Forge **client** into `game_dir` by running the official
    /// installer's `--installClient` (which downloads libraries and runs the
    /// processors), then return the generated version-profile id (e.g.
    /// `1.20.1-forge-47.3.0`). That id resolves via the normal `inheritsFrom`
    /// path for launching.
    pub async fn install_client(
        game_dir: &std::path::Path,
        game_version: &str,
        java: &std::path::Path,
        reporter: &crate::progress::SharedReporter,
    ) -> Result<String> {
        let forge_ver = recommended_version(game_version)
            .await?
            .ok_or_else(|| Error::other(format!("No Forge build for Minecraft {game_version}")))?;
        let full = format!("{game_version}-{forge_ver}");
        let url = format!(
            "https://maven.minecraftforge.net/net/minecraftforge/forge/{full}/forge-{full}-installer.jar"
        );

        crate::util::ensure_dir(game_dir).await?;
        // The installer expects a launcher_profiles.json to exist.
        let profiles = game_dir.join("launcher_profiles.json");
        if !profiles.exists() {
            let _ = tokio::fs::write(&profiles, b"{\"profiles\":{}}").await;
        }
        let installer = game_dir.join(".forge-installer.jar");
        crate::download::download_all(
            vec![crate::download::Download::new(url, installer.clone())],
            2,
            reporter.clone(),
        )
        .await?;

        let status = tokio::process::Command::new(java)
            .current_dir(game_dir)
            .arg("-jar")
            .arg(&installer)
            .arg("--installClient")
            .arg(game_dir)
            .status()
            .await
            .map_err(|e| Error::io(java, e))?;
        let _ = tokio::fs::remove_file(&installer).await;
        if !status.success() {
            return Err(Error::other("Forge client installer failed".to_string()));
        }

        let id = format!("{game_version}-forge-{forge_ver}");
        if game_dir.join("versions").join(&id).join(format!("{id}.json")).exists() {
            return Ok(id);
        }
        // Fallback: find a versions/* dir produced by the installer.
        if let Ok(mut rd) = tokio::fs::read_dir(game_dir.join("versions")).await {
            while let Ok(Some(entry)) = rd.next_entry().await {
                let name = entry.file_name().to_string_lossy().into_owned();
                if name.contains("forge") && name.contains(&forge_ver) {
                    return Ok(name);
                }
            }
        }
        Err(Error::other("Forge installed but no version profile was found".to_string()))
    }
}

/// NeoForge — the community fork of Forge. Same installer model.
pub mod neoforge {
    use super::*;

    const VERSIONS: &str =
        "https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge";

    #[derive(Deserialize)]
    struct NeoVersions {
        versions: Vec<String>,
    }

    /// Latest NeoForge version for a game version. NeoForge versions are
    /// `<mcMinor>.<patch>.<build>` (e.g. MC 1.21.1 → 21.1.x), so we match on the
    /// `1.` stripped prefix. Returns `None` for versions NeoForge doesn't cover.
    pub async fn latest_version(game_version: &str) -> Result<Option<String>> {
        let prefix = match game_version.strip_prefix("1.") {
            Some(rest) => rest.to_string(), // "21.1" or "20.4"
            None => return Ok(None),
        };
        let all = crate::http::client()
            .get(VERSIONS)
            .send()
            .await?
            .error_for_status()?
            .json::<NeoVersions>()
            .await?
            .versions;
        Ok(all
            .into_iter()
            .filter(|v| v.starts_with(&prefix))
            .last())
    }

    pub async fn install_client(
        game_dir: &std::path::Path,
        game_version: &str,
        java: &std::path::Path,
        reporter: &crate::progress::SharedReporter,
    ) -> Result<String> {
        let ver = latest_version(game_version)
            .await?
            .ok_or_else(|| Error::other(format!("No NeoForge build for Minecraft {game_version}")))?;
        let url = format!(
            "https://maven.neoforged.net/releases/net/neoforged/neoforge/{ver}/neoforge-{ver}-installer.jar"
        );

        crate::util::ensure_dir(game_dir).await?;
        let profiles = game_dir.join("launcher_profiles.json");
        if !profiles.exists() {
            let _ = tokio::fs::write(&profiles, b"{\"profiles\":{}}").await;
        }
        let installer = game_dir.join(".neoforge-installer.jar");
        crate::download::download_all(
            vec![crate::download::Download::new(url, installer.clone())],
            2,
            reporter.clone(),
        )
        .await?;

        let status = tokio::process::Command::new(java)
            .current_dir(game_dir)
            .arg("-jar")
            .arg(&installer)
            .arg("--installClient")
            .arg(game_dir)
            .status()
            .await
            .map_err(|e| Error::io(java, e))?;
        let _ = tokio::fs::remove_file(&installer).await;
        if !status.success() {
            return Err(Error::other("NeoForge client installer failed".to_string()));
        }

        let id = format!("neoforge-{ver}");
        if game_dir.join("versions").join(&id).join(format!("{id}.json")).exists() {
            return Ok(id);
        }
        if let Ok(mut rd) = tokio::fs::read_dir(game_dir.join("versions")).await {
            while let Ok(Some(entry)) = rd.next_entry().await {
                let name = entry.file_name().to_string_lossy().into_owned();
                if name.contains("neoforge") && name.contains(&ver) {
                    return Ok(name);
                }
            }
        }
        Err(Error::other("NeoForge installed but no version profile was found".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fabriclike_parses_loader_list() {
        let json = r#"[
            {"loader":{"version":"0.16.0","stable":true}},
            {"loader":{"version":"0.16.1-beta","stable":false}}
        ]"#;
        let entries: Vec<fabriclike::LoaderEntry> = serde_json::from_str(json).unwrap();
        assert_eq!(entries.len(), 2);
        assert!(entries[0].loader.stable);
        assert_eq!(entries[1].loader.version, "0.16.1-beta");
    }
}
