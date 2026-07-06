//! Schema and loader for `inventory/workloads.toml`: typical workloads per
//! node, the typed guest registry (VMID/IP/placement, one row per guest),
//! and the older free-text guest placement the reconciler still reads.

use std::path::Path;

use serde::Deserialize;

use crate::error::InventoryError;
use crate::load;

/// Top-level shape of `inventory/workloads.toml`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkloadsFile {
    /// Typical workloads, one entry per (node, workload).
    pub workload: Vec<Workload>,
    /// Free-text guest placement per host.
    ///
    /// Superseded by [`WorkloadsFile::guest_record`]; retained because
    /// `rollcall reconcile` still parses these strings to diff declared
    /// placement against the hypervisor sweep. New allocations go in the
    /// typed registry, not here.
    pub guest: Vec<GuestPlacement>,
    /// The typed guest registry: one row per VM/CT, the single declared
    /// source for a guest's VMID, IP, host, and status. `validate_registry`
    /// enforces VMID uniqueness per host and IP uniqueness across the fleet.
    #[serde(default)]
    pub guest_record: Vec<GuestRecord>,
}

/// One typical workload on a node.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Workload {
    /// Device id of the hosting node.
    pub node: String,
    /// Workload name.
    pub name: String,
    /// Workload category.
    pub kind: String,
    /// Resources it consumes.
    pub resources: String,
}

/// The guests recorded on one host, as free-text descriptions.
///
/// Superseded by [`GuestRecord`]; kept only so the reconciler can diff these
/// strings against the sweep until it reads the typed registry directly.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GuestPlacement {
    /// Device id of the host.
    pub host: String,
    /// Guest descriptions, one per VM/CT.
    pub guests: Vec<String>,
}

/// One guest in the typed registry — the declared identity of a single VM or
/// container. A guest's VMID, IP, host, and status are declared here first;
/// the reconciler proves the hypervisor followed.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GuestRecord {
    /// Proxmox guest id. Unique per host (a standalone node rejects a
    /// duplicate); the registry mirrors that so a migration cannot claim a
    /// VMID the destination host already owns.
    pub vmid: u32,
    /// Guest name, as the hypervisor reports it.
    pub name: String,
    /// Device id of the host this guest is placed on.
    pub host: String,
    /// Whether the guest is a full VM or a container.
    pub kind: GuestKind,
    /// LAN address, where one is allocated.
    pub ip_lan: Option<String>,
    /// Fabric (IPoIB) address, where one is allocated.
    pub ip_fabric: Option<String>,
    /// How `ip_lan` actually gets applied on the guest. Recording the
    /// mechanism, not just the address, is what catches a migration bug
    /// where an address survives by luck: the address alone looks identical
    /// whether it is declared and enforced, or just a DHCP lease nothing
    /// guarantees will persist across a VM re-create.
    #[serde(default)]
    pub ip_assignment: Option<IpAssignment>,
    /// Last known power state.
    pub status: GuestStatus,
    /// Free-form operator note (e.g. an allocation caveat or a known
    /// collision pending reconciliation).
    pub notes: Option<String>,
}

/// How a guest's `ip_lan` is actually made to stick.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum IpAssignment {
    /// Declared in the guest's own OS config (e.g. NixOS
    /// `systemd.network.networks`) and enforced independently of any
    /// network-side state — survives a VM re-create with a fresh MAC.
    Declared,
    /// A DHCP reservation keyed to the guest's MAC, held on the router or
    /// DHCP server, outside this repo's control. Breaks silently the
    /// moment the guest's MAC changes (e.g. `qm create` without a pinned
    /// `hwaddr`, as happens by default on every VM re-create).
    DhcpReservation,
    /// A plain, unreserved DHCP lease. Not guaranteed stable across
    /// reboots or lease expiry even without a re-create.
    DhcpDynamic,
    /// The mechanism has not been verified — the address in `ip_lan` may
    /// be accurate today but nothing here attests to why it will still be
    /// accurate tomorrow. Prefer this over guessing; an honest "unknown"
    /// is what should trigger verification before the next migration.
    Unverified,
}

/// Whether a guest is a full VM or a container.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GuestKind {
    /// Full virtual machine (`qm`); id renders as `VM<vmid>`.
    Vm,
    /// LXC container (`pct`); id renders as `CT<vmid>`.
    Ct,
}

impl GuestKind {
    /// The id prefix the hypervisor and the free-text placement use
    /// (`"VM"` / `"CT"`).
    pub fn prefix(self) -> &'static str {
        match self {
            GuestKind::Vm => "VM",
            GuestKind::Ct => "CT",
        }
    }
}

/// A guest's last known power state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GuestStatus {
    /// Powered on.
    Running,
    /// Powered off.
    Stopped,
}

/// Loads `workloads.toml`.
pub fn load_workloads_blocking(path: &Path) -> Result<WorkloadsFile, InventoryError> {
    load::load_toml_blocking(path)
}

#[cfg(test)]
mod tests;
