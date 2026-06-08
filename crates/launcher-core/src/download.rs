//! The download engine: integrity-checked, parallel, retrying file downloads.
//!
//! Every game file (the client jar, libraries, native jars, assets) is fetched
//! through here. The engine:
//!
//! * **skips work** when a file already exists and matches its SHA-1 (or size,
//!   when no hash is known), so installs are idempotent and resumable;
//! * downloads to a `.part` temp file and atomically renames on success, so an
//!   interrupted download never leaves a corrupt file in place;
//! * **verifies** the SHA-1 after download and fails loudly on mismatch;
//! * runs a bounded number of downloads concurrently;
//! * retries transient network failures with a small backoff.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use futures::stream::{self, StreamExt};
use tokio::io::AsyncWriteExt;

use crate::progress::SharedReporter;
use crate::{util, Error, Result};

/// Default number of simultaneous downloads.
pub const DEFAULT_CONCURRENCY: usize = 16;

const MAX_ATTEMPTS: usize = 4;

/// A single file to fetch.
#[derive(Debug, Clone)]
pub struct Download {
    pub url: String,
    pub dest: PathBuf,
    /// Expected SHA-1 (lowercase hex). When present it's the source of truth
    /// for "is this file already correct?".
    pub sha1: Option<String>,
    /// Expected size in bytes; used for progress totals and as a weak
    /// integrity check when no SHA-1 is available.
    pub size: Option<u64>,
}

impl Download {
    pub fn new(url: impl Into<String>, dest: impl Into<PathBuf>) -> Self {
        Self {
            url: url.into(),
            dest: dest.into(),
            sha1: None,
            size: None,
        }
    }

    pub fn sha1(mut self, sha1: impl Into<String>) -> Self {
        self.sha1 = Some(sha1.into());
        self
    }

    pub fn size(mut self, size: u64) -> Self {
        self.size = Some(size);
        self
    }

    /// Is the destination already present and valid? When so the download can
    /// be skipped entirely.
    async fn already_valid(&self) -> Result<bool> {
        let meta = match tokio::fs::metadata(&self.dest).await {
            Ok(m) => m,
            Err(_) => return Ok(false),
        };
        if !meta.is_file() {
            return Ok(false);
        }
        if let Some(expected) = &self.sha1 {
            let actual = util::sha1_file(&self.dest).await?;
            return Ok(&actual == expected);
        }
        if let Some(size) = self.size {
            return Ok(meta.len() == size);
        }
        // Exists but we have no way to verify — accept it.
        Ok(true)
    }

    fn temp_path(&self) -> PathBuf {
        let mut tmp = self.dest.clone();
        let name = self
            .dest
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "download".into());
        tmp.set_file_name(format!("{name}.part"));
        tmp
    }
}

/// Fetch a single file (one attempt, no retry), reporting bytes as they arrive.
async fn download_once(dl: &Download, reporter: &SharedReporter) -> Result<()> {
    util::ensure_parent(&dl.dest).await?;
    let tmp = dl.temp_path();

    let mut response = crate::http::client()
        .get(&dl.url)
        .send()
        .await?
        .error_for_status()?;

    let mut file = tokio::fs::File::create(&tmp)
        .await
        .map_err(|e| Error::io(&tmp, e))?;

    while let Some(chunk) = response.chunk().await? {
        file.write_all(&chunk)
            .await
            .map_err(|e| Error::io(&tmp, e))?;
        reporter.add_bytes(chunk.len() as u64);
    }
    file.flush().await.map_err(|e| Error::io(&tmp, e))?;
    drop(file);

    // Verify before publishing the file under its final name.
    if let Some(expected) = &dl.sha1 {
        let actual = util::sha1_file(&tmp).await?;
        if &actual != expected {
            let _ = tokio::fs::remove_file(&tmp).await;
            return Err(Error::Checksum {
                path: dl.dest.clone(),
                expected: expected.clone(),
                actual,
            });
        }
    }

    tokio::fs::rename(&tmp, &dl.dest)
        .await
        .map_err(|e| Error::io(&dl.dest, e))?;
    Ok(())
}

/// Fetch a single file with skip-if-valid and bounded retries.
pub async fn download(dl: &Download, reporter: &SharedReporter) -> Result<()> {
    if dl.already_valid().await? {
        // Count skipped bytes toward progress so totals stay consistent.
        if let Some(size) = dl.size {
            reporter.add_bytes(size);
        }
        reporter.item_done();
        return Ok(());
    }

    let mut attempt = 0;
    loop {
        attempt += 1;
        match download_once(dl, reporter).await {
            Ok(()) => {
                reporter.item_done();
                return Ok(());
            }
            // A checksum failure won't fix itself on retry — bail immediately.
            Err(e @ Error::Checksum { .. }) => return Err(e),
            Err(e) => {
                if attempt >= MAX_ATTEMPTS {
                    tracing::error!(url = %dl.url, attempts = attempt, "download failed");
                    return Err(e);
                }
                tracing::warn!(url = %dl.url, attempt, error = %e, "download attempt failed; retrying");
                tokio::time::sleep(Duration::from_millis(300 * attempt as u64)).await;
            }
        }
    }
}

/// Download many files with bounded concurrency.
///
/// The reporter's total is set to the sum of known sizes up front so a UI can
/// render a determinate progress bar. The call fails with the first error
/// encountered (after all in-flight downloads settle).
pub async fn download_all(
    downloads: Vec<Download>,
    concurrency: usize,
    reporter: SharedReporter,
) -> Result<()> {
    let total_bytes: u64 = downloads.iter().filter_map(|d| d.size).sum();
    reporter.set_total_bytes(total_bytes);

    let reporter = Arc::clone(&reporter);
    let results: Vec<Result<()>> = stream::iter(downloads)
        .map(|dl| {
            let reporter = Arc::clone(&reporter);
            async move { download(&dl, &reporter).await }
        })
        .buffer_unordered(concurrency.max(1))
        .collect()
        .await;

    for r in results {
        r?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temp_path_appends_part_suffix() {
        let dl = Download::new("https://x/y.jar", "/tmp/libs/y.jar");
        assert!(dl.temp_path().ends_with("y.jar.part"));
    }
}
