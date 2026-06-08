//! Building the launch command line and spawning the game.
//!
//! This is the final step: take an [`InstalledVersion`], a [`Account`], and a
//! Java executable, and produce the exact `java ...` invocation Minecraft
//! expects. The version JSON's JVM and game arguments are *templates* full of
//! `${...}` placeholders (classpath, natives dir, auth token, …); we resolve
//! the rule-gated argument list and substitute every placeholder.
//!
//! The classpath separator (`;` on Windows, `:` elsewhere) is one of the few
//! genuinely OS-specific bits and is handled here.

use std::collections::HashMap;
use std::path::PathBuf;

use tokio::process::{Child, Command};

use crate::account::Account;
use crate::install::InstalledVersion;
use crate::paths::Paths;
use crate::platform::{Environment, Os};
use crate::{Error, Result};

/// Tunable launch parameters.
#[derive(Debug, Clone)]
pub struct LaunchOptions {
    /// Maximum heap (`-Xmx`) in megabytes.
    pub max_memory_mb: u32,
    /// Minimum heap (`-Xms`) in megabytes.
    pub min_memory_mb: u32,
    /// Extra JVM arguments prepended before the version's own.
    pub extra_jvm_args: Vec<String>,
    /// Extra game arguments appended after the version's own.
    pub extra_game_args: Vec<String>,
    /// Optional custom window resolution.
    pub resolution: Option<(u32, u32)>,
    /// Identifier reported to the game as the launcher brand.
    pub launcher_name: String,
    pub launcher_version: String,
    /// Override for the game directory (`--gameDir`) and working directory.
    /// Lets a per-instance folder hold saves/mods/config/resourcepacks while
    /// the shared install (versions/libraries/assets) stays in `paths`.
    pub game_directory: Option<PathBuf>,
}

impl Default for LaunchOptions {
    fn default() -> Self {
        Self {
            max_memory_mb: 2048,
            min_memory_mb: 512,
            extra_jvm_args: Vec::new(),
            extra_game_args: Vec::new(),
            resolution: None,
            launcher_name: "MCLauncher".to_string(),
            launcher_version: env!("CARGO_PKG_VERSION").to_string(),
            game_directory: None,
        }
    }
}

/// The OS-specific classpath element separator.
fn classpath_separator(os: Os) -> &'static str {
    if os == Os::Windows {
        ";"
    } else {
        ":"
    }
}

/// Build the fully-resolved command (program + args) without spawning it.
///
/// Returns a [`Command`] with the working directory set to the game directory.
pub fn build_command(
    installed: &InstalledVersion,
    paths: &Paths,
    java: &PathBuf,
    account: &Account,
    options: &LaunchOptions,
    env: &Environment,
) -> Result<Command> {
    let version = &installed.version;
    let main_class = version
        .main_class
        .as_deref()
        .ok_or_else(|| Error::other("version JSON has no mainClass"))?;

    let sep = classpath_separator(env.os);
    let classpath = installed
        .classpath
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(sep);

    let substitutions = build_substitutions(installed, paths, account, options, env, &classpath, sep);

    // --- JVM arguments ---------------------------------------------------
    let mut args: Vec<String> = Vec::new();
    args.push(format!("-Xmx{}M", options.max_memory_mb));
    args.push(format!("-Xms{}M", options.min_memory_mb));
    args.extend(options.extra_jvm_args.iter().cloned());

    for token in version.jvm_arguments(env) {
        args.push(substitute(&token, &substitutions));
    }

    // --- Main class ------------------------------------------------------
    args.push(main_class.to_string());

    // --- Game arguments --------------------------------------------------
    for token in version.game_arguments(env) {
        args.push(substitute(&token, &substitutions));
    }

    if let Some((w, h)) = options.resolution {
        args.push("--width".into());
        args.push(w.to_string());
        args.push("--height".into());
        args.push(h.to_string());
    }
    args.extend(options.extra_game_args.iter().cloned());

    let game_dir = options
        .game_directory
        .clone()
        .unwrap_or_else(|| paths.game_dir.clone());

    let mut command = Command::new(java);
    command.args(&args);
    command.current_dir(&game_dir);
    Ok(command)
}

/// Spawn the game, inheriting stdio onto the parent (logs go to our console).
pub async fn launch(
    installed: &InstalledVersion,
    paths: &Paths,
    java: &PathBuf,
    account: &Account,
    options: &LaunchOptions,
    env: &Environment,
) -> Result<Child> {
    let mut command = build_command(installed, paths, java, account, options, env)?;
    let child = command.spawn().map_err(|e| Error::io(java, e))?;
    Ok(child)
}

/// Construct the `${...}` placeholder → value table.
fn build_substitutions(
    installed: &InstalledVersion,
    paths: &Paths,
    account: &Account,
    options: &LaunchOptions,
    env: &Environment,
    classpath: &str,
    sep: &str,
) -> HashMap<String, String> {
    let version_type = installed
        .version
        .kind
        .clone()
        .unwrap_or_else(|| "release".to_string());

    let assets_dir = paths.assets_dir();
    // Legacy/virtual asset roots point at the virtual dir; modern ones use the
    // shared assets root.
    let game_assets = if installed.asset_index_id == "legacy"
        || installed.asset_index_id.starts_with("pre-")
    {
        paths.asset_virtual_dir(&installed.asset_index_id)
    } else {
        assets_dir.clone()
    };

    let mut m = HashMap::new();
    let mut put = |k: &str, v: String| {
        m.insert(k.to_string(), v);
    };

    let game_dir = options
        .game_directory
        .clone()
        .unwrap_or_else(|| paths.game_dir.clone());

    put("auth_player_name", account.username.clone());
    put("version_name", installed.id.clone());
    put("game_directory", game_dir.to_string_lossy().into_owned());
    put("assets_root", assets_dir.to_string_lossy().into_owned());
    put("game_assets", game_assets.to_string_lossy().into_owned());
    put("assets_index_name", installed.asset_index_id.clone());
    put("auth_uuid", account.uuid.clone());
    put("auth_access_token", account.access_token.clone());
    put("auth_xuid", account.xuid.clone());
    put("auth_session", format!("token:{}:{}", account.access_token, account.uuid));
    put("clientid", String::new());
    put("user_type", account.user_type.clone());
    put("user_properties", "{}".to_string());
    put("version_type", version_type);
    put("natives_directory", installed.natives_dir.to_string_lossy().into_owned());
    put("library_directory", paths.libraries_dir().to_string_lossy().into_owned());
    put("classpath", classpath.to_string());
    put("classpath_separator", sep.to_string());
    put("launcher_name", options.launcher_name.clone());
    put("launcher_version", options.launcher_version.clone());
    if let Some((w, h)) = options.resolution {
        put("resolution_width", w.to_string());
        put("resolution_height", h.to_string());
    }
    let _ = env; // reserved for future env-dependent substitutions
    m
}

/// Replace every `${key}` in `token` with its mapped value. Unknown
/// placeholders are left untouched.
fn substitute(token: &str, map: &HashMap<String, String>) -> String {
    if !token.contains("${") {
        return token.to_string();
    }
    let mut out = String::with_capacity(token.len());
    let mut rest = token;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        if let Some(end) = after.find('}') {
            let key = &after[..end];
            match map.get(key) {
                Some(value) => out.push_str(value),
                None => {
                    // Leave unrecognised placeholders verbatim.
                    out.push_str("${");
                    out.push_str(key);
                    out.push('}');
                }
            }
            rest = &after[end + 1..];
        } else {
            // Unterminated `${` — emit the rest literally.
            out.push_str(&rest[start..]);
            rest = "";
            break;
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map() -> HashMap<String, String> {
        let mut m = HashMap::new();
        m.insert("classpath".into(), "a.jar:b.jar".into());
        m.insert("natives_directory".into(), "/n".into());
        m.insert("auth_player_name".into(), "Steve".into());
        m
    }

    #[test]
    fn substitutes_embedded_placeholders() {
        assert_eq!(
            substitute("-Djava.library.path=${natives_directory}", &map()),
            "-Djava.library.path=/n"
        );
        assert_eq!(substitute("${classpath}", &map()), "a.jar:b.jar");
        assert_eq!(substitute("--username", &map()), "--username");
    }

    #[test]
    fn unknown_placeholder_left_verbatim() {
        assert_eq!(substitute("${unknown_thing}", &map()), "${unknown_thing}");
    }

    #[test]
    fn windows_uses_semicolon_separator() {
        assert_eq!(classpath_separator(Os::Windows), ";");
        assert_eq!(classpath_separator(Os::Linux), ":");
        assert_eq!(classpath_separator(Os::MacOs), ":");
    }
}
