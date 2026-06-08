//! Installation orchestration: turn a version id into an on-disk, launchable
//! install.
//!
//! Responsibilities:
//! 1. **Resolve** the version JSON, following `inheritsFrom` so modloader
//!    profiles compose onto their vanilla base.
//! 2. **Download** the client jar, libraries, native jars, the asset index, and
//!    every asset object — all integrity-checked and parallel.
//! 3. **Extract** native libraries into the version's `natives/` directory.
//! 4. **Materialise** legacy/virtual assets when required.
//!
//! The result is an [`InstalledVersion`] carrying everything the launch step
//! needs (classpath, natives dir, asset index id, main class, …).

use std::path::{Path, PathBuf};

use crate::assets::AssetIndex;
use crate::download::{self, Download, DEFAULT_CONCURRENCY};
use crate::manifest::VersionManifest;
use crate::platform::Environment;
use crate::progress::{self, SharedReporter};
use crate::version::VersionJson;
use crate::{paths::Paths, Error, Result};

/// Drives installation for a given on-disk layout and platform.
#[derive(Debug, Clone)]
pub struct Installer {
    pub paths: Paths,
    pub env: Environment,
    pub concurrency: usize,
}

/// Everything the launcher needs to build a launch command.
#[derive(Debug, Clone)]
pub struct InstalledVersion {
    pub id: String,
    pub version: VersionJson,
    /// Absolute path to the client jar.
    pub jar_path: PathBuf,
    /// Directory holding extracted native libraries.
    pub natives_dir: PathBuf,
    /// Classpath entries in order: libraries first, then the client jar.
    pub classpath: Vec<PathBuf>,
    /// Asset index name (e.g. "1.21").
    pub asset_index_id: String,
}

impl Installer {
    /// Construct an installer for the current platform.
    pub fn new(paths: Paths) -> Self {
        Self {
            paths,
            env: Environment::detect(),
            concurrency: DEFAULT_CONCURRENCY,
        }
    }

    /// Resolve a version JSON by id, following `inheritsFrom`.
    ///
    /// Vanilla versions are fetched (and cached, SHA-1-verified) from the
    /// manifest. Modloader profiles, which aren't in the manifest, must already
    /// have their JSON on disk (written by the modloader installer).
    pub async fn resolve_version(
        &self,
        manifest: &VersionManifest,
        id: &str,
    ) -> Result<VersionJson> {
        let raw = self.load_or_fetch_version_json(manifest, id).await?;
        let version = VersionJson::parse(&raw)?;

        match version.inherits_from.clone() {
            Some(parent_id) => {
                // Box the recursive future (async fn can't be directly
                // self-recursive without indirection).
                let parent =
                    Box::pin(self.resolve_version(manifest, &parent_id)).await?;
                Ok(version.merge_onto_parent(parent))
            }
            None => Ok(version),
        }
    }

    async fn load_or_fetch_version_json(
        &self,
        manifest: &VersionManifest,
        id: &str,
    ) -> Result<String> {
        let path = self.paths.version_json(id);

        if let Some(entry) = manifest.find(id) {
            // Vanilla: ensure a cached, verified copy exists.
            let dl = Download::new(entry.url.clone(), path.clone()).sha1(entry.sha1.clone());
            download::download(&dl, &progress::noop()).await?;
        } else if tokio::fs::try_exists(&path).await.unwrap_or(false) {
            // Modloader profile already on disk — use it as-is.
        } else {
            return Err(Error::VersionNotFound(id.to_string()));
        }

        tokio::fs::read_to_string(&path)
            .await
            .map_err(|e| Error::io(&path, e))
    }

    /// Install everything required to launch `version`.
    pub async fn install(
        &self,
        version: &VersionJson,
        reporter: SharedReporter,
    ) -> Result<InstalledVersion> {
        let id = version.id.clone();

        // --- 1. Gather binary downloads (jar + libraries + natives) --------
        reporter.stage("Preparing");

        let jar_path = self.paths.version_jar(&id);
        let mut binary_downloads = Vec::new();

        if let Some(downloads) = &version.downloads {
            if let Some(client) = &downloads.client {
                binary_downloads.push(
                    Download::new(client.url.clone(), jar_path.clone())
                        .sha1(client.sha1.clone())
                        .size(client.size),
                );
            }
        }

        let mut classpath: Vec<PathBuf> = Vec::new();
        for (_lib, artifact) in version.classpath_libraries(&self.env) {
            let dest = self.paths.library_path(&artifact.path);
            let mut dl = Download::new(artifact.url.clone(), dest.clone());
            if let Some(sha1) = &artifact.sha1 {
                dl = dl.sha1(sha1.clone());
            }
            if let Some(size) = artifact.size {
                dl = dl.size(size);
            }
            binary_downloads.push(dl);
            classpath.push(dest);
        }

        // Native jars: download into the libraries dir, extract afterwards.
        let mut native_jars: Vec<(PathBuf, Vec<String>)> = Vec::new();
        for (lib, artifact) in version.native_libraries(&self.env) {
            let dest = self.paths.library_path(&artifact.path);
            let mut dl = Download::new(artifact.url.clone(), dest.clone());
            if let Some(sha1) = &artifact.sha1 {
                dl = dl.sha1(sha1.clone());
            }
            if let Some(size) = artifact.size {
                dl = dl.size(size);
            }
            binary_downloads.push(dl);
            let excludes = lib
                .extract
                .as_ref()
                .map(|e| e.exclude.clone())
                .unwrap_or_default();
            native_jars.push((dest, excludes));
        }

        reporter.stage("Downloading libraries");
        download::download_all(binary_downloads, self.concurrency, reporter.clone()).await?;

        // --- 2. Asset index + objects -------------------------------------
        let asset_index_id = self.install_assets(version, &reporter).await?;

        // --- 3. Extract natives -------------------------------------------
        reporter.stage("Extracting natives");
        let natives_dir = self.paths.natives_dir(&id);
        crate::util::ensure_dir(&natives_dir).await?;
        extract_natives(native_jars, natives_dir.clone()).await?;

        // Client jar goes last on the classpath.
        classpath.push(jar_path.clone());

        Ok(InstalledVersion {
            id,
            version: version.clone(),
            jar_path,
            natives_dir,
            classpath,
            asset_index_id,
        })
    }

    /// Download the asset index and all objects; returns the index id.
    async fn install_assets(
        &self,
        version: &VersionJson,
        reporter: &SharedReporter,
    ) -> Result<String> {
        let index_ref = version
            .asset_index
            .as_ref()
            .ok_or_else(|| Error::other("version JSON has no assetIndex"))?;

        let index_path = self.paths.asset_index_json(&index_ref.id);
        let index_dl = Download::new(index_ref.url.clone(), index_path.clone())
            .sha1(index_ref.sha1.clone())
            .size(index_ref.size);
        download::download(&index_dl, reporter).await?;

        let raw = tokio::fs::read_to_string(&index_path)
            .await
            .map_err(|e| Error::io(&index_path, e))?;
        let index = AssetIndex::parse(&raw)?;

        reporter.stage("Downloading assets");
        let downloads = index.object_downloads(&self.paths);
        download::download_all(downloads, self.concurrency, reporter.clone()).await?;

        if index.needs_materialization() {
            reporter.stage("Preparing assets");
            index.materialize(&self.paths, &index_ref.id).await?;
        }

        Ok(index_ref.id.clone())
    }
}

/// Extract a set of native jars into `dest`, honouring per-jar exclude lists.
/// Runs on the blocking pool because `zip` is synchronous.
async fn extract_natives(
    jars: Vec<(PathBuf, Vec<String>)>,
    dest: PathBuf,
) -> Result<()> {
    tokio::task::spawn_blocking(move || -> Result<()> {
        for (jar, excludes) in jars {
            extract_one(&jar, &dest, &excludes)?;
        }
        Ok(())
    })
    .await
    .map_err(|e| Error::other(format!("native extraction task panicked: {e}")))?
}

fn extract_one(jar: &Path, dest: &Path, excludes: &[String]) -> Result<()> {
    let file = std::fs::File::open(jar).map_err(|e| Error::io(jar, e))?;
    let mut archive = zip::ZipArchive::new(file)?;

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        if entry.is_dir() {
            continue;
        }
        // Use the sanitised path to defend against zip-slip.
        let rel = match entry.enclosed_name() {
            Some(p) => p,
            None => continue,
        };
        let name = rel.to_string_lossy();

        // Signatures and metadata are never useful as runtime natives.
        if name.starts_with("META-INF/") || name.starts_with("META-INF\\") {
            continue;
        }
        if excludes.iter().any(|ex| name.starts_with(ex.as_str())) {
            continue;
        }

        let out = dest.join(&rel);
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent).map_err(|e| Error::io(parent, e))?;
        }
        let mut out_file = std::fs::File::create(&out).map_err(|e| Error::io(&out, e))?;
        std::io::copy(&mut entry, &mut out_file).map_err(|e| Error::io(&out, e))?;
    }
    Ok(())
}
