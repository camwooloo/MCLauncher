//! Epic Games Store install detection.
//!
//! The Epic launcher records every install as a JSON "manifest" in
//! `%ProgramData%\Epic\EpicGamesLauncher\Data\Manifests\*.item` with the
//! game's `DisplayName` and `InstallLocation`. We match by display name —
//! no Epic login or API needed.

use std::path::PathBuf;

fn manifests_dir() -> Option<PathBuf> {
    let base = std::env::var_os("ProgramData")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\ProgramData"));
    let dir = base
        .join("Epic")
        .join("EpicGamesLauncher")
        .join("Data")
        .join("Manifests");
    dir.is_dir().then_some(dir)
}

/// Find an Epic-installed game whose display name contains `name_contains`
/// (case-insensitive). Returns its install directory if it still exists.
pub fn find_install(name_contains: &str) -> Option<PathBuf> {
    let needle = name_contains.to_lowercase();
    for entry in std::fs::read_dir(manifests_dir()?).ok()?.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("item") {
            continue;
        }
        let Ok(bytes) = std::fs::read(&path) else { continue };
        let Ok(json) = serde_json::from_slice::<serde_json::Value>(&bytes) else { continue };
        let display = json["DisplayName"].as_str().unwrap_or("").to_lowercase();
        if !display.contains(&needle) {
            continue;
        }
        if let Some(loc) = json["InstallLocation"].as_str() {
            let dir = PathBuf::from(loc);
            if dir.is_dir() {
                return Some(dir);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_install_is_safe_without_epic() {
        // Must not panic whether or not Epic is installed on this machine.
        let _ = find_install("definitely-not-a-real-game-xyz");
    }
}
