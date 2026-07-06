#![allow(clippy::expect_used, clippy::panic)]

use std::path::Path;

use crate::devices::load_devices_blocking;
use crate::registry::{RegistryConflict, validate_registry};
use crate::workloads::{
    GuestKind, GuestRecord, GuestStatus, WorkloadsFile, load_workloads_blocking,
};

fn fixture_devices_and_workloads() -> (crate::devices::DevicesFile, WorkloadsFile) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/inventory");
    let devices =
        load_devices_blocking(&root.join("devices.toml")).expect("devices.toml must parse");
    let workloads =
        load_workloads_blocking(&root.join("workloads.toml")).expect("workloads.toml must parse");
    (devices, workloads)
}

/// The fixture inventory is the golden input: the declared registry must be
/// collision-free. An edit that double-books a VMID on one host or an IP
/// anywhere fails here — this is the regression guard for the collision gate.
#[test]
fn fixture_inventory_has_no_allocation_collisions() {
    let (devices, workloads) = fixture_devices_and_workloads();
    let conflicts = validate_registry(&devices, &workloads);
    assert!(
        conflicts.is_empty(),
        "declared allocations collide: {conflicts:?}"
    );
}

/// The registry must actually carry its guests, not be an empty list that
/// trivially passes the collision check.
#[test]
fn registry_is_populated_with_the_declared_vms() {
    let (_devices, workloads) = fixture_devices_and_workloads();
    let find = |vmid: u32| workloads.guest_record.iter().find(|g| g.vmid == vmid);

    let vm_a = find(110).expect("vm-a (VM110) in the registry");
    assert_eq!(vm_a.host, "store");
    assert_eq!(vm_a.ip_lan.as_deref(), Some("192.0.2.110"));

    let vm_b = find(111).expect("vm-b (VM111) in the registry");
    assert_eq!(vm_b.host, "store");
    assert_eq!(vm_b.ip_lan.as_deref(), Some("192.0.2.111"));
}

/// The registry's first claim-before-creation row: an identity reserved before
/// the guest exists anywhere. The row must stay claimed — losing it silently
/// would hand the address back to ad-hoc "grep and hope" allocation.
#[test]
fn claim_before_creation_row_is_registered() {
    let (_devices, workloads) = fixture_devices_and_workloads();
    let claimed = workloads
        .guest_record
        .iter()
        .find(|g| g.vmid == 114)
        .expect("CT114 in the registry");

    assert_eq!(claimed.host, "store");
    assert_eq!(claimed.kind, GuestKind::Ct);
    assert_eq!(claimed.ip_lan.as_deref(), Some("192.0.2.114"));
}

/// The concrete failure the collision gate exists to prevent: declaring a
/// second guest with a VMID and IP the destination host already owns must be
/// refused before it reaches the hypervisor.
#[test]
fn double_booking_a_vmid_and_ip_on_one_host_collides() {
    let (devices, mut workloads) = fixture_devices_and_workloads();

    workloads.guest_record.push(GuestRecord {
        vmid: 111,
        name: "ct-x".to_owned(),
        host: "store".to_owned(),
        kind: GuestKind::Ct,
        ip_lan: Some("192.0.2.111".to_owned()),
        ip_fabric: None,
        ip_assignment: None,
        status: GuestStatus::Stopped,
        notes: None,
    });

    let conflicts = validate_registry(&devices, &workloads);
    assert!(
        conflicts.iter().any(|c| matches!(
            c,
            RegistryConflict::DuplicateVmid { host, vmid: 111, .. } if host == "store"
        )),
        "expected a VMID 111 collision on store, got {conflicts:?}"
    );
    assert!(
        conflicts.iter().any(|c| matches!(
            c,
            RegistryConflict::DuplicateIp { ip, .. } if ip == "192.0.2.111"
        )),
        "expected an IP 192.0.2.111 collision, got {conflicts:?}"
    );
}
