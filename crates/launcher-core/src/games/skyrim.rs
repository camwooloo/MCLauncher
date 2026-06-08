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

/// Detected state of the Skyrim install.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkyrimInfo {
    pub installed: bool,
    pub install_dir: Option<PathBuf>,
    /// `skse64_loader.exe` present (modded launch available).
    pub has_skse: bool,
    /// Skyrim Together Reborn loader present.
    pub has_skyrim_together: bool,
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
const STR_CANDIDATES: &[&str] = &[
    "SkyrimTogetherReborn.exe",
    "SkyrimTogetherReborn/SkyrimTogetherReborn.exe",
    "Data/SkyrimTogetherReborn/SkyrimTogetherReborn.exe",
];

/// Detect the Skyrim SE install and its co-op tooling.
pub fn detect() -> SkyrimInfo {
    let install_dir = steam::find_app_install_dir(APP_ID);
    let has_skse = install_dir
        .as_ref()
        .map(|d| d.join("skse64_loader.exe").exists())
        .unwrap_or(false);
    let skyrim_together_path = install_dir.as_ref().and_then(|d| find_skyrim_together(d));

    SkyrimInfo {
        installed: install_dir.is_some(),
        has_skse,
        has_skyrim_together: skyrim_together_path.is_some(),
        skyrim_together_path,
        install_dir,
    }
}

fn find_skyrim_together(install_dir: &Path) -> Option<PathBuf> {
    STR_CANDIDATES
        .iter()
        .map(|rel| install_dir.join(rel))
        .find(|p| p.exists())
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
        SkyrimLaunch::SkyrimTogether => info
            .skyrim_together_path
            .clone()
            .ok_or_else(|| crate::Error::other("Skyrim Together Reborn is not installed"))?,
    };

    launch_detached(&exe, &[], Some(dir), &[])
}

#[cfg(test)]
mod tests {
    use super::*;

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
