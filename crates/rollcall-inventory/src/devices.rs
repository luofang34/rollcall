//! Schema and loader for `inventory/devices.toml`: one entry per chassis,
//! with TDP-modeled power figures.

use std::path::Path;

use serde::Deserialize;

use crate::error::InventoryError;
use crate::load;

/// Top-level shape of `inventory/devices.toml`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DevicesFile {
    /// One entry per chassis.
    pub device: Vec<Device>,
}

/// One physical device.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Device {
    /// Stable identifier; matches the probe id where one exists.
    pub id: String,
    /// Display name.
    pub name: String,
    /// One-line role description.
    pub role: String,
    /// Hardware model, where recorded.
    pub model: Option<String>,
    /// LAN address, for LAN-attached devices (a fabric-only switch has
    /// none).
    pub ip_lan: Option<String>,
    /// Fabric address, for fabric-attached devices.
    pub ip_fabric: Option<String>,
    /// Management-network address, for devices whose only IP is on the
    /// mgmt net (e.g. the fabric switch).
    pub ip_mgmt: Option<String>,
    /// SSH target (`user@host`) for the read-only fact sweep. Absent means
    /// no access is provisioned; the device reports as unverified.
    pub ssh: Option<String>,
    /// Which command battery the fact sweep runs on this device.
    #[serde(default)]
    pub collector: CollectorKind,
    /// Out-of-band management controller, as a display string
    /// (`"BMC 203.0.113.4"`); drawn on the topology's mgmt rail.
    pub bmc: Option<String>,
    /// Baseboard vendor and product, exactly as DMI reports it
    /// (matched verbatim by the reconciler).
    pub motherboard: Option<String>,
    /// CPU description, where recorded (prose, for the report).
    pub cpu: Option<String>,
    /// CPU model substring the fact sweep's `lscpu` output must contain
    /// (matched by the reconciler).
    pub cpu_model: Option<String>,
    /// Installed RAM in GB, where recorded.
    pub ram_gb: Option<i64>,
    /// Fabric HCA family substring the sweep's PCI listing must contain
    /// (matched by the reconciler).
    pub fabric_hca: Option<String>,
    /// Installed drives, as `lsblk` model strings (informational).
    #[serde(default)]
    pub disks: Vec<String>,
    /// Modeled typical draw in watts.
    #[serde(deserialize_with = "load::int_or_float")]
    pub power_typical_w: f64,
    /// Modeled peak draw in watts.
    #[serde(deserialize_with = "load::int_or_float")]
    pub power_peak_w: f64,
    /// True while power figures are TDP-modeled rather than PDU-measured.
    #[serde(default)]
    pub power_estimate: bool,
    /// True when the device is deliberately kept offline (e.g. a GPU node
    /// between inference jobs); a down probe for it is expected, not an
    /// incident, and the risk register rates it as low.
    #[serde(default)]
    pub expected_offline: bool,
    /// Source-of-truth catalog entity, or `"none"` to flag an onboarding gap.
    pub source_of_truth: Option<String>,
    /// Free-form operator note.
    pub notes: Option<String>,
    /// Installed accelerators.
    #[serde(default)]
    pub accelerator: Vec<Accelerator>,
}

/// Which command battery the fact sweep runs on a device.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CollectorKind {
    /// POSIX shell battery (dmidecode, lscpu, nvidia-smi, qm/pct, …).
    #[default]
    Linux,
    /// RouterOS `print` battery for network devices.
    Routeros,
}

/// An accelerator group inside a device.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Accelerator {
    /// Accelerator model.
    pub model: String,
    /// Number installed.
    pub count: i64,
    /// Per-unit power in watts.
    #[serde(deserialize_with = "load::int_or_float")]
    pub power_each_w: f64,
}

/// Loads `devices.toml`.
pub fn load_devices_blocking(path: &Path) -> Result<DevicesFile, InventoryError> {
    load::load_toml_blocking(path)
}

#[cfg(test)]
mod tests;
