//! Cross-platform on-disk layout.
//!
//! Minecraft's data directory differs per OS by convention:
//!
//! | OS      | Classic `.minecraft` location                      |
//! |---------|----------------------------------------------------|
//! | Windows | `%APPDATA%\.minecraft`                             |
//! | macOS   | `~/Library/Application Support/minecraft`          |
//! | Linux   | `~/.minecraft`                                      |
//!
//! We keep our launcher's own data (this launcher's config, downloaded Java
//! runtimes, account tokens) in a dedicated `MCLauncher` directory so we never
//! clobber an existing vanilla install, but we lay the *game* directory out
//! exactly like the official launcher so worlds/resource packs are portable.

use std::path::{Path, PathBuf};

/// All directories the launcher reads from / writes to.
///
/// A `Paths` is cheap to clone (it's just owned `PathBuf`s) and is passed
/// through the download and launch subsystems.
#[derive(Debug, Clone)]
pub struct Paths {
    /// The game directory — the `.minecraft` equivalent. Contains
    /// `versions/`, `libraries/`, `assets/`, plus runtime data like `saves/`.
    pub game_dir: PathBuf,
    /// Launcher-private directory: downloaded Java runtimes, account store,
    /// launcher settings. Kept separate from the game dir.
    pub data_dir: PathBuf,
}

impl Paths {
    /// Build paths using the platform-conventional locations.
    pub fn discover() -> crate::Result<Self> {
        let game_dir = default_game_dir()?;
        let data_dir = default_data_dir()?;
        Ok(Self { game_dir, data_dir })
    }

    /// Build paths rooted at explicit directories (useful for tests and for a
    /// "portable" install mode).
    pub fn with_dirs(game_dir: impl Into<PathBuf>, data_dir: impl Into<PathBuf>) -> Self {
        Self {
            game_dir: game_dir.into(),
            data_dir: data_dir.into(),
        }
    }

    // --- Game directory layout -------------------------------------------

    pub fn versions_dir(&self) -> PathBuf {
        self.game_dir.join("versions")
    }

    /// `versions/<id>/`
    pub fn version_dir(&self, id: &str) -> PathBuf {
        self.versions_dir().join(id)
    }

    /// `versions/<id>/<id>.json`
    pub fn version_json(&self, id: &str) -> PathBuf {
        self.version_dir(id).join(format!("{id}.json"))
    }

    /// `versions/<id>/<id>.jar`
    pub fn version_jar(&self, id: &str) -> PathBuf {
        self.version_dir(id).join(format!("{id}.jar"))
    }

    /// Where extracted native libraries for a version go.
    pub fn natives_dir(&self, id: &str) -> PathBuf {
        self.version_dir(id).join("natives")
    }

    pub fn libraries_dir(&self) -> PathBuf {
        self.game_dir.join("libraries")
    }

    /// A library's absolute path given its Maven-style relative path
    /// (e.g. `com/mojang/blocklist/1.0.10/blocklist-1.0.10.jar`).
    pub fn library_path(&self, relative: &str) -> PathBuf {
        join_relative(&self.libraries_dir(), relative)
    }

    pub fn assets_dir(&self) -> PathBuf {
        self.game_dir.join("assets")
    }

    pub fn asset_indexes_dir(&self) -> PathBuf {
        self.assets_dir().join("indexes")
    }

    /// `assets/indexes/<id>.json`
    pub fn asset_index_json(&self, id: &str) -> PathBuf {
        self.asset_indexes_dir().join(format!("{id}.json"))
    }

    pub fn asset_objects_dir(&self) -> PathBuf {
        self.assets_dir().join("objects")
    }

    /// An asset object lives at `assets/objects/<first2>/<full-hash>`.
    pub fn asset_object_path(&self, hash: &str) -> PathBuf {
        self.asset_objects_dir().join(&hash[..2]).join(hash)
    }

    /// Legacy/virtual assets (pre-1.7 layouts) are mirrored here by name.
    pub fn asset_virtual_dir(&self, index_id: &str) -> PathBuf {
        self.assets_dir().join("virtual").join(index_id)
    }

    // --- Launcher-private layout -----------------------------------------

    /// Root for downloaded Java runtimes.
    pub fn java_dir(&self) -> PathBuf {
        self.data_dir.join("java")
    }

    /// Persisted account/token store.
    pub fn accounts_file(&self) -> PathBuf {
        self.data_dir.join("accounts.json")
    }

    /// Launcher settings (memory, selected java, etc.).
    pub fn settings_file(&self) -> PathBuf {
        self.data_dir.join("settings.json")
    }
}

/// Join a forward-slash- or backslash-separated relative path onto a base,
/// normalising separators for the current platform.
fn join_relative(base: &Path, relative: &str) -> PathBuf {
    let mut out = base.to_path_buf();
    for part in relative.split(['/', '\\']).filter(|p| !p.is_empty()) {
        out.push(part);
    }
    out
}

fn default_game_dir() -> crate::Result<PathBuf> {
    // The canonical per-OS game directory.
    #[cfg(target_os = "windows")]
    {
        let appdata = dirs::config_dir()
            .ok_or_else(|| crate::Error::other("could not resolve %APPDATA%"))?;
        Ok(appdata.join(".minecraft"))
    }
    #[cfg(target_os = "macos")]
    {
        let support = dirs::data_dir()
            .ok_or_else(|| crate::Error::other("could not resolve Application Support"))?;
        Ok(support.join("minecraft"))
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let home = dirs::home_dir()
            .ok_or_else(|| crate::Error::other("could not resolve home directory"))?;
        Ok(home.join(".minecraft"))
    }
}

fn default_data_dir() -> crate::Result<PathBuf> {
    let base = dirs::data_dir()
        .ok_or_else(|| crate::Error::other("could not resolve a data directory"))?;
    Ok(base.join("MCLauncher"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_object_is_sharded_by_hash_prefix() {
        let p = Paths::with_dirs("/game", "/data");
        let path = p.asset_object_path("abcdef1234567890");
        assert!(path.ends_with("objects/ab/abcdef1234567890") || path.ends_with("objects\\ab\\abcdef1234567890"));
    }

    #[test]
    fn library_path_normalises_separators() {
        let p = Paths::with_dirs("/game", "/data");
        let path = p.library_path("com/mojang/blocklist/1.0.10/blocklist-1.0.10.jar");
        assert!(path.ends_with("blocklist-1.0.10.jar"));
        assert!(path.starts_with("/game"));
    }
}
