//! The crate error type: what can go wrong pulling the escrow.

use std::path::PathBuf;

use thiserror::Error;

/// Errors raised while pulling the NetBox export.
#[derive(Debug, Error)]
pub enum NetboxError {
    /// The API token file could not be read.
    #[error(
        "failed to read NetBox token file {path} — create it with the read-only reconciler token"
    )]
    TokenRead {
        /// Token file path.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },
    /// The token file is empty.
    #[error("NetBox token file {path} is empty")]
    TokenEmpty {
        /// Token file path.
        path: PathBuf,
    },
    /// An API request failed at the transport level.
    #[error("NetBox request failed: {url}")]
    Http {
        /// Requested URL.
        url: String,
        /// Underlying HTTP error.
        /// Boxed: the ureq error is large and would bloat every `Result`.
        #[source]
        source: Box<ureq::Error>,
    },
    /// An API response was not the expected JSON shape.
    #[error("NetBox response from {url} is not the expected shape: {detail}")]
    UnexpectedResponse {
        /// Requested URL.
        url: String,
        /// What was wrong.
        detail: String,
    },
    /// An API response body failed to parse as JSON.
    #[error("failed to parse NetBox response from {url}")]
    Parse {
        /// Requested URL.
        url: String,
        /// Underlying JSON error.
        #[source]
        source: serde_json::Error,
    },
}
