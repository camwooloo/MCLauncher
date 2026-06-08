//! Async operations the UI dispatches via `Task::perform`.
//!
//! Each returns a `Result<_, String>` so the UI can display errors directly.
//! Progress flows through the shared [`Shared`] reporter, not return values.

use std::sync::Arc;

use launcher_core::account::Account;
use launcher_core::auth::Auth;
use launcher_core::java;
use launcher_core::launch::{self, LaunchOptions};
use launcher_core::manifest::VersionManifest;
use launcher_core::modloader::{fabric, quilt};
use launcher_core::paths::Paths;
use launcher_core::platform::Environment;
use launcher_core::progress::{Reporter, SharedReporter};
use launcher_core::Installer;

use crate::shared::Shared;
use crate::Loader;

/// Fetch the list of release version ids (newest first).
pub async fn load_releases() -> Result<Vec<String>, String> {
    let manifest = VersionManifest::fetch().await.map_err(|e| e.to_string())?;
    Ok(manifest.releases().map(|v| v.id.clone()).collect())
}

/// Everything needed to install + launch.
pub struct PlayRequest {
    pub paths: Paths,
    pub loader: Loader,
    pub game_version: String,
    pub account: Account,
    pub max_memory_mb: u32,
    pub shared: Arc<Shared>,
}

/// Install the selected version (+ loader) and launch the game.
pub async fn play(req: PlayRequest) -> Result<String, String> {
    play_inner(req).await.map_err(|e| e.to_string())
}

async fn play_inner(req: PlayRequest) -> launcher_core::Result<String> {
    let manifest = VersionManifest::fetch().await?;
    let installer = Installer::new(req.paths.clone());

    // Resolve the launchable version id, installing a loader profile if needed.
    req.shared.begin("Preparing");
    let version_id = match req.loader {
        Loader::Vanilla => req.game_version.clone(),
        Loader::Fabric => {
            req.shared.stage("Fetching Fabric loader");
            let lv = fabric::latest_stable(&req.game_version).await?;
            fabric::install(&req.paths, &req.game_version, &lv).await?
        }
        Loader::Quilt => {
            req.shared.stage("Fetching Quilt loader");
            let lv = quilt::latest_stable(&req.game_version).await?;
            quilt::install(&req.paths, &req.game_version, &lv).await?
        }
    };

    let version = installer.resolve_version(&manifest, &version_id).await?;
    let reporter: SharedReporter = req.shared.clone();
    let installed = installer.install(&version, reporter.clone()).await?;

    let major = version
        .java_version
        .as_ref()
        .map(|j| j.major_version)
        .unwrap_or(21);
    let java = java::ensure_java(&req.paths, major, &reporter).await?;

    req.shared.stage("Launching");
    let options = LaunchOptions {
        max_memory_mb: req.max_memory_mb,
        ..Default::default()
    };
    let env = Environment::detect();
    let child = launch::launch(&installed, &req.paths, &java, &req.account, &options, &env).await?;
    let pid = child.id();
    // Detach: let the game keep running after we return. tokio's Child does not
    // kill on drop unless asked, so dropping the handle is fine.
    drop(child);

    Ok(format!("Launched {version_id} (pid {pid:?})"))
}

/// Sign in with Microsoft via device code. The prompt is surfaced through the
/// shared state for the UI to display.
pub async fn login(
    client_id: String,
    shared: Arc<Shared>,
) -> Result<(Account, String), String> {
    let auth = Auth::new(client_id);
    let prompt_sink = shared.clone();
    let result = auth
        .login_device_code(|dc| {
            prompt_sink.set_login_prompt(dc.user_code.clone(), dc.verification_uri.clone());
        })
        .await
        .map_err(|e| e.to_string())?;
    shared.clear_login_prompt();
    Ok((result.account, result.refresh_token))
}

/// Persist an account to the on-disk store (fire-and-forget).
pub async fn persist_account(paths: Paths, account: Account, refresh_token: String) {
    use launcher_core::account::AccountStore;
    let path = paths.accounts_file();
    let mut store = AccountStore::load(&path).await.unwrap_or_default();
    store.upsert(account, refresh_token);
    let _ = store.save(&path).await;
}
