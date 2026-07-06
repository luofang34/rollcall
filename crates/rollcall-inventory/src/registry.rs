//! The allocation invariant over the typed guest registry: declared VMIDs
//! and IPs must not collide.
//!
//! Two rules, both checked against declared data alone (no sweep needed):
//!
//! - **VMID unique per host.** A standalone Proxmox node rejects a second
//!   guest with the same VMID; the registry mirrors that. Because the check
//!   is per host, the same number may exist on two different nodes today —
//!   but a guest declared onto a host that already owns its VMID collides,
//!   which is exactly what a same-VMID migration would hit on the hypervisor.
//! - **IP unique fleet-wide.** No two declared owners — guests or devices —
//!   may claim the same address on any network.
//!
//! This is the collision gate: it emits findings, never mutates. Live drift
//! (declared-vs-observed) is the reconciler's job; this catches a bad
//! allocation before it is ever applied.

use std::collections::BTreeMap;
use std::fmt;

use crate::devices::DevicesFile;
use crate::workloads::WorkloadsFile;

/// A declared-vs-declared allocation collision found by [`validate_registry`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryConflict {
    /// Two or more guests on one host claim the same VMID.
    DuplicateVmid {
        /// Host the collision is on.
        host: String,
        /// The VMID claimed more than once.
        vmid: u32,
        /// The colliding guests, as `VM101 vm-a`-style labels.
        holders: Vec<String>,
    },
    /// Two or more declared owners (guests or devices) claim the same IP.
    DuplicateIp {
        /// The address claimed more than once.
        ip: String,
        /// The colliding owners, as human-readable labels.
        holders: Vec<String>,
    },
}

impl fmt::Display for RegistryConflict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RegistryConflict::DuplicateVmid {
                host,
                vmid,
                holders,
            } => write!(
                f,
                "VMID {vmid} declared {} times on {host}: {}",
                holders.len(),
                holders.join(", ")
            ),
            RegistryConflict::DuplicateIp { ip, holders } => write!(
                f,
                "IP {ip} claimed by {} owners: {}",
                holders.len(),
                holders.join(", ")
            ),
        }
    }
}

/// Checks the typed guest registry against itself and the device inventory.
///
/// Returns every collision found, in a deterministic order; an empty vec
/// means the declared allocations are internally consistent. The free-text
/// [`crate::workloads::GuestPlacement`] list is not consulted — only the
/// typed [`crate::workloads::GuestRecord`] registry and the devices.
pub fn validate_registry(
    devices: &DevicesFile,
    workloads: &WorkloadsFile,
) -> Vec<RegistryConflict> {
    let mut conflicts = duplicate_vmids(workloads);
    conflicts.extend(duplicate_ips(devices, workloads));
    conflicts
}

fn duplicate_vmids(workloads: &WorkloadsFile) -> Vec<RegistryConflict> {
    let mut by_host_vmid: BTreeMap<(&str, u32), Vec<String>> = BTreeMap::new();
    for guest in &workloads.guest_record {
        by_host_vmid
            .entry((guest.host.as_str(), guest.vmid))
            .or_default()
            .push(format!(
                "{}{} {}",
                guest.kind.prefix(),
                guest.vmid,
                guest.name
            ));
    }
    by_host_vmid
        .into_iter()
        .filter(|(_, holders)| holders.len() > 1)
        .map(|((host, vmid), holders)| RegistryConflict::DuplicateVmid {
            host: host.to_owned(),
            vmid,
            holders,
        })
        .collect()
}

fn duplicate_ips(devices: &DevicesFile, workloads: &WorkloadsFile) -> Vec<RegistryConflict> {
    // Every (ip, owner-label) an inventory row declares. Materialized up
    // front so the address strings outlive the map that borrows them.
    let owners: Vec<(Option<String>, String)> = devices
        .device
        .iter()
        .flat_map(|d| {
            let label = format!("device {}", d.id);
            [
                (d.ip_lan.clone(), label.clone()),
                (d.ip_fabric.clone(), label.clone()),
                (d.ip_mgmt.clone(), label),
            ]
        })
        .chain(workloads.guest_record.iter().flat_map(|g| {
            let label = format!("{}{} {} ({})", g.kind.prefix(), g.vmid, g.name, g.host);
            [
                (g.ip_lan.clone(), label.clone()),
                (g.ip_fabric.clone(), label),
            ]
        }))
        .collect();
    let mut by_ip: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for (ip, owner) in &owners {
        if let Some(ip) = ip {
            by_ip.entry(ip.as_str()).or_default().push(owner.clone());
        }
    }
    by_ip
        .into_iter()
        .filter(|(_, holders)| holders.len() > 1)
        .map(|(ip, holders)| RegistryConflict::DuplicateIp {
            ip: ip.to_owned(),
            holders,
        })
        .collect()
}

#[cfg(test)]
mod tests;
