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
}

/// Which way to launch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EldenRingLaunch {
    /// Official game with EasyAntiCheat (online play).
    Vanilla,
    /// Seamless Co-op (EAC bypassed, drop-in co-op).
    SeamlessCoop,
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

    EldenRingInfo {
        installed: install_dir.is_some(),
        has_seamless_coop: seamless_launcher_path.is_some(),
        seamless_launcher_path,
        coop_password,
        game_dir,
        install_dir,
    }
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

    let exe = match mode {
        EldenRingLaunch::Vanilla => game_dir.join("start_protected_game.exe"),
        EldenRingLaunch::SeamlessCoop => info
            .seamless_launcher_path
            .clone()
            .ok_or_else(|| crate::Error::other("Seamless Co-op is not installed"))?,
    };

    launch_detached(&exe, &[], Some(game_dir), &[])
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
