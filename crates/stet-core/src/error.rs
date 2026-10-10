//! Errors produced by document and buffer operations.

use std::path::PathBuf;

/// Errors produced by [`Document`](crate::Document) operations.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The file could not be read.
    #[error("failed to read {path}: {source}")]
    Read {
        /// The path that failed to read.
        path: PathBuf,
        /// The underlying I/O error.
        source: std::io::Error,
    },
    /// The file could not be written.
    #[error("failed to write {path}: {source}")]
    Write {
        /// The path that failed to write.
        path: PathBuf,
        /// The underlying I/O error.
        source: std::io::Error,
    },
    /// The file is not valid UTF-8.
    #[error("{path} is not valid UTF-8")]
    NotUtf8 {
        /// The offending path.
        path: PathBuf,
    },
    /// A save was attempted on a document with no path.
    #[error("document has no path; use save-as first")]
    NoPath,
}

/// Convenience alias used across this crate.
pub type Result<T> = std::result::Result<T, Error>;
