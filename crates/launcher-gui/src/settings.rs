//! Persisted launcher settings (memory, Azure client id, last selection).

use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Maximum JVM heap in MiB.
    pub max_memory_mb: u32,
    /// Azure AD application (client) id for Microsoft login. Empty until the
    /// user supplies one.
    pub azure_client_id: String,
    /// Last selected loader ("Vanilla" / "Fabric" / "Quilt").
    pub last_loader: String,
    /// Last selected game version.
    pub last_version: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            max_memory_mb: 2048,
            azure_client_id: String::new(),
            last_loader: "Vanilla".to_string(),
            last_version: String::new(),
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

    pub async fn save(&self, path: &Path) {
        if let Some(parent) = path.parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }
        if let Ok(bytes) = serde_json::to_vec_pretty(self) {
            let _ = tokio::fs::write(path, bytes).await;
        }
    }
}
