//! Persisted launcher settings, stored in the launcher data dir.

use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    /// Maximum JVM heap in MiB (Minecraft).
    pub max_memory_mb: u32,
    /// Last selected loader ("vanilla" / "fabric" / "quilt").
    pub last_loader: String,
    /// Last selected Minecraft version.
    pub last_version: String,
    /// UI theme: "dark" | "light".
    pub theme: String,
    /// UI style/look: "aurora" | "liquidglass".
    pub ui_style: String,
    /// Background mode: "static" | "pulsing".
    pub background: String,
    /// Tailscale API access token for *hosting* on Aurora Net (minting guest
    /// keys + access rules). Stored locally only; never committed.
    #[serde(default)]
    pub tailscale_api_token: String,
    /// Show "Playing … via Aurora Launcher" in Discord.
    #[serde(default = "default_true")]
    pub discord_rpc: bool,
    /// Which view the launcher opens to: "home", a section ("network",
    /// "settings", a game key), or "<game>:<tab>" (e.g. "minecraft:Servers").
    #[serde(default = "default_view")]
    pub default_view: String,
    /// Start Aurora automatically when Windows boots.
    #[serde(default)]
    pub launch_at_login: bool,
    /// Start hidden to the system tray (used with launch-at-login).
    #[serde(default)]
    pub start_minimized: bool,
    /// Closing the window hides to tray (keeps servers running) instead of quitting.
    #[serde(default = "default_true")]
    pub close_to_tray: bool,
    /// Whether the first-run onboarding has been completed/skipped.
    #[serde(default)]
    pub onboarded: bool,
}

fn default_true() -> bool {
    true
}
fn default_view() -> String {
    "home".to_string()
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            max_memory_mb: 4096,
            last_loader: "vanilla".to_string(),
            last_version: String::new(),
            theme: "dark".to_string(),
            ui_style: "aurora".to_string(),
            background: "liquid".to_string(),
            tailscale_api_token: String::new(),
            discord_rpc: true,
            default_view: "home".to_string(),
            launch_at_login: false,
            start_minimized: false,
            close_to_tray: true,
            onboarded: false,
        }
    }
}

impl Settings {
    pub async fn load(path: &Path) -> Self {
        match tokio::fs::read(path).await {
            Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub async fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        tokio::fs::write(path, bytes)
            .await
            .map_err(|e| e.to_string())
    }
}
