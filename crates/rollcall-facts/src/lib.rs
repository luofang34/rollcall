//! Read-only hardware fact collection and declared-vs-actual reconciliation.
//!
//! `collect` sweeps every device that has an `ssh` target with a fixed
//! battery of read-only commands and records the parsed evidence as a dated
//! `facts/<date>.json`. `reconcile` diffs the newest sweep against the
//! declared inventory: every check lands as ok, drift, or unverified — a
//! host that is powered off or has no provisioned access is reported
//! unverified, never assumed to match.

pub mod collect;
mod error;
pub mod parse;
pub mod reconcile;
pub mod schema;

pub use collect::collect_fleet_blocking;
pub use error::FactsError;
pub use reconcile::{CheckResult, CheckState, reconcile};
pub use schema::{
    AccessState, FactsFile, GuestFacts, GuestKind, HostFacts, HostReport,
    load_newest_facts_blocking, write_facts_blocking,
};
