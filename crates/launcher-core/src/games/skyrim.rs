//! Skyrim Special Edition detection & launch, including Skyrim Together Reborn.
//!
//! Modded Skyrim (and Skyrim Together) launches through **SKSE** — the Skyrim
//! Script Extender (`skse64_loader.exe`) — rather than the game exe directly.
//! Skyrim Together Reborn ships its own loader (`SkyrimTogetherReborn.exe`)
//! which starts the game with the co-op client injected; players then connect
//! to a server by IP from the in-game overlay.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::games::launch_detached;
use crate::steam;

/// Skyrim SE Steam app id.
pub const APP_ID: u32 = 489830;

/// GitHub repo distributing SKSE64 release archives (.7z).
pub const SKSE_REPO: &str = "ianpatt/skse64";

/// Skyrim Together Reborn's Nexus page — its binaries are Nexus-only (no API
/// downloads for free accounts), so install is guided: open this page, the
/// user downloads the zip, and [`install_together_from_zip`] places it.
pub const TOGETHER_NEXUS_URL: &str =
    "https://www.nexusmods.com/skyrimspecialedition/mods/69993?tab=files";

/// Address Library for SKSE Plugins — required by Skyrim Together at runtime
/// ("Failed to load Skyrim Address Library"). Also Nexus-only.
pub const ADDRESS_LIBRARY_NEXUS_URL: &str =
    "https://www.nexusmods.com/skyrimspecialedition/mods/32444?tab=files";

/// Detected state of the Skyrim install.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkyrimInfo {
    pub installed: bool,
    pub install_dir: Option<PathBuf>,
    /// "steam" or "epic", when installed.
    pub source: Option<String>,
    /// `skse64_loader.exe` present (modded launch available).
    pub has_skse: bool,
    /// Skyrim Together Reborn loader present.
    pub has_skyrim_together: bool,
    /// Address Library for SKSE Plugins present (Skyrim Together needs it).
    pub has_address_library: bool,
    /// Resolved Skyrim Together loader path, when found.
    pub skyrim_together_path: Option<PathBuf>,
}

/// Which way to launch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SkyrimLaunch {
    /// Official launcher / Steam (vanilla).
    Vanilla,
    /// Script Extender (modded).
    Skse,
    /// Skyrim Together Reborn (co-op).
    SkyrimTogether,
}

/// Candidate locations for the Skyrim Together loader, relative to the install.
/// Since 1.8 the loader is `SkyrimTogether.exe` inside the mod's app folder
/// under `Data/`; older names kept for back-compat.
const STR_CANDIDATES: &[&str] = &[
    "Data/SkyrimTogetherReborn/SkyrimTogether.exe",
    "SkyrimTogetherReborn/SkyrimTogether.exe",
    "Data/SkyrimTogetherReborn/SkyrimTogetherReborn.exe",
    "SkyrimTogetherReborn/SkyrimTogetherReborn.exe",
    "SkyrimTogetherReborn.exe",
];

/// Detect the Skyrim SE install (Steam, falling back to Epic) and its co-op
/// tooling.
pub fn detect() -> SkyrimInfo {
    let (install_dir, source) = match steam::find_app_install_dir(APP_ID) {
        Some(d) => (Some(d), Some("steam".to_string())),
        None => match crate::epic::find_install("skyrim special edition") {
            Some(d) => (Some(d), Some("epic".to_string())),
            None => (None, None),
        },
    };
    let has_skse = install_dir
        .as_ref()
        .map(|d| d.join("skse64_loader.exe").exists())
        .unwrap_or(false);
    let skyrim_together_path = install_dir.as_ref().and_then(|d| find_skyrim_together(d));
    let has_address_library = install_dir
        .as_ref()
        .map(|d| has_address_library(d))
        .unwrap_or(false);

    SkyrimInfo {
        installed: install_dir.is_some(),
        has_skse,
        has_skyrim_together: skyrim_together_path.is_some(),
        has_address_library,
        skyrim_together_path,
        source,
        install_dir,
    }
}

/// Address Library installs `version*-….bin` files into `Data/SKSE/Plugins`.
fn has_address_library(install_dir: &Path) -> bool {
    let plugins = install_dir.join("Data").join("SKSE").join("Plugins");
    let Ok(rd) = std::fs::read_dir(plugins) else { return false };
    rd.flatten().any(|e| {
        let n = e.file_name().to_string_lossy().to_lowercase();
        n.starts_with("version") && n.ends_with(".bin")
    })
}

/// Install Address Library from a Nexus zip ("All in one"): extract, find the
/// `SKSE` folder wherever it sits, merge its parent into `Data/`.
pub fn install_address_library_from_zip(install_dir: &Path, zip: &Path) -> crate::Result<()> {
    use crate::games::install::{extract_archive_into, merge_move};

    let staging = install_dir.join(".aurora-addrlib-extract");
    let _ = std::fs::remove_dir_all(&staging);
    extract_archive_into(zip, &staging)?;

    // Find a dir literally named SKSE (the zip root is the Data layout).
    let mut skse_dir: Option<PathBuf> = None;
    let mut stack = vec![staging.clone()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for entry in rd.flatten() {
            let p = entry.path();
            if p.is_dir() {
                if p.file_name().map(|n| n.eq_ignore_ascii_case("SKSE")).unwrap_or(false) {
                    skse_dir = Some(p.clone());
                }
                stack.push(p);
            }
        }
    }
    let skse = skse_dir.ok_or_else(|| {
        let _ = std::fs::remove_dir_all(&staging);
        crate::Error::other(
            "That zip doesn't look like Address Library — download \"All in one\" for your game version from the Nexus page",
        )
    })?;
    let data_root = skse.parent().unwrap_or(&staging).to_path_buf();
    merge_move(&data_root, &install_dir.join("Data"))?;
    let _ = std::fs::remove_dir_all(&staging);
    Ok(())
}

fn find_skyrim_together(install_dir: &Path) -> Option<PathBuf> {
    STR_CANDIDATES
        .iter()
        .map(|rel| install_dir.join(rel))
        .find(|p| p.exists())
}

/// One-click install of the latest SKSE64 into the Skyrim install dir
/// (`skse64_loader.exe`, DLLs, `Data/Scripts`). The release `.7z` wraps
/// everything in a version folder, which we strip. Returns the release tag.
pub async fn install_skse(
    install_dir: &Path,
    reporter: &crate::progress::SharedReporter,
) -> crate::Result<String> {
    crate::games::install::install_github_archive(
        SKSE_REPO,
        |n| n.ends_with(".7z"),
        install_dir,
        true,
        reporter,
    )
    .await
}

/// Install Skyrim Together Reborn from a zip the user downloaded from Nexus.
///
/// The 1.8+ zip is a Data-folder mod: `SkyrimTogether.esp`, `scripts/`,
/// `meshes/`, plus the `SkyrimTogetherReborn/` app folder (whose loader is
/// `SkyrimTogether.exe`). We locate the loader regardless of wrapper folders
/// and merge the level that *contains* `SkyrimTogetherReborn/` into
/// `<install>/Data/`.
pub fn install_together_from_zip(install_dir: &Path, zip: &Path) -> crate::Result<()> {
    use crate::games::install::{extract_archive_into, find_file, merge_move};

    let staging = install_dir.join(".aurora-str-extract");
    let _ = std::fs::remove_dir_all(&staging);
    extract_archive_into(zip, &staging)?;

    let exe = find_file(&staging, "SkyrimTogether.exe")
        .or_else(|| find_file(&staging, "SkyrimTogetherReborn.exe"))
        .ok_or_else(|| {
            let _ = std::fs::remove_dir_all(&staging);
            crate::Error::other(
                "That zip doesn't contain the Skyrim Together loader — download the main file from the Nexus page",
            )
        })?;
    // exe = …/SkyrimTogetherReborn/SkyrimTogether.exe → the mod root is the
    // folder holding SkyrimTogetherReborn/ (it also has the esp + scripts).
    let app_dir = exe.parent().unwrap_or(&staging);
    let mod_root = app_dir.parent().unwrap_or(&staging).to_path_buf();
    merge_move(&mod_root, &install_dir.join("Data"))?;
    let _ = std::fs::remove_dir_all(&staging);
    Ok(())
}

/// Newest `*skyrim*together*.zip` in the user's Downloads folder, if any —
/// lets "I downloaded it" find the file without a path prompt.
pub fn find_downloaded_together_zip() -> Option<PathBuf> {
    crate::games::install::find_downloaded_zip(|n| n.contains("skyrim") && n.contains("together"))
}

/// Newest Address Library zip in Downloads (Nexus names it "All in one (…)").
pub fn find_downloaded_addrlib_zip() -> Option<PathBuf> {
    crate::games::install::find_downloaded_zip(|n| {
        (n.contains("address") && n.contains("librar")) || (n.contains("all") && n.contains("one"))
    })
}

/// Launch Skyrim in the requested mode; returns the child pid.
pub fn launch(info: &SkyrimInfo, mode: SkyrimLaunch) -> crate::Result<u32> {
    let dir = info
        .install_dir
        .as_ref()
        .ok_or_else(|| crate::Error::other("Skyrim Special Edition is not installed"))?;

    let exe = match mode {
        SkyrimLaunch::Vanilla => dir.join("SkyrimSE.exe"),
        SkyrimLaunch::Skse => dir.join("skse64_loader.exe"),
        // cwd must be the game root for all modes — the Together loader
        // resolves SkyrimSE.exe relative to it (else it prompts the user).
        SkyrimLaunch::SkyrimTogether => info
            .skyrim_together_path
            .clone()
            .ok_or_else(|| crate::Error::other("Skyrim Together Reborn is not installed"))?,
    };

    launch_detached(&exe, &[], Some(dir), &[])
}

// --- Skyrim Together dedicated server (hosting) --------------------------

fn together_dir(install_dir: &Path) -> PathBuf {
    install_dir.join("Data").join("SkyrimTogetherReborn")
}
fn server_exe(install_dir: &Path) -> PathBuf {
    together_dir(install_dir).join("SkyrimTogetherServer.exe")
}
fn server_ini(install_dir: &Path) -> PathBuf {
    together_dir(install_dir).join("config").join("STServer.ini")
}

/// Host-side settings, mapped to the keys Aurora surfaces from `STServer.ini`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TogetherServerConfig {
    /// The dedicated-server exe is present (hosting is possible).
    pub available: bool,
    pub server_name: String,
    pub password: String,
    pub max_players: u32,
    pub port: u16,
    pub pvp: bool,
    pub death_system: bool,
    pub xp_sync: bool,
    pub item_drops: bool,
    pub auto_party_join: bool,
    /// Game difficulty 0–5 (Novice…Legendary).
    pub difficulty: u32,
}

impl Default for TogetherServerConfig {
    fn default() -> Self {
        Self {
            available: false,
            server_name: "Aurora Together Server".into(),
            password: String::new(),
            max_players: 8,
            port: 10578,
            pvp: false,
            death_system: true,
            xp_sync: true,
            item_drops: false,
            auto_party_join: true,
            difficulty: 4,
        }
    }
}

fn parse_bool(v: &str) -> bool {
    matches!(v.trim(), "true" | "1" | "yes")
}
fn bstr(b: bool) -> &'static str {
    if b {
        "true"
    } else {
        "false"
    }
}

/// Read the Together server config (defaults fill any missing keys).
/// `available` reflects whether the dedicated-server exe exists.
pub fn read_server_config(install_dir: &Path) -> TogetherServerConfig {
    let mut cfg = TogetherServerConfig {
        available: server_exe(install_dir).exists(),
        ..Default::default()
    };
    if let Ok(text) = std::fs::read_to_string(server_ini(install_dir)) {
        for line in text.lines() {
            let Some((k, v)) = line.trim().split_once('=') else { continue };
            let (k, v) = (k.trim(), v.trim());
            match k {
                "sServerName" => cfg.server_name = v.to_string(),
                "sPassword" => cfg.password = v.to_string(),
                "uMaxPlayerCount" => cfg.max_players = v.parse().unwrap_or(cfg.max_players),
                "uPort" => cfg.port = v.parse().unwrap_or(cfg.port),
                "bEnablePvp" => cfg.pvp = parse_bool(v),
                "bEnableDeathSystem" => cfg.death_system = parse_bool(v),
                "bEnableXpSync" => cfg.xp_sync = parse_bool(v),
                "bEnableItemDrops" => cfg.item_drops = parse_bool(v),
                "bAutoPartyJoin" => cfg.auto_party_join = parse_bool(v),
                "uDifficulty" => cfg.difficulty = v.parse().unwrap_or(cfg.difficulty),
                _ => {}
            }
        }
    }
    cfg
}

/// Write the exposed keys back into `STServer.ini` *in place* — every other
/// line, comment and section is preserved; missing keys are appended under the
/// right `[section]`.
pub fn write_server_config(install_dir: &Path, cfg: &TogetherServerConfig) -> crate::Result<()> {
    let path = server_ini(install_dir);
    let updates: &[(&str, String, &str)] = &[
        ("sServerName", cfg.server_name.clone(), "GameServer"),
        ("sPassword", cfg.password.clone(), "GameServer"),
        ("uMaxPlayerCount", cfg.max_players.to_string(), "GameServer"),
        ("uPort", cfg.port.to_string(), "GameServer"),
        ("bEnablePvp", bstr(cfg.pvp).to_string(), "Gameplay"),
        ("bEnableDeathSystem", bstr(cfg.death_system).to_string(), "Gameplay"),
        ("bEnableXpSync", bstr(cfg.xp_sync).to_string(), "Gameplay"),
        ("bEnableItemDrops", bstr(cfg.item_drops).to_string(), "Gameplay"),
        ("bAutoPartyJoin", bstr(cfg.auto_party_join).to_string(), "Gameplay"),
        ("uDifficulty", cfg.difficulty.to_string(), "Gameplay"),
    ];

    let mut lines: Vec<String> = std::fs::read_to_string(&path)
        .unwrap_or_default()
        .lines()
        .map(str::to_string)
        .collect();

    let mut handled = std::collections::HashSet::new();
    for line in lines.iter_mut() {
        let key = line.trim_start().split_once('=').map(|(k, _)| k.trim().to_string());
        if let Some(key) = key {
            if let Some((_, val, _)) = updates.iter().find(|(uk, _, _)| *uk == key) {
                *line = format!("{key}={val}");
                handled.insert(key);
            }
        }
    }
    for (key, val, section) in updates {
        if handled.contains(*key) {
            continue;
        }
        let header = format!("[{section}]");
        match lines.iter().position(|l| l.trim() == header) {
            Some(idx) => lines.insert(idx + 1, format!("{key}={val}")),
            None => {
                lines.push(String::new());
                lines.push(header);
                lines.push(format!("{key}={val}"));
            }
        }
    }

    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(&path, lines.join("\r\n") + "\r\n").map_err(|e| crate::Error::io(&path, e))
}

/// Path to the Together dedicated-server executable (for firewall rules etc.).
pub fn server_exe_path(install_dir: &Path) -> PathBuf {
    server_exe(install_dir)
}

/// Launch the Together dedicated server (the host side); returns its pid.
pub fn launch_server(install_dir: &Path) -> crate::Result<u32> {
    launch_detached(&server_exe(install_dir), &[], Some(&together_dir(install_dir)), &[])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_config_roundtrip_preserves_other_keys() {
        let base = std::env::temp_dir().join(format!("aurora-str-test-{}", std::process::id()));
        let ini = base.join("Data/SkyrimTogetherReborn/config/STServer.ini");
        std::fs::create_dir_all(ini.parent().unwrap()).unwrap();
        std::fs::write(
            &ini,
            "[general]\nsLogLevel=info\n\n[GameServer]\nsServerName=Old\nuPort=10578\nsAdminPassword=keepme\n\n[Gameplay]\nbEnablePvp=false\n",
        )
        .unwrap();

        let mut cfg = read_server_config(&base);
        assert_eq!(cfg.server_name, "Old");
        assert_eq!(cfg.port, 10578);

        cfg.server_name = "New".into();
        cfg.pvp = true;
        cfg.port = 11000;
        write_server_config(&base, &cfg).unwrap();

        let text = std::fs::read_to_string(&ini).unwrap();
        assert!(text.contains("sServerName=New"));
        assert!(text.contains("bEnablePvp=true"));
        assert!(text.contains("uPort=11000"));
        assert!(text.contains("sAdminPassword=keepme")); // untouched key preserved
        assert!(text.contains("sLogLevel=info")); // unrelated section preserved

        let reread = read_server_config(&base);
        assert_eq!(reread.server_name, "New");
        assert!(reread.pvp);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn detect_is_safe_when_not_installed() {
        // We can't assume Skyrim is present in CI; detect must not panic and
        // should report a coherent "not installed" state on this machine if
        // absent.
        let info = detect();
        if !info.installed {
            assert!(info.install_dir.is_none());
            assert!(!info.has_skyrim_together);
        }
    }
}
