//! Multi-game support beyond Minecraft.
//!
//! Minecraft is *installed* by us (see the rest of the crate). Skyrim and Elden
//! Ring are Steam-owned games we only *detect, configure, and launch* — plus
//! wire up their popular co-op mods (Skyrim Together Reborn, Elden Ring
//! Seamless Co-op).
//!
//! Each game module exposes a `detect()` returning a serialisable status the UI
//! can render (installed? co-op mod present? where?) and `launch_*` helpers.

pub mod eldenring;
pub mod skyrim;

use std::path::Path;

use serde::{Deserialize, Serialize};

/// The games this launcher knows about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GameId {
    Minecraft,
    SkyrimSe,
    EldenRing,
}

impl GameId {
    /// Steam app id, for the games distributed via Steam.
    pub fn steam_app_id(self) -> Option<u32> {
        match self {
            GameId::SkyrimSe => Some(489830),
            GameId::EldenRing => Some(1245620),
            GameId::Minecraft => None,
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            GameId::Minecraft => "Minecraft",
            GameId::SkyrimSe => "Skyrim Special Edition",
            GameId::EldenRing => "Elden Ring",
        }
    }
}

/// Launch a native executable detached from the launcher and return its pid.
///
/// The working directory defaults to the executable's own folder (games
/// generally require this), and the child is not killed when the handle drops.
pub fn launch_detached(
    exe: &Path,
    args: &[String],
    cwd: Option<&Path>,
    envs: &[(String, String)],
) -> crate::Result<u32> {
    if !exe.exists() {
        return Err(crate::Error::other(format!(
            "executable not found: {}",
            exe.display()
        )));
    }
    let mut cmd = std::process::Command::new(exe);
    cmd.args(args);
    match cwd {
        Some(dir) => {
            cmd.current_dir(dir);
        }
        None => {
            if let Some(parent) = exe.parent() {
                cmd.current_dir(parent);
            }
        }
    }
    for (key, value) in envs {
        cmd.env(key, value);
    }
    let child = cmd.spawn().map_err(|e| crate::Error::io(exe, e))?;
    Ok(child.id())
}

/// Build a `steam://rungameid/<id>` URL to launch a title through Steam (so
/// Steam overlay, cloud saves, and anti-cheat behave normally).
pub fn steam_run_url(app_id: u32) -> String {
    format!("steam://rungameid/{app_id}")
}
