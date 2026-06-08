//! Asset index handling.
//!
//! A version references an *asset index* (e.g. "1.21"), a JSON mapping logical
//! names like `minecraft/sounds/...` to content-addressed objects. Objects live
//! in a shared, de-duplicated store at `assets/objects/<first-2-hex>/<hash>`,
//! downloaded from Mojang's resources CDN.
//!
//! Two legacy layouts exist:
//! * `virtual` indexes (pre-1.7) expect files laid out by name under
//!   `assets/virtual/<index>/`.
//! * `map_to_resources` indexes (pre-1.6) expect them under the game dir's
//!   `resources/` folder.
//!
//! Both are materialised by copying from the objects store.

use std::collections::HashMap;

use serde::Deserialize;

use crate::download::Download;
use crate::paths::Paths;

/// Mojang's content-addressed asset CDN.
pub const RESOURCES_BASE: &str = "https://resources.download.minecraft.net";

#[derive(Debug, Clone, Deserialize)]
pub struct AssetIndex {
    #[serde(default)]
    pub objects: HashMap<String, AssetObject>,
    /// Pre-1.6 layout flag.
    #[serde(default)]
    pub map_to_resources: bool,
    /// Pre-1.7 layout flag.
    #[serde(default, rename = "virtual")]
    pub is_virtual: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AssetObject {
    pub hash: String,
    pub size: u64,
}

impl AssetObject {
    /// CDN URL for this object.
    pub fn url(&self) -> String {
        format!("{}/{}/{}", RESOURCES_BASE, &self.hash[..2], self.hash)
    }
}

impl AssetIndex {
    pub fn parse(s: &str) -> crate::Result<Self> {
        Ok(serde_json::from_str(s)?)
    }

    /// Build the download list for every object into the shared objects store.
    pub fn object_downloads(&self, paths: &Paths) -> Vec<Download> {
        self.objects
            .values()
            .map(|obj| {
                Download::new(obj.url(), paths.asset_object_path(&obj.hash))
                    .sha1(obj.hash.clone())
                    .size(obj.size)
            })
            .collect()
    }

    /// Total size of all objects (for progress display).
    pub fn total_size(&self) -> u64 {
        self.objects.values().map(|o| o.size).sum()
    }

    /// Whether this index needs files materialised outside the objects store.
    pub fn needs_materialization(&self) -> bool {
        self.is_virtual || self.map_to_resources
    }

    /// Materialise legacy/virtual assets by copying objects to their named
    /// locations. `index_id` is the asset index name; `game_dir` is needed for
    /// the `map_to_resources` case.
    pub async fn materialize(&self, paths: &Paths, index_id: &str) -> crate::Result<()> {
        if !self.needs_materialization() {
            return Ok(());
        }

        let dest_root = if self.map_to_resources {
            paths.game_dir.join("resources")
        } else {
            paths.asset_virtual_dir(index_id)
        };

        for (name, obj) in &self.objects {
            let src = paths.asset_object_path(&obj.hash);
            let dest = dest_root.join(name);
            crate::util::ensure_parent(&dest).await?;
            // Copy only if missing/changed to keep re-installs cheap.
            let needs_copy = match tokio::fs::metadata(&dest).await {
                Ok(m) => m.len() != obj.size,
                Err(_) => true,
            };
            if needs_copy {
                tokio::fs::copy(&src, &dest)
                    .await
                    .map_err(|e| crate::Error::io(&dest, e))?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_url_is_sharded() {
        let obj = AssetObject {
            hash: "abcd1234".into(),
            size: 10,
        };
        assert_eq!(
            obj.url(),
            "https://resources.download.minecraft.net/ab/abcd1234"
        );
    }

    #[test]
    fn parses_index() {
        let json = r#"{"objects":{"minecraft/sounds/x.ogg":{"hash":"deadbeef00","size":42}}}"#;
        let idx = AssetIndex::parse(json).unwrap();
        assert_eq!(idx.total_size(), 42);
        assert_eq!(idx.object_downloads(&Paths::with_dirs("/g", "/d")).len(), 1);
        assert!(!idx.needs_materialization());
    }
}
