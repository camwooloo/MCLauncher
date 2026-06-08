//! Unified error type for the whole core crate.

use std::path::PathBuf;

/// Convenience alias used throughout the crate.
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("network request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("i/o error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// I/O error without a path attached (e.g. from a generic stream).
    #[error("i/o error: {0}")]
    IoBare(#[from] std::io::Error),

    #[error("failed to parse JSON: {0}")]
    Json(#[from] serde_json::Error),

    #[error("archive (zip) error: {0}")]
    Zip(#[from] zip::result::ZipError),

    #[error("checksum mismatch for {path}: expected {expected}, got {actual}")]
    Checksum {
        path: PathBuf,
        expected: String,
        actual: String,
    },

    #[error("version {0:?} not found in the manifest")]
    VersionNotFound(String),

    #[error("no compatible artifact for this platform: {0}")]
    UnsupportedPlatform(String),

    #[error("authentication failed: {0}")]
    Auth(String),

    #[error("{0}")]
    Other(String),
}

impl Error {
    /// Helper to attach a path to a raw [`std::io::Error`].
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Error::Io {
            path: path.into(),
            source,
        }
    }

    pub fn other(msg: impl Into<String>) -> Self {
        Error::Other(msg.into())
    }
}
