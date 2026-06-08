//! End-to-end: install a version and launch it offline (singleplayer).
//!
//! Usage:
//!   cargo run -p launcher-core --example play              # latest release
//!   cargo run -p launcher-core --example play -- 1.20.1    # a specific version
//!
//! This performs a *full* install (client jar, all libraries, natives, the
//! complete asset set) into `./mc-test/`, auto-downloads a matching Temurin
//! JRE, then launches the game with an offline account. Expect a few hundred MB
//! of downloads the first time; subsequent runs are incremental.

use std::sync::Arc;

use launcher_core::account::Account;
use launcher_core::java;
use launcher_core::launch::{self, LaunchOptions};
use launcher_core::manifest::VersionManifest;
use launcher_core::paths::Paths;
use launcher_core::platform::Environment;
use launcher_core::progress::{Reporter, SharedReporter};
use launcher_core::Installer;

/// A reporter that prints stage changes and a coarse byte progress line.
struct CliReporter {
    total: std::sync::atomic::AtomicU64,
    done: std::sync::atomic::AtomicU64,
}
impl Reporter for CliReporter {
    fn stage(&self, name: &str) {
        use std::sync::atomic::Ordering;
        self.total.store(0, Ordering::Relaxed);
        self.done.store(0, Ordering::Relaxed);
        println!("\n== {name} ==");
    }
    fn set_total_bytes(&self, total: u64) {
        self.total
            .store(total, std::sync::atomic::Ordering::Relaxed);
    }
    fn add_bytes(&self, n: u64) {
        use std::sync::atomic::Ordering;
        let done = self.done.fetch_add(n, Ordering::Relaxed) + n;
        let total = self.total.load(Ordering::Relaxed);
        if total > 0 {
            print!(
                "\r  {:>6.1} / {:>6.1} MiB ({:>3.0}%)   ",
                done as f64 / 1_048_576.0,
                total as f64 / 1_048_576.0,
                (done as f64 / total as f64) * 100.0
            );
            use std::io::Write;
            let _ = std::io::stdout().flush();
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let paths = Paths::with_dirs("./mc-test/game", "./mc-test/data");
    let env = Environment::detect();
    println!("platform: {:?}/{:?}", env.os, env.arch);

    let manifest = VersionManifest::fetch().await?;
    let id = std::env::args()
        .nth(1)
        .unwrap_or_else(|| manifest.latest.release.clone());
    println!("target version: {id}");

    let installer = Installer::new(paths.clone());
    let version = installer.resolve_version(&manifest, &id).await?;

    let reporter: SharedReporter = Arc::new(CliReporter {
        total: Default::default(),
        done: Default::default(),
    });

    let installed = installer.install(&version, reporter.clone()).await?;
    println!("\ninstall complete: {} classpath entries", installed.classpath.len());

    // Ensure a matching Java runtime.
    let major = version
        .java_version
        .as_ref()
        .map(|j| j.major_version)
        .unwrap_or(21);
    let java = java::ensure_java(&paths, major, &reporter).await?;
    println!("\nusing java: {}", java.display());

    // Launch offline.
    let account = Account::offline("Tester");
    let options = LaunchOptions {
        max_memory_mb: 2048,
        ..Default::default()
    };

    println!("\n== Launching {id} ==");
    let mut child = launch::launch(&installed, &paths, &java, &account, &options, &env).await?;
    println!("game started (pid {:?}); waiting for exit…", child.id());
    let status = child.wait().await?;
    println!("game exited with {status}");
    Ok(())
}
