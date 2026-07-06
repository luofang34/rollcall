//! The `facts/<date>.json` evidence format: one dated sweep, one entry per
//! declared device, honest about what could not be observed.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::FactsError;

/// A dated fact sweep across the declared fleet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FactsFile {
    /// Local wall-clock time the sweep started, RFC 3339 with offset.
    pub collected_at: String,
    /// One entry per declared device, in `devices.toml` order.
    pub hosts: Vec<HostReport>,
}

/// What the sweep observed (or could not observe) on one device.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostReport {
    /// Device id from `devices.toml`.
    pub id: String,
    /// Whether the device could be swept.
    pub access: AccessState,
    /// Failure detail when `access` is not `ok`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Parsed evidence when `access` is `ok`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facts: Option<HostFacts>,
}

/// Whether a device could be swept.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccessState {
    /// The battery ran and parsed.
    Ok,
    /// The device declares no `ssh` target — access not provisioned.
    NoSshConfigured,
    /// SSH failed (host down, key rejected, timeout).
    Failed,
}

/// Evidence parsed from one device's command battery.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct HostFacts {
    /// Reported hostname.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,
    /// OS pretty name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub os: Option<String>,
    /// Kernel release.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kernel: Option<String>,
    /// Proxmox VE version, on hypervisor hosts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pve_version: Option<String>,
    /// Baseboard vendor and product from DMI.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motherboard: Option<String>,
    /// CPU model name from `lscpu`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpu_model: Option<String>,
    /// Populated sockets.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sockets: Option<i64>,
    /// Cores per socket.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cores_per_socket: Option<i64>,
    /// Threads per core (2 = SMT on).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub threads_per_core: Option<i64>,
    /// NUMA nodes exposed (NPS setting on EPYC).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub numa_nodes: Option<i64>,
    /// Sum of installed DIMM sizes in GB.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ram_installed_gb: Option<i64>,
    /// Populated DIMM count.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dimms_populated: Option<i64>,
    /// Empty DIMM slots.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dimm_slots_empty: Option<i64>,
    /// GPUs from `nvidia-smi`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gpus: Vec<GpuFacts>,
    /// Whether any NVLink connection appears in the GPU topology matrix.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nvlink: Option<bool>,
    /// Accelerator/fabric PCI devices (deduplicated, `Nx ` prefix on repeats).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pci_devices: Vec<String>,
    /// InfiniBand ports (`<sysfs path>|<rate>|<state>|<link layer>`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ib_ports: Vec<String>,
    /// Interface addresses.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub addresses: Vec<AddressFacts>,
    /// Physical drives (`lsblk` name, size, model).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub disks: Vec<String>,
    /// Guests on hypervisor hosts, from `qm list` and `pct list`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub guests: Vec<GuestFacts>,
}

/// One GPU as `nvidia-smi` reports it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GpuFacts {
    /// Product name.
    pub name: String,
    /// Memory in MiB.
    pub memory_mib: i64,
}

/// One interface and its addresses.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AddressFacts {
    /// Interface name.
    pub interface: String,
    /// CIDR addresses on the interface.
    pub addresses: Vec<String>,
}

/// One guest on a hypervisor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GuestFacts {
    /// VM or CT.
    pub kind: GuestKind,
    /// Numeric guest id.
    pub vmid: i64,
    /// Guest name.
    pub name: String,
    /// Status as the hypervisor reports it (`running`, `stopped`, …).
    pub status: String,
}

/// Guest flavor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GuestKind {
    /// QEMU virtual machine.
    Vm,
    /// LXC container.
    Ct,
}

impl GuestKind {
    /// The `VM`/`CT` prefix used in declared guest strings.
    pub fn prefix(self) -> &'static str {
        match self {
            GuestKind::Vm => "VM",
            GuestKind::Ct => "CT",
        }
    }
}

/// Writes the sweep as `<dir>/<date>.json` and returns the path.
pub fn write_facts_blocking(
    dir: &Path,
    date: &str,
    facts: &FactsFile,
) -> Result<PathBuf, FactsError> {
    std::fs::create_dir_all(dir).map_err(|source| FactsError::CreateDir {
        path: dir.to_path_buf(),
        source,
    })?;
    let mut json =
        serde_json::to_string_pretty(facts).map_err(|source| FactsError::Serialize { source })?;
    json.push('\n');
    let path = dir.join(format!("{date}.json"));
    std::fs::write(&path, json).map_err(|source| FactsError::Write {
        path: path.clone(),
        source,
    })?;
    Ok(path)
}

/// Loads the lexicographically newest `<dir>/*.json` sweep.
pub fn load_newest_facts_blocking(dir: &Path) -> Result<(FactsFile, PathBuf), FactsError> {
    let entries = std::fs::read_dir(dir).map_err(|source| {
        // A facts dir that never existed means no sweep has run — point the
        // operator at the fix, not at the ENOENT.
        if source.kind() == std::io::ErrorKind::NotFound {
            FactsError::NoFacts {
                dir: dir.to_path_buf(),
            }
        } else {
            FactsError::ListFacts {
                dir: dir.to_path_buf(),
                source,
            }
        }
    })?;
    let mut paths: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    paths.sort();
    let Some(path) = paths.pop() else {
        return Err(FactsError::NoFacts {
            dir: dir.to_path_buf(),
        });
    };
    let text = std::fs::read_to_string(&path).map_err(|source| FactsError::Read {
        path: path.clone(),
        source,
    })?;
    let facts = serde_json::from_str(&text).map_err(|source| FactsError::Parse {
        path: path.clone(),
        source,
    })?;
    Ok((facts, path))
}
