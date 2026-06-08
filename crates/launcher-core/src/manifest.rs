//! The global version manifest (`version_manifest_v2.json`).
//!
//! This is the launcher's index of every published Minecraft version. Each
//! entry points at a per-version JSON (see [`crate::version`]) by URL, with a
//! SHA-1 so we can cache it safely.

use serde::Deserialize;

/// Mojang's piston-meta endpoint for the v2 manifest.
pub const MANIFEST_URL: &str =
    "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

#[derive(Debug, Clone, Deserialize)]
pub struct VersionManifest {
    pub latest: LatestVersions,
    pub versions: Vec<VersionEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LatestVersions {
    pub release: String,
    pub snapshot: String,
}

/// One version's entry in the manifest.
#[derive(Debug, Clone, Deserialize)]
pub struct VersionEntry {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: VersionKind,
    /// URL of the per-version JSON.
    pub url: String,
    /// SHA-1 of the per-version JSON (lets us trust a cached copy).
    pub sha1: String,
    #[serde(rename = "releaseTime")]
    pub release_time: String,
    pub time: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VersionKind {
    Release,
    Snapshot,
    OldBeta,
    OldAlpha,
}

impl VersionManifest {
    /// Fetch and parse the manifest from Mojang.
    pub async fn fetch() -> crate::Result<Self> {
        let manifest = crate::http::client()
            .get(MANIFEST_URL)
            .send()
            .await?
            .error_for_status()?
            .json::<VersionManifest>()
            .await?;
        Ok(manifest)
    }

    /// Look up a version entry by id.
    pub fn find(&self, id: &str) -> Option<&VersionEntry> {
        self.versions.iter().find(|v| v.id == id)
    }

    /// The entry for the latest stable release.
    pub fn latest_release(&self) -> Option<&VersionEntry> {
        self.find(&self.latest.release)
    }

    /// Iterator over only release (non-snapshot) versions, newest first —
    /// the manifest is already ordered newest-first.
    pub fn releases(&self) -> impl Iterator<Item = &VersionEntry> {
        self.versions
            .iter()
            .filter(|v| v.kind == VersionKind::Release)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_manifest_shape() {
        let json = r#"{
            "latest": {"release": "1.21", "snapshot": "24w20a"},
            "versions": [
                {"id":"1.21","type":"release","url":"https://example/1.21.json","sha1":"abc","releaseTime":"2024-06-13T08:32:38+00:00","time":"2024-06-13T08:32:38+00:00"},
                {"id":"24w20a","type":"snapshot","url":"https://example/24w20a.json","sha1":"def","releaseTime":"2024-05-15T13:21:46+00:00","time":"2024-05-15T13:21:46+00:00"}
            ]
        }"#;
        let m: VersionManifest = serde_json::from_str(json).unwrap();
        assert_eq!(m.latest.release, "1.21");
        assert_eq!(m.releases().count(), 1);
        assert_eq!(m.find("24w20a").unwrap().kind, VersionKind::Snapshot);
    }
}
