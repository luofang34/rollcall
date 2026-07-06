//! Probe the fleet and record dated status snapshots.
//!
//! Runs the probes declared in `inventory/probes.toml` (ICMP via the system
//! `ping`, HTTP GET without following redirects) and writes the
//! `status/<date>.json` snapshot consumed by the report builder.
//!
//! A down host is a finding recorded in the snapshot, not an error: probe
//! failures never fail the run. Errors are reserved for a broken environment
//! (unspawnable `ping`, unwritable snapshot).

pub mod probe;
pub mod snapshot;

pub use probe::{ProbeError, TIMEOUT_S, run_probes_blocking};
pub use snapshot::{
    ProbeResult, ProbeState, Snapshot, SnapshotError, SnapshotStamp, local_stamp,
    write_snapshot_blocking,
};
