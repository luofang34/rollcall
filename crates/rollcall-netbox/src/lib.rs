//! The NetBox git escrow.
//!
//! NetBox owns DCIM/IPAM; this crate pulls its data into a canonical,
//! deterministic export (`netbox/declared.json`) that is committed to the
//! fleet repo. Reconcile and report treat the *committed* export as declared
//! state — a NetBox edit only becomes declared once its export diff lands in
//! git — and the export doubles as the data-level backup that keeps the
//! NetBox database rebuildable rather than a pet.

pub mod canonical;
pub mod declared;
mod error;
pub mod logical;
pub mod pull;
pub mod restore;

pub use canonical::canonicalize_objects;
pub use declared::load_declared_blocking;
pub use error::NetboxError;
pub use logical::logical_view;
pub use pull::{ENDPOINTS, pull_blocking, read_token_blocking};
pub use restore::{Restore, RestoreReport, wipe_blocking};
