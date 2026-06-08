//! Built-in credentials so sign-in and modpack installs work with zero setup.
//!
//! The Azure **client id** is a public identifier (safe to embed — it's meant to
//! ship in client apps).
//!
//! The CurseForge API key is a **secret** and must NOT be committed to the
//! public repo. It's baked in at build time from the `AURORA_CF_KEY` environment
//! variable instead:
//!
//! ```powershell
//! $env:AURORA_CF_KEY = "<your CurseForge key>"; npm run tauri build
//! ```
//!
//! If it's unset, CurseForge modpack browsing/installing is simply disabled;
//! everything else (Modrinth, FTB, sign-in, hosting…) still works.

pub const AZURE_CLIENT_ID: &str = "807e7c3a-1ab3-4dd0-a78c-95e5892945d5";

pub const CURSEFORGE_API_KEY: &str = match option_env!("AURORA_CF_KEY") {
    Some(key) => key,
    None => "",
};
