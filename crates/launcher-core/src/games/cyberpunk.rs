//! Cyberpunk 2077 detection & launch, including the CyberpunkMP co-op mod and
//! Cyber Engine Tweaks (the foundation most Cyberpunk mods build on).
//!
//! **CyberpunkMP** (by Tilted Phoques, the Skyrim Together team) is an
//! experimental multiplayer mod distributed as a standalone launcher app on
//! GitHub — we install it to `<install>/CyberpunkMP/` and run its own
//! `CyberpunkMP.exe`, which injects into the game and connects to servers.
//!
//! **Cyber Engine Tweaks** extracts into the game folder itself
//! (`bin/x64/plugins/…`) and gives mods a scripting/console layer.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::games::launch_detached;
use crate::steam;

/// Cyberpunk 2077 Steam app id.
pub const APP_ID: u32 = 1091500;

/// GitHub repo distributing Cyber Engine Tweaks release zips.
pub const CET_REPO: &str = "yamashi/CyberEngineTweaks";
/// GitHub repo distributing the CyberpunkMP multiplayer mod.
pub const MP_REPO: &str = "tiltedphoques/CyberpunkMP";

/// Detected state of the Cyberpunk 2077 install.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CyberpunkInfo {
    pub installed: bool,
    pub install_dir: Option<PathBuf>,
    /// "steam" or "epic", when installed.
    pub source: Option<String>,
    /// Cyber Engine Tweaks present (modding foundation).
    pub has_cet: bool,
    /// CyberpunkMP launcher present (experimental co-op).
    pub has_mp: bool,
    pub mp_path: Option<PathBuf>,
    /// CET's Lua mods folder, once CET is installed.
    pub mods_dir: Option<PathBuf>,
}

/// Which way to launch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CyberpunkLaunch {
    /// Through Steam (REDlauncher, GOG overlay etc. behave normally).
    Vanilla,
    /// Game exe directly, skipping the REDlauncher.
    SkipLauncher,
    /// CyberpunkMP co-op launcher.
    Mp,
}

/// Detect the Cyberpunk install (Steam, falling back to Epic) and its mod
/// tooling.
pub fn detect() -> CyberpunkInfo {
    let (install_dir, source) = match steam::find_app_install_dir(APP_ID) {
        Some(d) => (Some(d), Some("steam".to_string())),
        None => match crate::epic::find_install("cyberpunk 2077") {
            Some(d) => (Some(d), Some("epic".to_string())),
            None => (None, None),
        },
    };

    let plugins = install_dir
        .as_ref()
        .map(|d| d.join("bin").join("x64").join("plugins"));
    let has_cet = plugins
        .as_ref()
        .map(|p| p.join("cyber_engine_tweaks.asi").exists() || p.join("cyber_engine_tweaks").exists())
        .unwrap_or(false);
    let mods_dir = plugins
        .filter(|_| has_cet)
        .map(|p| p.join("cyber_engine_tweaks").join("mods"));

    let mp_path = install_dir.as_ref().and_then(|d| {
        let p = d.join("CyberpunkMP").join("CyberpunkMP.exe");
        p.exists().then_some(p)
    });

    CyberpunkInfo {
        installed: install_dir.is_some(),
        has_cet,
        has_mp: mp_path.is_some(),
        mp_path,
        mods_dir,
        source,
        install_dir,
    }
}

/// Launch Cyberpunk in the requested mode; returns the child pid.
///
/// For [`CyberpunkLaunch::Vanilla`], prefer launching through Steam
/// ([`crate::games::steam_run_url`]); this direct path is a fallback.
pub fn launch(info: &CyberpunkInfo, mode: CyberpunkLaunch) -> crate::Result<u32> {
    let dir = info
        .install_dir
        .as_ref()
        .ok_or_else(|| crate::Error::other("Cyberpunk 2077 is not installed"))?;

    let (exe, args) = match mode {
        CyberpunkLaunch::Vanilla => (dir.join("REDprelauncher.exe"), vec![]),
        CyberpunkLaunch::SkipLauncher => (
            dir.join("bin").join("x64").join("Cyberpunk2077.exe"),
            vec!["--launcher-skip".to_string()],
        ),
        CyberpunkLaunch::Mp => (
            info.mp_path
                .clone()
                .ok_or_else(|| crate::Error::other("CyberpunkMP is not installed"))?,
            vec![],
        ),
    };

    launch_detached(&exe, &args, None, &[])
}

/// One-click install of Cyber Engine Tweaks into the game folder. The release
/// zip already carries the `bin/x64/plugins/…` structure. Returns the tag.
pub async fn install_cet(
    install_dir: &std::path::Path,
    reporter: &crate::progress::SharedReporter,
) -> crate::Result<String> {
    crate::games::install::install_github_archive(
        CET_REPO,
        |n| n.starts_with("cet_") && n.ends_with(".zip"),
        install_dir,
        false,
        reporter,
    )
    .await
}

/// One-click install of CyberpunkMP into `<install>/CyberpunkMP/` (wrapper
/// folder stripped). Returns the release tag.
pub async fn install_mp(
    install_dir: &std::path::Path,
    reporter: &crate::progress::SharedReporter,
) -> crate::Result<String> {
    crate::games::install::install_github_archive(
        MP_REPO,
        |n| n.ends_with(".zip"),
        &install_dir.join("CyberpunkMP"),
        true,
        reporter,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_is_safe_when_not_installed() {
        let info = detect();
        if !info.installed {
            assert!(info.install_dir.is_none());
            assert!(!info.has_mp);
        }
    }
}
