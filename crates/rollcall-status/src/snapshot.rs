//! The `status/<date>.json` snapshot format consumed by the report builder.

use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors raised while writing a snapshot.
#[derive(Debug, Error)]
pub enum SnapshotError {
    /// The status directory could not be created.
    #[error("failed to create status dir {path}")]
    CreateDir {
        /// Directory that was being created.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },
    /// The snapshot could not be serialized to JSON.
    #[error("failed to serialize snapshot")]
    Serialize {
        /// Underlying serialization error.
        #[source]
        source: serde_json::Error,
    },
    /// The snapshot file could not be written.
    #[error("failed to write snapshot {path}")]
    Write {
        /// File that was being written.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },
}

/// A dated point-in-time record of every probe's outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    /// Local wall-clock time the probe run started, RFC 3339 with offset.
    pub generated_at: String,
    /// One entry per probe, in `probes.toml` order.
    pub results: Vec<ProbeResult>,
}

/// The outcome of a single probe.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbeResult {
    /// Probe identifier from `probes.toml`.
    pub id: String,
    /// Human-readable description from `probes.toml`.
    pub desc: String,
    /// Judged state of the target.
    pub state: ProbeState,
    /// Mechanism-level detail behind the judgement (e.g. `HTTP 200`).
    pub detail: String,
}

/// Judged state of a probed target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProbeState {
    /// The probe succeeded.
    Up,
    /// The probe failed and the target should have been reachable.
    Down,
    /// The probe failed but the target is not reachable from this LAN
    /// (`lan_reachable = false`), so nothing can be concluded.
    Unverifiable,
}

impl fmt::Display for ProbeState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            ProbeState::Up => "up",
            ProbeState::Down => "down",
            ProbeState::Unverifiable => "unverifiable",
        };
        // pad, not write_str, so format width specifiers apply.
        f.pad(s)
    }
}

/// Timestamps for one snapshot: the instant recorded inside the file and the
/// local date that names it.
#[derive(Debug, Clone)]
pub struct SnapshotStamp {
    /// RFC 3339 local time with offset, microsecond precision.
    pub generated_at: String,
    /// Local date (`YYYY-MM-DD`) used as the snapshot filename stem.
    pub date: String,
}

/// Stamps the current local time for a snapshot.
pub fn local_stamp() -> SnapshotStamp {
    let now = jiff::Zoned::now();
    SnapshotStamp {
        generated_at: now.strftime("%Y-%m-%dT%H:%M:%S%.6f%:z").to_string(),
        date: now.strftime("%Y-%m-%d").to_string(),
    }
}

/// Writes the snapshot as `<dir>/<date>.json` and returns the path.
pub fn write_snapshot_blocking(
    dir: &Path,
    date: &str,
    snapshot: &Snapshot,
) -> Result<PathBuf, SnapshotError> {
    std::fs::create_dir_all(dir).map_err(|source| SnapshotError::CreateDir {
        path: dir.to_path_buf(),
        source,
    })?;
    let mut json = serde_json::to_string_pretty(snapshot)
        .map_err(|source| SnapshotError::Serialize { source })?;
    json.push('\n');
    let path = dir.join(format!("{date}.json"));
    std::fs::write(&path, json).map_err(|source| SnapshotError::Write {
        path: path.clone(),
        source,
    })?;
    Ok(path)
}

#[cfg(test)]
mod tests;
