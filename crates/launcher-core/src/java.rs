//! Java runtime management.
//!
//! A version JSON declares the Java major version it needs
//! (`javaVersion.majorVersion`). Modern Minecraft moves fast — 26.1.2 wants
//! Java 25 — so rather than depend on a user-installed JDK we download a
//! matching **Eclipse Temurin** JRE from the Adoptium API, which provides
//! builds for every major on Windows/macOS/Linux across x64 and arm64.
//!
//! Layout: runtimes are installed under `<data>/java/temurin-<major>/`. An
//! install is reused if a `java` executable is already present there.
//!
//! Adoptium ships `.zip` on Windows and `.tar.gz` on macOS/Linux; both are
//! handled here. We also offer [`detect_system_java`] for users who already
//! have a suitable runtime on `PATH`.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::download::{self, Download};
use crate::platform::{Arch, Environment, Os};
use crate::progress::SharedReporter;
use crate::{paths::Paths, Error, Result};

const ADOPTIUM_API: &str = "https://api.adoptium.net/v3";

/// Name of the Java executable for the target platform.
pub const JAVA_EXE: &str = if cfg!(windows) { "java.exe" } else { "java" };
/// The `javaw` variant (no console window) on Windows.
pub const JAVAW_EXE: &str = if cfg!(windows) { "javaw.exe" } else { "java" };

/// Ensure a Temurin JRE of the given major version is installed; returns the
/// path to its `java` executable.
pub async fn ensure_java(
    paths: &Paths,
    major: u32,
    reporter: &SharedReporter,
) -> Result<PathBuf> {
    let install_dir = paths.java_dir().join(format!("temurin-{major}"));

    // Reuse an existing install.
    if let Some(exe) = find_java_executable(&install_dir) {
        tracing::debug!(major, path = %exe.display(), "reusing installed Java");
        return Ok(exe);
    }

    let env = Environment::detect();
    reporter.stage(&format!("Downloading Java {major}"));

    let asset = query_adoptium(major, env.os, env.arch).await?;
    tracing::info!(major, name = %asset.name, "downloading Temurin JRE");

    crate::util::ensure_dir(&paths.java_dir()).await?;
    let archive_path = paths.java_dir().join(&asset.name);

    // Adoptium publishes SHA-256, which our Download verifier doesn't check, so
    // we download by size then verify the hash separately.
    let dl = Download::new(asset.link.clone(), archive_path.clone()).size(asset.size);
    download::download_all(vec![dl], 4, reporter.clone()).await?;

    let actual = crate::util::sha256_file(&archive_path).await?;
    if actual != asset.checksum {
        let _ = tokio::fs::remove_file(&archive_path).await;
        return Err(Error::Checksum {
            path: archive_path,
            expected: asset.checksum,
            actual,
        });
    }

    reporter.stage("Extracting Java");
    crate::util::ensure_dir(&install_dir).await?;
    extract_archive(archive_path.clone(), install_dir.clone()).await?;
    // The archive is no longer needed once extracted.
    let _ = tokio::fs::remove_file(&archive_path).await;

    find_java_executable(&install_dir)
        .ok_or_else(|| Error::other("java executable not found after extraction"))
}

// --- Adoptium query ------------------------------------------------------

struct AdoptiumAsset {
    link: String,
    name: String,
    size: u64,
    /// SHA-256 hex.
    checksum: String,
}

#[derive(Deserialize)]
struct AdoptiumRelease {
    binary: AdoptiumBinary,
}

#[derive(Deserialize)]
struct AdoptiumBinary {
    package: AdoptiumPackage,
}

#[derive(Deserialize)]
struct AdoptiumPackage {
    checksum: String,
    link: String,
    name: String,
    size: u64,
}

async fn query_adoptium(major: u32, os: Os, arch: Arch) -> Result<AdoptiumAsset> {
    let os_str = adoptium_os(os);
    let arch_str = adoptium_arch(arch);

    // Prefer a JRE (smaller); fall back to a full JDK if no JRE is published
    // for this major/platform.
    for image_type in ["jre", "jdk"] {
        let url = format!(
            "{ADOPTIUM_API}/assets/latest/{major}/hotspot\
             ?architecture={arch_str}&image_type={image_type}&os={os_str}&vendor=eclipse"
        );
        let releases = crate::http::client()
            .get(&url)
            .send()
            .await?
            .error_for_status()?
            .json::<Vec<AdoptiumRelease>>()
            .await?;

        if let Some(release) = releases.into_iter().next() {
            let pkg = release.binary.package;
            return Ok(AdoptiumAsset {
                link: pkg.link,
                name: pkg.name,
                size: pkg.size,
                checksum: pkg.checksum,
            });
        }
    }

    Err(Error::UnsupportedPlatform(format!(
        "no Temurin Java {major} build for {os_str}/{arch_str}"
    )))
}

fn adoptium_os(os: Os) -> &'static str {
    match os {
        Os::Windows => "windows",
        Os::MacOs => "mac",
        Os::Linux => "linux",
    }
}

fn adoptium_arch(arch: Arch) -> &'static str {
    match arch {
        Arch::X64 => "x64",
        Arch::X86 => "x86",
        Arch::Arm64 => "aarch64",
        Arch::Arm32 => "arm",
    }
}

// --- Extraction ----------------------------------------------------------

async fn extract_archive(archive: PathBuf, dest: PathBuf) -> Result<()> {
    tokio::task::spawn_blocking(move || {
        let name = archive.to_string_lossy().to_lowercase();
        if name.ends_with(".zip") {
            extract_zip(&archive, &dest)
        } else if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
            extract_tar_gz(&archive, &dest)
        } else {
            Err(Error::other(format!(
                "unknown Java archive format: {}",
                archive.display()
            )))
        }
    })
    .await
    .map_err(|e| Error::other(format!("Java extraction task panicked: {e}")))?
}

fn extract_zip(archive: &Path, dest: &Path) -> Result<()> {
    let file = std::fs::File::open(archive).map_err(|e| Error::io(archive, e))?;
    let mut zip = zip::ZipArchive::new(file)?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        let rel = match entry.enclosed_name() {
            Some(p) => p,
            None => continue,
        };
        let out = dest.join(&rel);
        if entry.is_dir() {
            std::fs::create_dir_all(&out).map_err(|e| Error::io(&out, e))?;
            continue;
        }
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent).map_err(|e| Error::io(parent, e))?;
        }
        let mut out_file = std::fs::File::create(&out).map_err(|e| Error::io(&out, e))?;
        std::io::copy(&mut entry, &mut out_file).map_err(|e| Error::io(&out, e))?;

        // Preserve the unix executable bit (matters for `bin/java`).
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Some(mode) = entry.unix_mode() {
                let _ = std::fs::set_permissions(&out, std::fs::Permissions::from_mode(mode));
            }
        }
    }
    Ok(())
}

fn extract_tar_gz(archive: &Path, dest: &Path) -> Result<()> {
    let file = std::fs::File::open(archive).map_err(|e| Error::io(archive, e))?;
    let decoder = flate2::read::GzDecoder::new(file);
    let mut tar = tar::Archive::new(decoder);
    // `unpack` preserves unix permissions, including the executable bit.
    tar.unpack(dest).map_err(|e| Error::io(dest, e))?;
    Ok(())
}

// --- Locating the executable --------------------------------------------

/// Search an install directory for a `java` executable.
///
/// Temurin archives contain a single top-level release directory; the
/// executable lives at `<release>/bin/java[.exe]` (Windows/Linux) or
/// `<release>/Contents/Home/bin/java` (macOS bundles).
pub fn find_java_executable(install_dir: &Path) -> Option<PathBuf> {
    if !install_dir.is_dir() {
        return None;
    }
    find_exe_recursive(install_dir, 0)
}

fn find_exe_recursive(dir: &Path, depth: usize) -> Option<PathBuf> {
    // The executable is always within a `bin/` a few levels down; cap depth.
    if depth > 6 {
        return None;
    }
    // Direct hit: this dir is a `bin` containing the executable.
    if dir.file_name().map(|n| n == "bin").unwrap_or(false) {
        let candidate = dir.join(JAVA_EXE);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_exe_recursive(&path, depth + 1) {
                return Some(found);
            }
        }
    }
    None
}

/// Try to find a Java on `PATH` and check its major version, for users who
/// prefer their own runtime. Returns the executable path if the major matches.
pub async fn detect_system_java(required_major: u32) -> Option<PathBuf> {
    let output = tokio::process::Command::new("java")
        .arg("-version")
        .output()
        .await
        .ok()?;
    // `java -version` prints to stderr.
    let text = String::from_utf8_lossy(&output.stderr);
    let major = parse_java_major(&text)?;
    if major == required_major {
        Some(PathBuf::from("java"))
    } else {
        None
    }
}

/// Parse the major version from `java -version` output, handling both the old
/// `1.8.0_x` scheme and the modern `17.0.2` / `25` scheme.
fn parse_java_major(version_output: &str) -> Option<u32> {
    // Find a quoted version string like "25.0.1" or "1.8.0_362".
    let start = version_output.find('"')?;
    let rest = &version_output[start + 1..];
    let end = rest.find('"')?;
    let version = &rest[..end];

    let mut parts = version.split('.');
    let first = parts.next()?;
    if first == "1" {
        // Old scheme: 1.8 -> 8.
        parts.next()?.parse().ok()
    } else {
        // Modern: take leading digits (handles "25" and "25-ea").
        let digits: String = first.chars().take_while(|c| c.is_ascii_digit()).collect();
        digits.parse().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_modern_java_version() {
        let out = "openjdk version \"25.0.1\" 2025-10-21\nOpenJDK Runtime Environment";
        assert_eq!(parse_java_major(out), Some(25));
    }

    #[test]
    fn parses_legacy_java_version() {
        let out = "java version \"1.8.0_362\"";
        assert_eq!(parse_java_major(out), Some(8));
    }

    #[test]
    fn parses_ea_java_version() {
        let out = "openjdk version \"25-ea\" 2025-09-16";
        assert_eq!(parse_java_major(out), Some(25));
    }
}
