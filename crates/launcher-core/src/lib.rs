//! `launcher-core` — the UI-agnostic engine behind the Minecraft launcher.
//!
//! The crate is organised as a set of subsystems that mirror what a launcher
//! has to do, in order:
//!
//! 1. [`paths`]    — where everything lives on disk, per OS.
//! 2. [`platform`] — OS/arch detection and the Mojang "rule" evaluator.
//! 3. [`manifest`] — the global `version_manifest_v2.json`.
//! 4. [`version`]  — a single version's JSON (libraries, args, assets, …).
//! 5. `download`   — fetch + verify game files (added next).
//! 6. `java`       — locate / download a JRE (added next).
//! 7. `auth`       — Microsoft → Xbox → Minecraft token flow (added next).
//! 8. `launch`     — build the command line and spawn the game (added next).
//!
//! Everything async is built on `tokio` + `reqwest`.

pub mod account;
pub mod assets;
pub mod auth;
pub mod download;
pub mod epic;
pub mod error;
pub mod games;
pub mod http;
pub mod install;
pub mod java;
pub mod launch;
pub mod manifest;
pub mod modloader;
pub mod modpacks;
pub mod modrinth;
pub mod paths;
pub mod platform;
pub mod progress;
pub mod server;
pub mod steam;
pub mod util;
pub mod version;

pub use account::{Account, AccountStore};
pub use auth::Auth;
pub use error::{Error, Result};
pub use install::{InstalledVersion, Installer};
pub use launch::{launch, LaunchOptions};
