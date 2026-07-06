#![allow(clippy::expect_used, clippy::panic)]

use std::io::Write as _;

use rollcall_inventory::{GuestKind, GuestStatus, IpAssignment};
use serde_json::json;

use crate::declared::load_declared_blocking;

fn write_escrow(value: serde_json::Value) -> tempfile::NamedTempFile {
    let mut f = tempfile::NamedTempFile::new().expect("tempfile");
    f.write_all(value.to_string().as_bytes())
        .expect("write escrow");
    f
}

/// One hypervisor with lan+fabric interfaces, an accelerator, and one VM
/// with eth0+ib0 must reconstruct into the Device/WorkloadsFile shapes the
/// TOML loaders produce.
#[test]
fn reconstructs_device_and_guest_from_escrow() {
    let escrow = json!({
        "dcim.devices": [{
            "id": 7, "name": "Compute-B",
            "role": {"slug": "hypervisor"},
            "status": {"value": "active"},
            "primary_ip4": {"address": "192.0.2.5/24"},
            "oob_ip": null,
            "custom_fields": {
                "motherboard": "Example Board X1",
                "cpu_model": "Example EPYC Z1",
                "fabric_hca": "Example HCA H3",
                "ram_gb": 512,
                "power_typical_w": 950, "power_peak_w": 1375, "power_estimate": true
            },
            "comments": "note"
        }],
        "dcim.interfaces": [
            {"id": 10, "name": "lan", "device": {"id": 7}},
            {"id": 20, "name": "fabric", "device": {"id": 7}}
        ],
        "dcim.inventory-items": [
            {"id": 1, "name": "2x Example GPU G2 24 GB",
             "device": {"id": 7}, "description": "450 W each"}
        ],
        "ipam.ip-addresses": [
            {"address": "192.0.2.5/24", "assigned_object_id": 10,
             "assigned_object_type": "dcim.interface"},
            {"address": "198.51.100.5/24", "assigned_object_id": 20,
             "assigned_object_type": "dcim.interface"},
            // A vminterface with the SAME id as a dcim.interface — the index
            // must not confuse them.
            {"address": "192.0.2.111/24", "assigned_object_id": 10,
             "assigned_object_type": "virtualization.vminterface"},
            {"address": "198.51.100.111/24", "assigned_object_id": 20,
             "assigned_object_type": "virtualization.vminterface"}
        ],
        "virtualization.virtual-machines": [{
            "id": 12, "name": "vm-a",
            "cluster": {"name": "store"},
            "status": {"value": "active"},
            "primary_ip4": {"address": "192.0.2.111/24"},
            "custom_fields": {"vmid": 111, "guest_kind": "vm", "ip_assignment": "declared"},
            "comments": ""
        }],
        "virtualization.interfaces": [
            {"id": 10, "name": "eth0", "virtual_machine": {"id": 12}},
            {"id": 20, "name": "ib0", "virtual_machine": {"id": 12}}
        ]
    });
    let f = write_escrow(escrow);
    let (devices, workloads) = load_declared_blocking(f.path()).expect("escrow adapts");

    let compute_b = &devices.device[0];
    assert_eq!(compute_b.id, "compute-b", "id derived from name");
    assert_eq!(
        compute_b.ip_lan.as_deref(),
        Some("192.0.2.5"),
        "lan iface, mask stripped"
    );
    assert_eq!(
        compute_b.ip_fabric.as_deref(),
        Some("198.51.100.5"),
        "fabric iface — NOT the same-id vminterface's 198.51.100.111"
    );
    assert_eq!(compute_b.motherboard.as_deref(), Some("Example Board X1"));
    assert_eq!(compute_b.cpu_model.as_deref(), Some("Example EPYC Z1"));
    assert_eq!(compute_b.ram_gb, Some(512));
    assert_eq!(
        compute_b.power_typical_w, 950.0,
        "custom int coerces to f64"
    );
    assert!(compute_b.power_estimate);
    assert_eq!(compute_b.accelerator.len(), 1);
    assert_eq!(
        compute_b.accelerator[0].count, 2,
        "count parsed from '2x ...'"
    );
    assert!(compute_b.accelerator[0].model.contains("G2"));
    assert_eq!(compute_b.accelerator[0].power_each_w, 450.0);

    let vm = &workloads.guest_record[0];
    assert_eq!(vm.vmid, 111);
    assert_eq!(vm.host, "store");
    assert_eq!(vm.kind, GuestKind::Vm);
    assert_eq!(vm.status, GuestStatus::Running);
    assert_eq!(
        vm.ip_lan.as_deref(),
        Some("192.0.2.111"),
        "eth0, not the same-id dcim iface"
    );
    assert_eq!(vm.ip_fabric.as_deref(), Some("198.51.100.111"));
    assert_eq!(vm.ip_assignment, Some(IpAssignment::Declared));

    let store = workloads
        .guest
        .iter()
        .find(|g| g.host == "store")
        .expect("store placement");
    assert_eq!(store.guests, ["VM111 vm-a (running)"]);
}

/// A mgmt-only device (fabric switch) whose primary_ip4 is its mgmt address
/// must not surface that address as ip_lan — else ip_lan collides with
/// ip_mgmt for the same device.
#[test]
fn mgmt_only_device_has_no_lan_address() {
    let escrow = json!({
        "dcim.devices": [{
            "id": 5, "name": "Fabric-Sw",
            "role": {"slug": "fabric-switch"},
            "status": {"value": "active"},
            "primary_ip4": {"address": "203.0.113.7/24"},
            "custom_fields": {}
        }],
        "dcim.interfaces": [{"id": 30, "name": "mgmt", "device": {"id": 5}}],
        "ipam.ip-addresses": [
            {"address": "203.0.113.7/24", "assigned_object_id": 30,
             "assigned_object_type": "dcim.interface"}
        ],
        "virtualization.virtual-machines": []
    });
    let f = write_escrow(escrow);
    let (devices, _workloads) = load_declared_blocking(f.path()).expect("escrow adapts");
    let m = &devices.device[0];
    assert_eq!(m.ip_lan, None, "no lan interface -> no ip_lan");
    assert_eq!(m.ip_mgmt.as_deref(), Some("203.0.113.7"));
    assert!(m.role.starts_with("Fabric switch"));
}

/// A CT with offline status maps to Ct/Stopped and a "(stopped)" placement.
#[test]
fn container_and_offline_status_map_correctly() {
    let escrow = json!({
        "dcim.devices": [],
        "virtualization.virtual-machines": [{
            "id": 3, "name": "ct-a", "cluster": {"name": "store"},
            "status": {"value": "offline"},
            "custom_fields": {"vmid": 131, "guest_kind": "ct"}
        }]
    });
    let f = write_escrow(escrow);
    let (_devices, workloads) = load_declared_blocking(f.path()).expect("escrow adapts");
    let rec = &workloads.guest_record[0];
    assert_eq!(rec.kind, GuestKind::Ct);
    assert_eq!(rec.status, GuestStatus::Stopped);
    assert_eq!(workloads.guest[0].guests, ["CT131 ct-a (stopped)"]);
}
