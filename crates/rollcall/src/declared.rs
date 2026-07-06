//! Selecting where declared inventory comes from: the TOML files or the
//! committed NetBox escrow. Both yield the same `DevicesFile` +
//! `WorkloadsFile`, so every consumer is source-blind.

use std::path::Path;

use anyhow::{Context, Result};
use clap::ValueEnum;
use rollcall_inventory::{DevicesFile, WorkloadsFile};

/// Where the declared inventory is read from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
pub enum DeclaredSource {
    /// The committed `netbox/declared.json` escrow — the DCIM/IPAM source of
    /// truth once NetBox owns the inventory.
    #[default]
    Netbox,
    /// The `inventory/*.toml` files — kept as an escape hatch and for the
    /// fields NetBox does not own.
    Toml,
}

/// Loads devices and workloads from the selected source.
pub fn load_declared_blocking(
    repo: &Path,
    source: DeclaredSource,
) -> Result<(DevicesFile, WorkloadsFile)> {
    match source {
        DeclaredSource::Toml => {
            let inventory = repo.join("inventory");
            let devices =
                rollcall_inventory::load_devices_blocking(&inventory.join("devices.toml"))?;
            let workloads =
                rollcall_inventory::load_workloads_blocking(&inventory.join("workloads.toml"))?;
            Ok((devices, workloads))
        }
        DeclaredSource::Netbox => {
            let escrow = repo.join("netbox").join("declared.json");
            rollcall_netbox::load_declared_blocking(&escrow).with_context(|| {
                format!(
                    "reading NetBox escrow {} — run `rollcall netbox pull` first",
                    escrow.display()
                )
            })
        }
    }
}
