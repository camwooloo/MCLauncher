//! Small filesystem / hashing helpers shared across subsystems.

use std::path::Path;

use sha1::{Digest, Sha1};
use tokio::io::AsyncReadExt;

/// Compute the SHA-1 of a file's contents as a lowercase hex string,
/// streaming so we never hold a whole jar in memory.
pub async fn sha1_file(path: &Path) -> crate::Result<String> {
    let mut file = tokio::fs::File::open(path)
        .await
        .map_err(|e| crate::Error::io(path, e))?;
    let mut hasher = Sha1::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = file
            .read(&mut buf)
            .await
            .map_err(|e| crate::Error::io(path, e))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

/// SHA-1 of an in-memory byte slice.
pub fn sha1_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha1::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

/// Compute the SHA-256 of a file's contents as a lowercase hex string.
/// Used for Adoptium archives, which publish SHA-256 checksums.
pub async fn sha256_file(path: &Path) -> crate::Result<String> {
    use sha2::{Digest, Sha256};
    let mut file = tokio::fs::File::open(path)
        .await
        .map_err(|e| crate::Error::io(path, e))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = file
            .read(&mut buf)
            .await
            .map_err(|e| crate::Error::io(path, e))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

/// `mkdir -p`.
pub async fn ensure_dir(path: &Path) -> crate::Result<()> {
    tokio::fs::create_dir_all(path)
        .await
        .map_err(|e| crate::Error::io(path, e))
}

/// Ensure a file's parent directory exists.
pub async fn ensure_parent(path: &Path) -> crate::Result<()> {
    if let Some(parent) = path.parent() {
        ensure_dir(parent).await?;
    }
    Ok(())
}
