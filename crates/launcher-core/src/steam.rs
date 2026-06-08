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
