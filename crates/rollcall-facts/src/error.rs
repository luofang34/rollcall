//! The crate error type. An unreachable or credential-less host is a
//! per-host finding recorded in the facts file, not an error; errors are
//! reserved for a broken environment (unwritable facts dir, no sweep yet).

use std::path::PathBuf;

use thiserror::Error;

/// Errors raised while writing, listing, or loading fact sweeps.
#[derive(Debug, Error)]
pub enum FactsError {
    /// The facts directory could not be created.
    #[error("failed to create facts dir {path}")]
    CreateDir {
        /// Directory that was being created.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },
    /// The facts file could not be serialized.
    #[error("failed to serialize facts")]
    Serialize {
        /// Underlying serialization error.
        #[source]
        source: serde_json::Error,
    },
    /// The facts file could not be written.
    #[error("failed to write facts {path}")]
    Write {
        /// File that was being written.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },
    /// The facts directory could not be listed.
    #[error("failed to list facts in {dir}")]
    ListFacts {
        /// Directory that was listed.
        dir: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },
    /// No fact sweep exists to reconcile against.
    #[error("no fact sweep in {dir} — run `rollcall facts` first")]
    NoFacts {
        /// Directory that was searched.
        dir: PathBuf,
    },
    /// A facts file could not be read.
    #[error("failed to read facts {path}")]
    Read {
        /// Facts path.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },
    /// A facts file is not valid facts JSON.
    #[error("failed to parse facts {path}")]
    Parse {
        /// Facts path.
        path: PathBuf,
        /// Underlying JSON error.
        #[source]
        source: serde_json::Error,
    },
}
