//! The crate error type: what can go wrong loading declared data files.

use std::path::PathBuf;

use thiserror::Error;

/// Errors raised while loading declared fleet data.
#[derive(Debug, Error)]
pub enum InventoryError {
    /// A data file could not be read.
    #[error("failed to read {path}")]
    Read {
        /// Path that was read.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },
    /// A data file is not valid TOML of its expected shape.
    #[error("failed to parse {path}")]
    Parse {
        /// Path that was parsed.
        path: PathBuf,
        /// Underlying TOML error, pointing at the offending input.
        /// Boxed: the TOML error is large and would bloat every `Result`.
        #[source]
        source: Box<toml::de::Error>,
    },
    /// A probe entry has no `kind` key.
    #[error("{entry} in {path} has no kind")]
    MissingProbeKind {
        /// Path that was parsed.
        path: PathBuf,
        /// Position and id of the offending entry.
        entry: String,
    },
    /// A probe entry names a `kind` this loader does not implement.
    #[error("{entry} in {path} has unsupported kind {kind}")]
    UnknownProbeKind {
        /// Path that was parsed.
        path: PathBuf,
        /// Position and id of the offending entry.
        entry: String,
        /// The unrecognized kind value.
        kind: String,
    },
    /// A probe entry does not match its kind's schema — a missing,
    /// misspelled, or mistyped key. Unknown keys are rejected rather than
    /// ignored: a typo in an optional key (`exepct`) would otherwise
    /// silently fall back to the default and misreport the target.
    #[error("invalid {entry} in {path}")]
    InvalidProbe {
        /// Path that was parsed.
        path: PathBuf,
        /// Position and id of the offending entry.
        entry: String,
        /// Underlying TOML error naming the offending key.
        /// Boxed: the TOML error is large and would bloat every `Result`.
        #[source]
        source: Box<toml::de::Error>,
    },
    /// Two probe entries share an id. Ids key snapshot entries, and
    /// downstream consumers index by id — a duplicate would silently drop
    /// one of the rows there.
    #[error("duplicate probe id {id:?} in {path}")]
    DuplicateProbeId {
        /// Path that was parsed.
        path: PathBuf,
        /// The id that appears more than once.
        id: String,
    },
}
