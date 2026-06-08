//! Minecraft *server* hosting support (the dedicated server jar).
//!
//! Each hosted server has its own directory (keyed by a stable id) so several
//! can run side by side on different ports. We download the dedicated server
//! jar, write the EULA acceptance and a `server.properties` from the user's
//! config, and hand the jar path to the caller, which runs
//! `java -jar server.jar nogui` and manages the process. Process lifecycle,
//! console streaming, and resource sampling live in the UI layer.

use std::path::{Path, PathBuf};

use crate::download::{self, Download};
use crate::paths::Paths;
use crate::progress::SharedReporter;
use crate::version::VersionJson;
use crate::{Error, Result};

/// Directory holding a hosted server instance (keyed by its config id).
pub fn server_dir(paths: &Paths, id: &str) -> PathBuf {
    paths.data_dir.join("servers").join(id)
}

/// Download (if needed) the dedicated server jar for `version` into `dir`.
pub async fn ensure_server_jar(
    dir: &Path,
    version: &VersionJson,
    reporter: &SharedReporter,
) -> Result<PathBuf> {
    let server = version
        .downloads
        .as_ref()
        .and_then(|d| d.server.as_ref())
        .ok_or_else(|| {
            Error::other(format!(
                "version {} has no dedicated server download",
                version.id
            ))
        })?;

    crate::util::ensure_dir(dir).await?;
    let jar = dir.join("server.jar");
    let dl = Download::new(server.url.clone(), jar.clone())
        .sha1(server.sha1.clone())
        .size(server.size);
    download::download_all(vec![dl], 4, reporter.clone()).await?;
    Ok(jar)
}

/// Download a runnable **Fabric** server launcher jar into `dir` (enables
/// server-side mods). Picks the latest stable Fabric loader + installer.
pub async fn ensure_fabric_server_jar(
    dir: &Path,
    game_version: &str,
    reporter: &SharedReporter,
) -> Result<PathBuf> {
    let loader = crate::modloader::fabric::latest_stable(game_version).await?;
    let installer = crate::modloader::fabric::latest_installer().await?;
    let url = crate::modloader::fabric::server_launcher_url(game_version, &loader, &installer);

    crate::util::ensure_dir(dir).await?;
    let jar = dir.join("server.jar");
    download::download_all(vec![Download::new(url, jar.clone())], 2, reporter.clone()).await?;
    Ok(jar)
}

/// Install a **Forge** server via the official installer's `--installServer`
/// (which downloads libraries and runs the binpatch/deobf processors), and
/// return the relative path of the launch *args file* produced for modern
/// Forge (1.17+). Run with `java @<that file> nogui`.
pub async fn ensure_forge_server(
    dir: &Path,
    mc_version: &str,
    java: &Path,
    reporter: &SharedReporter,
) -> Result<String> {
    let forge_ver = crate::modloader::forge::recommended_version(mc_version)
        .await?
        .ok_or_else(|| Error::other(format!("No Forge build found for Minecraft {mc_version}")))?;
    let full = format!("{mc_version}-{forge_ver}");
    let url = format!(
        "https://maven.minecraftforge.net/net/minecraftforge/forge/{full}/forge-{full}-installer.jar"
    );

    crate::util::ensure_dir(dir).await?;
    let installer = dir.join("forge-installer.jar");
    download::download_all(vec![Download::new(url, installer.clone())], 2, reporter.clone()).await?;

    // The installer downloads libraries and runs processors into `dir`.
    let status = tokio::process::Command::new(java)
        .current_dir(dir)
        .arg("-jar")
        .arg("forge-installer.jar")
        .arg("--installServer")
        .arg(dir)
        .status()
        .await
        .map_err(|e| Error::io(java, e))?;
    if !status.success() {
        return Err(Error::other(format!(
            "Forge installer failed (exit {:?})",
            status.code()
        )));
    }

    let args_name = if cfg!(windows) { "win_args.txt" } else { "unix_args.txt" };
    let rel = format!("libraries/net/minecraftforge/forge/{full}/{args_name}");
    if !dir.join(&rel).exists() {
        return Err(Error::other(
            "Forge installed, but no launch args file was produced (Forge < 1.17 isn't supported for hosting yet)".to_string(),
        ));
    }
    Ok(rel)
}

/// Download a **Paper** server jar (runnable like vanilla) for plugin support.
pub async fn ensure_paper_jar(
    dir: &Path,
    mc_version: &str,
    reporter: &SharedReporter,
) -> Result<PathBuf> {
    #[derive(serde::Deserialize)]
    struct Versions {
        builds: Vec<u32>,
    }
    #[derive(serde::Deserialize)]
    struct BuildInfo {
        downloads: PaperDownloads,
    }
    #[derive(serde::Deserialize)]
    struct PaperDownloads {
        application: PaperApp,
    }
    #[derive(serde::Deserialize)]
    struct PaperApp {
        name: String,
    }

    const BASE: &str = "https://api.papermc.io/v2/projects/paper";
    let versions = crate::http::client()
        .get(format!("{BASE}/versions/{mc_version}"))
        .send()
        .await?
        .error_for_status()?
        .json::<Versions>()
        .await?;
    let build = *versions
        .builds
        .last()
        .ok_or_else(|| Error::other(format!("No Paper build for Minecraft {mc_version}")))?;

    let info = crate::http::client()
        .get(format!("{BASE}/versions/{mc_version}/builds/{build}"))
        .send()
        .await?
        .error_for_status()?
        .json::<BuildInfo>()
        .await?;
    let name = info.downloads.application.name;
    let url = format!("{BASE}/versions/{mc_version}/builds/{build}/downloads/{name}");

    crate::util::ensure_dir(dir).await?;
    let jar = dir.join("server.jar");
    download::download_all(vec![Download::new(url, jar.clone())], 2, reporter.clone()).await?;
    Ok(jar)
}

/// Write `eula.txt` accepting Mojang's EULA — required before a server starts.
pub async fn accept_eula(dir: &Path) -> Result<()> {
    let path = dir.join("eula.txt");
    tokio::fs::write(&path, "# Accepted via Aurora Launcher\neula=true\n")
        .await
        .map_err(|e| Error::io(&path, e))
}

/// Write the parts of `server.properties` the launcher manages, preserving any
/// other keys the server may have written on a previous run.
pub async fn write_properties(
    dir: &Path,
    port: u16,
    max_players: u32,
    motd: &str,
) -> Result<()> {
    let path = dir.join("server.properties");
    let existing = tokio::fs::read_to_string(&path).await.unwrap_or_default();

    let mut managed: Vec<(String, String)> = vec![
        ("server-port".into(), port.to_string()),
        ("query.port".into(), port.to_string()),
        ("max-players".into(), max_players.to_string()),
        ("motd".into(), sanitize_motd(motd)),
    ];

    // Keep any unmanaged keys from the existing file.
    let mut out = String::new();
    for line in existing.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('#') || !trimmed.contains('=') {
            out.push_str(line);
            out.push('\n');
            continue;
        }
        let key = trimmed.split('=').next().unwrap_or("").trim();
        if managed.iter().any(|(k, _)| k == key) {
            continue; // replaced below
        }
        out.push_str(line);
        out.push('\n');
    }
    for (k, v) in managed.drain(..) {
        out.push_str(&format!("{k}={v}\n"));
    }

    crate::util::ensure_dir(dir).await?;
    tokio::fs::write(&path, out)
        .await
        .map_err(|e| Error::io(&path, e))
}

fn sanitize_motd(motd: &str) -> String {
    // server.properties is line-based; strip newlines and escape nothing else.
    motd.replace(['\n', '\r'], " ")
}

/// Parse a player name from a server log line: `Some((joined, name))`.
pub fn parse_player_event(line: &str) -> Option<(bool, String)> {
    for (marker, joined) in [(" joined the game", true), (" left the game", false)] {
        if let Some(idx) = line.find(marker) {
            let name = line[..idx].split_whitespace().last().unwrap_or("").to_string();
            if !name.is_empty() {
                return Some((joined, name));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_join_and_leave() {
        assert_eq!(
            parse_player_event("[12:00:00] [Server thread/INFO]: Steve joined the game"),
            Some((true, "Steve".into()))
        );
        assert_eq!(
            parse_player_event("[12:01:00] [Server thread/INFO]: Alex left the game"),
            Some((false, "Alex".into()))
        );
        assert_eq!(parse_player_event("Done (5.1s)! For help, type \"help\""), None);
    }

    #[test]
    fn motd_is_single_line() {
        assert_eq!(sanitize_motd("hello\nworld"), "hello world");
    }
}
