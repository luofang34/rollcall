//! The crate error type: what can go wrong producing a report.

use std::path::PathBuf;

use thiserror::Error;

/// Errors raised while producing a report.
#[derive(Debug, Error)]
pub enum ReportError {
    /// A declared data file failed to load.
    #[error(transparent)]
    Inventory(#[from] rollcall_inventory::InventoryError),
    /// The status directory could not be listed.
    #[error("failed to list snapshots in {dir}")]
    ListSnapshots {
        /// Directory that was listed.
        dir: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },
    /// No snapshot exists to report on.
    #[error("no status snapshot in {dir} — run `rollcall status` first")]
    NoSnapshot {
        /// Directory that was searched.
        dir: PathBuf,
    },
    /// The newest snapshot could not be read.
    #[error("failed to read snapshot {path}")]
    ReadSnapshot {
        /// Snapshot path.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },
    /// The newest snapshot is not valid snapshot JSON.
    #[error("failed to parse snapshot {path}")]
    ParseSnapshot {
        /// Snapshot path.
        path: PathBuf,
        /// Underlying JSON error.
        #[source]
        source: serde_json::Error,
    },
    /// An editorial fragment could not be read.
    #[error("failed to read editorial fragment {path}")]
    ReadFragment {
        /// Fragment path.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },
    /// An editorial fragment references a placeholder the renderer does not
    /// compute.
    #[error("editorial fragment {path} references unknown placeholder {key:?}")]
    UnknownPlaceholder {
        /// Fragment path.
        path: PathBuf,
        /// The unrecognized placeholder key.
        key: String,
    },
    /// An editorial fragment opens a `@@` placeholder and never closes it.
    #[error("editorial fragment {path} has an unterminated @@ placeholder")]
    UnterminatedPlaceholder {
        /// Fragment path.
        path: PathBuf,
    },
    /// A drafted editorial fragment could not be written (`--narrate=draft`).
    #[error("failed to write drafted editorial fragment {path}")]
    WriteFragment {
        /// Fragment path.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },
    /// An output directory could not be created.
    #[error("failed to create {path}")]
    CreateDir {
        /// Directory that was being created.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },
    /// The rendered LaTeX could not be written.
    #[error("failed to write {path}")]
    WriteTex {
        /// File that was being written.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },
    /// XeLaTeX could not be started.
    #[error("failed to spawn xelatex — is TeX Live installed?")]
    XelatexSpawn {
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },
    /// XeLaTeX exited non-zero.
    #[error("xelatex failed for {path}; log tail:\n{log_tail}")]
    CompileFailed {
        /// Document that failed to compile.
        path: PathBuf,
        /// Tail of the XeLaTeX transcript.
        log_tail: String,
    },
    /// The compiled PDF could not be copied to `reports/`.
    #[error("failed to copy {from} to {to}")]
    CopyPdf {
        /// Compiled PDF in the source directory.
        from: PathBuf,
        /// Destination in `reports/`.
        to: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },
}
