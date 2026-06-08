//! Manual smoke test of the live pipeline against Mojang's servers.
//!
//! Run with: `cargo run -p launcher-core --example smoke`
//!
//! It fetches the manifest, resolves the latest release's version JSON (a real
//! parse test), then downloads + verifies just the client jar — enough to prove
//! manifest → resolve → download → SHA-1 works without pulling hundreds of MB
//! of assets.

use std::sync::Arc;

use launcher_core::download::{self, Download};
use launcher_core::manifest::VersionManifest;
use launcher_core::paths::Paths;
use launcher_core::platform::Environment;
use launcher_core::progress::CountingReporter;
use launcher_core::Installer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Stay out of the real game directory.
    let tmp = std::env::temp_dir().join("mclauncher-smoke");
    let paths = Paths::with_dirs(tmp.join("game"), tmp.join("data"));
    println!("using temp game dir: {}", paths.game_dir.display());

    let env = Environment::detect();
    println!("platform: {:?} / {:?}", env.os, env.arch);

    let manifest = VersionManifest::fetch().await?;
    let latest = &manifest.latest.release;
    println!("latest release: {latest}");

    let installer = Installer::new(paths.clone());
    let version = installer.resolve_version(&manifest, latest).await?;
    println!(
        "resolved {} — main class {:?}, {} libraries, java {}",
        version.id,
        version.main_class.as_deref().unwrap_or("<none>"),
        version.libraries.len(),
        version
            .java_version
            .as_ref()
            .map(|j| j.major_version)
            .unwrap_or(0),
    );

    let cp = version.classpath_libraries(&env);
    let natives = version.native_libraries(&env);
    println!(
        "for this platform: {} classpath libs, {} native libs",
        cp.len(),
        natives.len()
    );

    // Download just the client jar and verify it.
    let client = version
        .downloads
        .as_ref()
        .and_then(|d| d.client.as_ref())
        .expect("no client download");
    let jar = paths.version_jar(&version.id);
    let reporter = Arc::new(CountingReporter::default());
    let dl = Download::new(client.url.clone(), jar.clone())
        .sha1(client.sha1.clone())
        .size(client.size);

    println!("downloading client jar ({} bytes)…", client.size);
    download::download_all(vec![dl], 4, reporter.clone()).await?;
    println!(
        "client jar OK at {} ({:.0}% of tracked bytes)",
        jar.display(),
        reporter.fraction() * 100.0
    );

    println!("smoke test passed ✅");
    Ok(())
}
