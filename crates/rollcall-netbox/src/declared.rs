//! The escrow adapter: reconstruct the same
//! [`DevicesFile`] and [`WorkloadsFile`] the TOML loaders produce, but from
//! the committed `netbox/declared.json`. Downstream — reconcile, validate —
//! is identical whether the declared state came from TOML or NetBox.

use std::collections::HashMap;
use std::path::Path;

use rollcall_inventory::{
    Accelerator, CollectorKind, Device, DevicesFile, GuestKind, GuestPlacement, GuestRecord,
    GuestStatus, Workload, WorkloadsFile,
};
use serde_json::Value;

use crate::error::NetboxError;

/// Loads the escrow and reconstructs the declared inventory.
pub fn load_declared_blocking(
    escrow_path: &Path,
) -> Result<(DevicesFile, WorkloadsFile), NetboxError> {
    let text = std::fs::read_to_string(escrow_path).map_err(|source| NetboxError::TokenRead {
        path: escrow_path.to_path_buf(),
        source,
    })?;
    let escrow: Value = serde_json::from_str(&text).map_err(|source| NetboxError::Parse {
        url: escrow_path.display().to_string(),
        source,
    })?;
    let devices = adapt_devices(&escrow);
    let workloads = adapt_workloads(&escrow);
    Ok((DevicesFile { device: devices }, workloads))
}

fn array<'a>(escrow: &'a Value, key: &str) -> &'a [Value] {
    escrow
        .get(key)
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

fn str_field(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_owned)
}

/// NetBox status is `{value, label}`; the declared side uses the value.
fn status_value(v: &Value) -> Option<&str> {
    v.get("status").and_then(|s| {
        s.get("value")
            .and_then(Value::as_str)
            .or_else(|| s.as_str())
    })
}

fn adapt_devices(escrow: &Value) -> Vec<Device> {
    // Address book keyed on (assigned_object_type, interface id): dcim and
    // virtualization interface ids are separate sequences that overlap, so
    // the type must be part of the key.
    let iface_ip = build_iface_ip_index(escrow);
    let dcim_ifaces = index_ifaces(escrow, "dcim.interfaces", "device");
    let accels = accelerators_by_device(escrow);

    let mut devices = Vec::new();
    for d in array(escrow, "dcim.devices") {
        let name = str_field(d, "name").unwrap_or_default();
        let id = name.to_lowercase();
        let cf = d.get("custom_fields").cloned().unwrap_or(Value::Null);
        let device_id = d.get("id").and_then(Value::as_i64);

        let ifaces = device_id.and_then(|i| dcim_ifaces.get(&i));
        let addr_of = |iface_name: &str| {
            ifaces.and_then(|list| {
                list.iter()
                    .find(|(n, _)| n == iface_name)
                    .and_then(|(_, iid)| iface_ip.get(&("dcim.interface", *iid)).cloned())
            })
        };

        devices.push(Device {
            id: id.clone(),
            name,
            role: role_prose(d),
            model: None,
            // Addresses come from the named interfaces, not primary_ip4: a
            // mgmt-only device (the fabric switch) has its mgmt address as
            // primary_ip4, and reading ip_lan from it would collide ip_lan
            // with ip_mgmt.
            ip_lan: addr_of("lan"),
            ip_fabric: addr_of("fabric"),
            ip_mgmt: addr_of("mgmt"),
            ssh: None,
            collector: CollectorKind::default(),
            bmc: primary_ip(d, "oob_ip"),
            motherboard: cf_str(&cf, "motherboard"),
            cpu: None,
            cpu_model: cf_str(&cf, "cpu_model"),
            ram_gb: cf_int(&cf, "ram_gb"),
            fabric_hca: cf_str(&cf, "fabric_hca"),
            disks: Vec::new(),
            power_typical_w: cf_int(&cf, "power_typical_w").unwrap_or(0) as f64,
            power_peak_w: cf_int(&cf, "power_peak_w").unwrap_or(0) as f64,
            power_estimate: cf
                .get("power_estimate")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            expected_offline: cf
                .get("expected_offline")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            source_of_truth: None,
            notes: str_field(d, "comments").filter(|s| !s.is_empty()),
            accelerator: device_id
                .and_then(|i| accels.get(&i))
                .cloned()
                .unwrap_or_default(),
        });
    }
    devices.sort_by(|a, b| a.id.cmp(&b.id));
    devices
}

fn adapt_workloads(escrow: &Value) -> WorkloadsFile {
    let vm_iface_ip = build_iface_ip_index(escrow);
    let vm_ifaces = index_ifaces(escrow, "virtualization.interfaces", "virtual_machine");

    let mut placement: HashMap<String, Vec<String>> = HashMap::new();
    let mut records = Vec::new();
    for vm in array(escrow, "virtualization.virtual-machines") {
        let cf = vm.get("custom_fields").cloned().unwrap_or(Value::Null);
        let Some(vmid) = cf_int(&cf, "vmid") else {
            continue;
        };
        let vmid = vmid as u32;
        let name = str_field(vm, "name").unwrap_or_default();
        let host = vm
            .get("cluster")
            .and_then(|c| str_field(c, "name"))
            .unwrap_or_default();
        let kind = match cf_str(&cf, "guest_kind").as_deref() {
            Some("ct") => GuestKind::Ct,
            _ => GuestKind::Vm,
        };
        let running = status_value(vm) == Some("active");
        let status = if running {
            GuestStatus::Running
        } else {
            GuestStatus::Stopped
        };

        // Free-text placement string, in the shape reconcile parses:
        // "<PREFIX><vmid> <name> (<status>)".
        let state = if running { "running" } else { "stopped" };
        placement
            .entry(host.clone())
            .or_default()
            .push(format!("{}{vmid} {name} ({state})", kind.prefix()));

        let vm_id = vm.get("id").and_then(Value::as_i64);
        let addr_of = |iface_name: &str| {
            vm_id
                .and_then(|i| vm_ifaces.get(&i))
                .and_then(|list| list.iter().find(|(n, _)| n == iface_name))
                .and_then(|(_, iid)| {
                    vm_iface_ip
                        .get(&("virtualization.vminterface", *iid))
                        .cloned()
                })
        };
        records.push(GuestRecord {
            vmid,
            name,
            host,
            kind,
            ip_lan: addr_of("eth0"),
            ip_fabric: addr_of("ib0"),
            ip_assignment: cf_str(&cf, "ip_assignment").and_then(parse_ip_assignment),
            status,
            notes: str_field(vm, "comments").filter(|s| !s.is_empty()),
        });
    }

    let mut guest: Vec<GuestPlacement> = placement
        .into_iter()
        .map(|(host, mut guests)| {
            guests.sort();
            GuestPlacement { host, guests }
        })
        .collect();
    guest.sort_by(|a, b| a.host.cmp(&b.host));
    records.sort_by_key(|a| (a.host.clone(), a.vmid));

    WorkloadsFile {
        // Typical-workload rows are TOML-only editorial (report), not part of
        // the NetBox-owned DCIM/IPAM surface; the adapter leaves them empty.
        workload: Vec::<Workload>::new(),
        guest,
        guest_record: records,
    }
}

fn cf_str(cf: &Value, key: &str) -> Option<String> {
    cf.get(key).and_then(Value::as_str).map(str::to_owned)
}

fn cf_int(cf: &Value, key: &str) -> Option<i64> {
    cf.get(key).and_then(Value::as_i64)
}

fn parse_ip_assignment(s: String) -> Option<rollcall_inventory::IpAssignment> {
    use rollcall_inventory::IpAssignment::*;
    match s.as_str() {
        "declared" => Some(Declared),
        "dhcp-reservation" => Some(DhcpReservation),
        "dhcp-dynamic" => Some(DhcpDynamic),
        "unverified" => Some(Unverified),
        _ => None,
    }
}

/// A NetBox brief-IP object (`primary_ip4`, `oob_ip`) → address sans mask.
fn primary_ip(v: &Value, key: &str) -> Option<String> {
    v.get(key)
        .and_then(|ip| ip.get("address").and_then(Value::as_str))
        .and_then(|a| a.split('/').next())
        .map(str::to_owned)
}

/// (assigned_object_type, interface id) → assigned IP address (sans /mask).
/// The type is part of the key because dcim interface ids and vminterface
/// ids are independent sequences that overlap.
fn build_iface_ip_index(escrow: &Value) -> HashMap<(&str, i64), String> {
    let mut index = HashMap::new();
    for ip in array(escrow, "ipam.ip-addresses") {
        let (Some(kind), Some(iid), Some(addr)) = (
            ip.get("assigned_object_type").and_then(Value::as_str),
            ip.get("assigned_object_id").and_then(Value::as_i64),
            ip.get("address").and_then(Value::as_str),
        ) else {
            continue;
        };
        if let Some(bare) = addr.split('/').next() {
            index.entry((kind, iid)).or_insert_with(|| bare.to_owned());
        }
    }
    index
}

/// parent object id → list of (interface name, interface id).
fn index_ifaces(escrow: &Value, key: &str, parent: &str) -> HashMap<i64, Vec<(String, i64)>> {
    let mut index: HashMap<i64, Vec<(String, i64)>> = HashMap::new();
    for iface in array(escrow, key) {
        let (Some(pid), Some(name), Some(iid)) = (
            iface
                .get(parent)
                .and_then(|p| p.get("id"))
                .and_then(Value::as_i64),
            str_field(iface, "name"),
            iface.get("id").and_then(Value::as_i64),
        ) else {
            continue;
        };
        index.entry(pid).or_default().push((name, iid));
    }
    index
}

/// device id → its accelerators, parsed back from inventory-item names of the
/// form "<count>x <model>" with a "<watts> W each" description.
fn accelerators_by_device(escrow: &Value) -> HashMap<i64, Vec<Accelerator>> {
    let mut index: HashMap<i64, Vec<Accelerator>> = HashMap::new();
    for item in array(escrow, "dcim.inventory-items") {
        let Some(did) = item
            .get("device")
            .and_then(|d| d.get("id"))
            .and_then(Value::as_i64)
        else {
            continue;
        };
        let name = str_field(item, "name").unwrap_or_default();
        let (count, model) = match name.split_once('x') {
            Some((c, rest)) => match c.trim().parse::<i64>() {
                Ok(n) => (n, rest.trim().to_owned()),
                Err(_) => (1, name.clone()),
            },
            None => (1, name.clone()),
        };
        let power_each_w = str_field(item, "description")
            .and_then(|d| {
                d.split_whitespace()
                    .next()
                    .and_then(|w| w.parse::<f64>().ok())
            })
            .unwrap_or(0.0);
        index.entry(did).or_default().push(Accelerator {
            model,
            count,
            power_each_w,
        });
    }
    index
}

fn role_prose(device: &Value) -> String {
    // Map the NetBox role slug back to a prose role that preserves the
    // prefixes the report's topology generator keys on.
    let slug = device
        .get("role")
        .and_then(|r| r.get("slug").and_then(Value::as_str))
        .unwrap_or("");
    match slug {
        "edge-router" => "Edge router (from NetBox)".to_owned(),
        "fabric-switch" => "Fabric switch (from NetBox)".to_owned(),
        "hypervisor" => "Hypervisor (from NetBox)".to_owned(),
        "gpu-node" => "GPU node (from NetBox)".to_owned(),
        other => other.to_owned(),
    }
}

#[cfg(test)]
mod tests;
