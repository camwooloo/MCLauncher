//! Steam installation discovery.
//!
//! Native games (Skyrim, Elden Ring) aren't downloaded by us — they're owned
//! on Steam. To launch/mod them we must locate their install directory, which
//! means:
//!
//! 1. find the Steam root (registry on Windows; well-known paths elsewhere),
//! 2. read `steamapps/libraryfolders.vdf` to enumerate library folders, and
//! 3. read each `steamapps/appmanifest_<appid>.acf` for the `installdir`.
//!
//! Valve's config files use the VDF (KeyValues) format; we only need to pull a
//! couple of string values, so a tiny scanner suffices rather than a full
//! parser.

use std::path::PathBuf;

/// Find the Steam root directory, if Steam is installed.
pub fn steam_root() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
        use winreg::RegKey;

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        if let Ok(key) = hkcu.open_subkey(r"Software\Valve\Steam") {
            if let Ok(path) = key.get_value::<String, _>("SteamPath") {
                let pb = PathBuf::from(path);
                if pb.is_dir() {
                    return Some(pb);
                }
            }
        }
        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        for sub in [r"SOFTWARE\WOW6432Node\Valve\Steam", r"SOFTWARE\Valve\Steam"] {
            if let Ok(key) = hklm.open_subkey(sub) {
                if let Ok(path) = key.get_value::<String, _>("InstallPath") {
                    let pb = PathBuf::from(path);
                    if pb.is_dir() {
                        return Some(pb);
                    }
                }
            }
        }
        None
    }
    #[cfg(not(windows))]
    {
        let home = dirs::home_dir()?;
        for candidate in [
            ".steam/steam",
            ".local/share/Steam",
            "Library/Application Support/Steam",
        ] {
            let pb = home.join(candidate);
            if pb.join("steamapps").is_dir() {
                return Some(pb);
            }
        }
        None
    }
}

/// All Steam library folders (the main install plus any extra drives).
pub fn steam_libraries() -> Vec<PathBuf> {
    let Some(root) = steam_root() else {
        return Vec::new();
    };
    let mut libraries = vec![root.clone()];

    let vdf = root.join("steamapps").join("libraryfolders.vdf");
    if let Ok(content) = std::fs::read_to_string(&vdf) {
        for path in vdf_values(&content, "path") {
            let pb = PathBuf::from(path);
            if pb.is_dir() && !libraries.contains(&pb) {
                libraries.push(pb);
            }
        }
    }
    libraries
}

/// Find the install directory of a Steam app by its app id.
pub fn find_app_install_dir(app_id: u32) -> Option<PathBuf> {
    for library in steam_libraries() {
        let steamapps = library.join("steamapps");
        let manifest = steamapps.join(format!("appmanifest_{app_id}.acf"));
        let Ok(content) = std::fs::read_to_string(&manifest) else {
            continue;
        };
        if let Some(install_dir) = vdf_values(&content, "installdir").into_iter().next() {
            let dir = steamapps.join("common").join(install_dir);
            if dir.is_dir() {
                return Some(dir);
            }
        }
    }
    None
}

/// The installed build of a Steam app (`buildid` in its app manifest); it
/// changes with every game update.
pub fn app_build_id(app_id: u32) -> Option<String> {
    steam_libraries().into_iter().find_map(|library| {
        let manifest = library.join("steamapps").join(format!("appmanifest_{app_id}.acf"));
        let content = std::fs::read_to_string(manifest).ok()?;
        vdf_values(&content, "buildid").into_iter().next()
    })
}

/// The game's own icon as the Steam client caches it, if Steam has shown it.
///
/// Newer clients keep per-app art in `appcache/librarycache/<appid>/`, where
/// the icon is the file named by its 40-hex-digit hash; older clients wrote a
/// flat `<appid>_icon.jpg` instead.
pub fn library_icon(app_id: u32) -> Option<PathBuf> {
    library_icon_in(&steam_root()?.join("appcache").join("librarycache"), app_id)
}

fn library_icon_in(cache: &std::path::Path, app_id: u32) -> Option<PathBuf> {
    if let Ok(entries) = std::fs::read_dir(cache.join(app_id.to_string())) {
        for entry in entries.flatten() {
            let path = entry.path();
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
            let is_image = matches!(
                path.extension().and_then(|e| e.to_str()),
                Some("jpg" | "jpeg" | "png" | "ico")
            );
            if is_image && stem.len() == 40 && stem.chars().all(|c| c.is_ascii_hexdigit()) {
                return Some(path);
            }
        }
    }
    let flat = cache.join(format!("{app_id}_icon.jpg"));
    flat.is_file().then_some(flat)
}

/// Extract every string value paired with `key` in a VDF document.
///
/// VDF pairs look like `"key"<whitespace>"value"`. We scan for the quoted key
/// token and capture the next quoted token, unescaping `\\` and `\"`.
fn vdf_values(content: &str, key: &str) -> Vec<String> {
    let needle = format!("\"{key}\"");
    let mut out = Vec::new();
    let bytes = content.as_bytes();
    let mut search_from = 0;

    while let Some(rel) = content[search_from..].find(&needle) {
        let after_key = search_from + rel + needle.len();
        // Find the opening quote of the value.
        if let Some(open) = next_quote(bytes, after_key) {
            if let Some((value, end)) = read_quoted(content, open) {
                out.push(unescape_vdf(&value));
                search_from = end;
                continue;
            }
        }
        search_from = after_key;
    }
    out
}

fn next_quote(bytes: &[u8], from: usize) -> Option<usize> {
    (from..bytes.len()).find(|&i| bytes[i] == b'"')
}

/// Read a quoted string starting at the opening-quote index; returns the
/// unescaped-raw content and the index just past the closing quote.
fn read_quoted(content: &str, open: usize) -> Option<(String, usize)> {
    let bytes = content.as_bytes();
    let mut i = open + 1;
    let start = i;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2, // skip escaped char
            b'"' => {
                return Some((content[start..i].to_string(), i + 1));
            }
            _ => i += 1,
        }
    }
    None
}

fn unescape_vdf(s: &str) -> String {
    s.replace("\\\\", "\\").replace("\\\"", "\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_library_icons_in_both_cache_layouts() {
        let cache = std::env::temp_dir().join(format!("aurora-icon-test-{}", std::process::id()));
        let hashed = cache.join("1245620");
        std::fs::create_dir_all(&hashed).unwrap();
        std::fs::write(hashed.join("header.jpg"), b"x").unwrap();
        let icon = hashed.join("b6e290dd5a92ce98f89089a207733c70c769be46.jpg");
        std::fs::write(&icon, b"x").unwrap();
        std::fs::write(cache.join("489830_icon.jpg"), b"x").unwrap();

        assert_eq!(library_icon_in(&cache, 1245620), Some(icon));
        assert_eq!(library_icon_in(&cache, 489830), Some(cache.join("489830_icon.jpg")));
        assert_eq!(library_icon_in(&cache, 1091500), None);
        std::fs::remove_dir_all(&cache).unwrap();
    }

    #[test]
    fn extracts_library_paths() {
        let vdf = r#"
"libraryfolders"
{
    "0"
    {
        "path"      "C:\\Program Files (x86)\\Steam"
        "apps" { "489830" "123" }
    }
    "1"
    {
        "path"      "D:\\SteamLibrary"
    }
}
"#;
        let paths = vdf_values(vdf, "path");
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0], r"C:\Program Files (x86)\Steam");
        assert_eq!(paths[1], r"D:\SteamLibrary");
    }

    #[test]
    fn extracts_installdir() {
        let acf = r#"
"AppState"
{
    "appid"     "489830"
    "installdir"        "Skyrim Special Edition"
}
"#;
        let dirs = vdf_values(acf, "installdir");
        assert_eq!(dirs, vec!["Skyrim Special Edition"]);
    }
}
