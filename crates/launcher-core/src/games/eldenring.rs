//! Elden Ring detection & launch, including the Seamless Co-op mod.
//!
//! Elden Ring installs to `…/ELDEN RING/Game/`, where `start_protected_game.exe`
//! boots the game *with* EasyAntiCheat (required for official online play).
//!
//! The **Seamless Co-op** mod enables drop-in co-op without the session limits
//! of the official multiplayer. It must run with EAC **disabled**, which is why
//! it ships its own loader (`ersc_launcher.exe`) that starts `eldenring.exe`
//! directly with the mod injected. Its co-op password lives in
//! `Game/SeamlessCoop/ersc_settings.ini` under `[PASSWORD] cooppassword`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::games::launch_detached;
use crate::steam;

/// Elden Ring Steam app id.
pub const APP_ID: u32 = 1245620;

/// GitHub repo distributing Seamless Co-op release zips.
pub const SEAMLESS_REPO: &str = "LukeYui/EldenRingSeamlessCoopRelease";
/// GitHub repo distributing Mod Engine 2 (general ER mod loader).
pub const MOD_ENGINE_REPO: &str = "soulsmods/ModEngine2";

/// Detected state of the Elden Ring install.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EldenRingInfo {
    pub installed: bool,
    /// Root install dir (`…/ELDEN RING`).
    pub install_dir: Option<PathBuf>,
    /// The `Game` subfolder containing the executables.
    pub game_dir: Option<PathBuf>,
    /// Seamless Co-op loader present.
    pub has_seamless_coop: bool,
    pub seamless_launcher_path: Option<PathBuf>,
    /// Current co-op password from the mod settings, if readable.
    pub coop_password: Option<String>,
    /// Mod Engine 2 installed (modded launch available).
    pub has_mod_engine: bool,
    /// Folder where Mod Engine 2 loads mods from (`ModEngine2/mod`).
    pub mods_dir: Option<PathBuf>,
    /// Ultrawide fix (Elden Mod Loader `dinput8.dll`) is present, enabled or not.
    pub ultrawide_installed: bool,
    /// Ultrawide fix present *and* active (loader not renamed off).
    pub ultrawide_enabled: bool,
}

/// Which way to launch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EldenRingLaunch {
    /// Official game with EasyAntiCheat (online play).
    Vanilla,
    /// Seamless Co-op (EAC bypassed, drop-in co-op).
    SeamlessCoop,
    /// Mod Engine 2 (offline modded play, EAC off).
    Modded,
}

/// Where Mod Engine 2 lives, relative to the install root.
pub fn mod_engine_dir(install_dir: &Path) -> PathBuf {
    install_dir.join("ModEngine2")
}

/// Detect the Elden Ring install and Seamless Co-op tooling.
pub fn detect() -> EldenRingInfo {
    let install_dir = steam::find_app_install_dir(APP_ID);
    let game_dir = install_dir.as_ref().map(|d| d.join("Game"));

    let seamless_launcher_path = game_dir.as_ref().and_then(|g| {
        let p = g.join("ersc_launcher.exe");
        p.exists().then_some(p)
    });
    let coop_password = game_dir.as_ref().and_then(|g| read_coop_password(g).ok().flatten());

    let me2 = install_dir.as_ref().map(|d| mod_engine_dir(d));
    let has_mod_engine = me2
        .as_ref()
        .map(|d| d.join("modengine2_launcher.exe").exists())
        .unwrap_or(false);
    let mods_dir = me2.filter(|_| has_mod_engine).map(|d| d.join("mod"));

    // The ultrawide fix ships the Elden Mod Loader (`dinput8.dll`). We disable
    // it by renaming to `.off`, so "installed" means either name exists.
    let uw_on = game_dir.as_ref().map(|g| g.join("dinput8.dll").exists()).unwrap_or(false);
    let uw_off = game_dir.as_ref().map(|g| g.join("dinput8.dll.off").exists()).unwrap_or(false);

    EldenRingInfo {
        installed: install_dir.is_some(),
        has_seamless_coop: seamless_launcher_path.is_some(),
        seamless_launcher_path,
        coop_password,
        has_mod_engine,
        mods_dir,
        ultrawide_installed: uw_on || uw_off,
        ultrawide_enabled: uw_on,
        game_dir,
        install_dir,
    }
}

/// Nexus page for the Elden Ring Ultrawide Fix (its files are Nexus-only).
pub const ULTRAWIDE_NEXUS_URL: &str = "https://www.nexusmods.com/eldenring/mods/7870?tab=files";

/// Newest ultrawide-fix zip in Downloads.
pub fn find_downloaded_ultrawide_zip() -> Option<PathBuf> {
    crate::games::install::find_downloaded_zip(|n| {
        n.contains("ultrawide") || n.contains("ultra-wide") || (n.contains("elden") && n.contains("wide"))
    })
}

/// Install the Ultrawide Fix from a downloaded zip: locate the Elden Mod Loader
/// (`dinput8.dll`) and merge its folder into `Game/` (next to the executable).
pub fn install_ultrawide_from_zip(game_dir: &Path, zip: &Path) -> crate::Result<()> {
    use crate::games::install::{extract_archive_into, find_file, merge_move};

    let staging = game_dir.join(".aurora-uw-extract");
    let _ = std::fs::remove_dir_all(&staging);
    extract_archive_into(zip, &staging)?;
    let dll = find_file(&staging, "dinput8.dll").ok_or_else(|| {
        let _ = std::fs::remove_dir_all(&staging);
        crate::Error::other(
            "That zip doesn't look like the Ultrawide Fix (no dinput8.dll/mod loader) — download the main file from the Nexus page",
        )
    })?;
    let src = dll.parent().unwrap_or(&staging).to_path_buf();
    merge_move(&src, game_dir)?;
    let _ = std::fs::remove_dir_all(&staging);
    Ok(())
}

/// Enable/disable the ultrawide fix by renaming its loader DLL (`dinput8.dll`
/// ⇄ `dinput8.dll.off`) — non-destructive, instant toggle.
pub fn set_ultrawide(game_dir: &Path, enabled: bool) -> crate::Result<()> {
    let on = game_dir.join("dinput8.dll");
    let off = game_dir.join("dinput8.dll.off");
    if enabled {
        if off.exists() && !on.exists() {
            std::fs::rename(&off, &on).map_err(|e| crate::Error::io(&on, e))?;
        }
    } else if on.exists() {
        let _ = std::fs::remove_file(&off);
        std::fs::rename(&on, &off).map_err(|e| crate::Error::io(&off, e))?;
    }
    Ok(())
}

/// One-click install of the latest Seamless Co-op release into `Game/`
/// (`ersc_launcher.exe` + `SeamlessCoop/`). Returns the release tag.
pub async fn install_seamless(
    game_dir: &Path,
    reporter: &crate::progress::SharedReporter,
) -> crate::Result<String> {
    crate::games::install::install_github_archive(
        SEAMLESS_REPO,
        |n| n.ends_with(".zip"),
        game_dir,
        false,
        reporter,
    )
    .await
}

/// Nexus page for Seamless Co-op — updates land here before the GitHub
/// releases, so when the mod's self-check says "out of date" the guided
/// install path uses the Nexus zip.
pub const SEAMLESS_NEXUS_URL: &str = "https://www.nexusmods.com/eldenring/mods/510?tab=files";

/// Install/update Seamless Co-op from a zip the user downloaded (Nexus).
/// Locates `ersc_launcher.exe` regardless of wrapper folders and merges its
/// folder (launcher + `SeamlessCoop/`) into `Game/`.
pub fn install_seamless_from_zip(game_dir: &Path, zip: &Path) -> crate::Result<()> {
    use crate::games::install::{extract_archive_into, find_file, merge_move};

    let staging = game_dir.join(".aurora-ersc-extract");
    let _ = std::fs::remove_dir_all(&staging);
    extract_archive_into(zip, &staging)?;

    let exe = find_file(&staging, "ersc_launcher.exe").ok_or_else(|| {
        let _ = std::fs::remove_dir_all(&staging);
        crate::Error::other(
            "That zip doesn't contain ersc_launcher.exe — download the main Seamless Co-op file from the Nexus page",
        )
    })?;
    let src = exe.parent().unwrap_or(&staging).to_path_buf();
    merge_move(&src, game_dir)?;
    let _ = std::fs::remove_dir_all(&staging);
    Ok(())
}

/// Newest Seamless Co-op zip in Downloads.
pub fn find_downloaded_seamless_zip() -> Option<PathBuf> {
    crate::games::install::find_downloaded_zip(|n| n.contains("seamless") || n.starts_with("ersc"))
}

/// One-click install of Mod Engine 2 into `<install>/ModEngine2/`
/// (wrapper folder stripped). Returns the release tag.
pub async fn install_mod_engine(
    install_dir: &Path,
    reporter: &crate::progress::SharedReporter,
) -> crate::Result<String> {
    let dest = mod_engine_dir(install_dir);
    let tag = crate::games::install::install_github_archive(
        MOD_ENGINE_REPO,
        |n| n.ends_with("win64.zip"),
        &dest,
        true,
        reporter,
    )
    .await?;
    let _ = std::fs::create_dir_all(dest.join("mod"));
    Ok(tag)
}

/// Launch Elden Ring in the requested mode; returns the child pid.
///
/// For [`EldenRingLaunch::Vanilla`], prefer launching through Steam
/// ([`crate::games::steam_run_url`]) so EAC and online services initialise
/// correctly; this direct path is a fallback.
pub fn launch(info: &EldenRingInfo, mode: EldenRingLaunch) -> crate::Result<u32> {
    let game_dir = info
        .game_dir
        .as_ref()
        .ok_or_else(|| crate::Error::other("Elden Ring is not installed"))?;

    let (exe, args, cwd) = match mode {
        EldenRingLaunch::Vanilla => (game_dir.join("start_protected_game.exe"), vec![], game_dir.clone()),
        EldenRingLaunch::SeamlessCoop => (
            info.seamless_launcher_path
                .clone()
                .ok_or_else(|| crate::Error::other("Seamless Co-op is not installed"))?,
            vec![],
            game_dir.clone(),
        ),
        EldenRingLaunch::Modded => {
            let install = info
                .install_dir
                .as_ref()
                .ok_or_else(|| crate::Error::other("Elden Ring is not installed"))?;
            let me2 = mod_engine_dir(install);
            (
                me2.join("modengine2_launcher.exe"),
                vec![
                    "-t".to_string(),
                    "er".to_string(),
                    "-c".to_string(),
                    "config_eldenring.toml".to_string(),
                ],
                me2,
            )
        }
    };

    launch_detached(&exe, &args, Some(&cwd), &[])
}

fn settings_path(game_dir: &Path) -> PathBuf {
    game_dir.join("SeamlessCoop").join("ersc_settings.ini")
}

/// Read the Seamless Co-op password from `ersc_settings.ini`.
pub fn read_coop_password(game_dir: &Path) -> crate::Result<Option<String>> {
    let path = settings_path(game_dir);
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return Ok(None),
    };
    Ok(parse_ini_value(&content, "cooppassword"))
}

/// Write the Seamless Co-op password into `ersc_settings.ini`, preserving the
/// rest of the file.
pub fn set_coop_password(game_dir: &Path, password: &str) -> crate::Result<()> {
    let path = settings_path(game_dir);
    let content = std::fs::read_to_string(&path).map_err(|e| crate::Error::io(&path, e))?;
    let updated = replace_ini_value(&content, "cooppassword", password);
    std::fs::write(&path, updated).map_err(|e| crate::Error::io(&path, e))?;
    Ok(())
}

/// Find `key = value` (ignoring surrounding whitespace, skipping comments).
fn parse_ini_value(content: &str, key: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with(';') || trimmed.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = trimmed.split_once('=') {
            if k.trim().eq_ignore_ascii_case(key) {
                return Some(v.trim().to_string());
            }
        }
    }
    None
}

/// Replace the value for `key`, keeping original formatting where possible.
fn replace_ini_value(content: &str, key: &str, value: &str) -> String {
    let mut out = Vec::new();
    let mut replaced = false;
    for line in content.lines() {
        let trimmed = line.trim_start();
        if !trimmed.starts_with(';')
            && !trimmed.starts_with('#')
            && trimmed
                .split_once('=')
                .map(|(k, _)| k.trim().eq_ignore_ascii_case(key))
                .unwrap_or(false)
        {
            out.push(format!("{key} = {value}"));
            replaced = true;
        } else {
            out.push(line.to_string());
        }
    }
    if !replaced {
        out.push(format!("{key} = {value}"));
    }
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_replaces_password() {
        let ini = "[PASSWORD]\n; the co-op password\ncooppassword = oldpass\n[GAMEPLAY]\nallow_invaders = 1\n";
        assert_eq!(parse_ini_value(ini, "cooppassword"), Some("oldpass".into()));
        let updated = replace_ini_value(ini, "cooppassword", "newpass");
        assert_eq!(parse_ini_value(&updated, "cooppassword"), Some("newpass".into()));
        assert!(updated.contains("allow_invaders = 1"));
    }

    #[test]
    fn detect_is_safe_when_not_installed() {
        let info = detect();
        if !info.installed {
            assert!(info.game_dir.is_none());
        }
    }
}
