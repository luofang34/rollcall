//! Typed access to the fleet's declared data files — `inventory/*.toml`,
//! `finance/capex.toml`, `thermal/thermal.toml` — the hand-curated
//! declarations of what the site should contain.
//!
//! Each module mirrors one file: schema structs plus a `load_*_blocking`
//! function. The real data files are the schemas' golden test inputs, so
//! `cargo test` is also the schema-validation gate for the data.

pub mod capex;
pub mod devices;
mod error;
mod load;
pub mod probes;
pub mod registry;
pub mod site;
pub mod thermal;
pub mod workloads;

pub use capex::{CapexFile, CapexItem, load_capex_blocking};
pub use devices::{Accelerator, CollectorKind, Device, DevicesFile, load_devices_blocking};
pub use error::InventoryError;
pub use probes::{Probe, ProbeKind, load_probes_blocking};
pub use registry::{RegistryConflict, validate_registry};
pub use site::{Networks, PowerConstants, SiteFile, SiteIdentity, load_site_blocking};
pub use thermal::{Assumptions, Cooling, ThermalFile, load_thermal_blocking};
pub use workloads::{
    GuestKind, GuestPlacement, GuestRecord, GuestStatus, IpAssignment, Workload, WorkloadsFile,
    load_workloads_blocking,
};
