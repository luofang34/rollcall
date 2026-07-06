#![allow(clippy::expect_used, clippy::panic)]

use std::path::Path;

use crate::workloads::{GuestKind, GuestStatus, IpAssignment, load_workloads_blocking};

fn fixture_path() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/inventory/workloads.toml")
}

/// The fixture workloads.toml is the schema's golden input.
#[test]
fn parses_fixture_workloads_file() {
    let workloads = load_workloads_blocking(&fixture_path()).expect("fixture workloads must parse");
    assert!(!workloads.workload.is_empty());
    assert!(!workloads.guest.is_empty());
    for placement in &workloads.guest {
        assert!(
            !placement.guests.is_empty(),
            "{}: empty guest list",
            placement.host
        );
    }
}

/// The typed registry parses, and its enum fields round-trip through serde's
/// lowercase renames (`kind = "vm"`, `status = "stopped"`).
#[test]
fn parses_typed_guest_registry() {
    let workloads = load_workloads_blocking(&fixture_path()).expect("fixture workloads must parse");
    assert!(!workloads.guest_record.is_empty(), "registry populated");

    let ct_b = workloads
        .guest_record
        .iter()
        .find(|g| g.name == "ct-b")
        .expect("ct-b in the registry");
    assert_eq!(ct_b.vmid, 109);
    assert_eq!(ct_b.host, "store");
    assert_eq!(ct_b.kind, GuestKind::Ct);
    assert_eq!(ct_b.status, GuestStatus::Stopped);
    assert_eq!(
        ct_b.ip_lan.as_deref(),
        Some("192.0.2.109"),
        "renumbered guest declares its own IP, matching the VMID convention"
    );
    assert!(ct_b.notes.is_some());
}

/// The same address looks identical in `ip_lan` whether it is declared and
/// enforced or just an unverified DHCP lease — `ip_assignment` is the field
/// that distinguishes them, and would catch a migration bug where a re-created
/// VM silently falls back to DHCP because nothing recorded that its static IP
/// depended on a MAC-keyed router reservation. Pin that the recorded
/// mechanisms are `declared`, not merely implied by the address.
#[test]
fn ip_assignment_mechanism_is_distinct_from_the_address_itself() {
    let workloads = load_workloads_blocking(&fixture_path()).expect("fixture workloads must parse");

    let vm_b = workloads
        .guest_record
        .iter()
        .find(|g| g.vmid == 111)
        .expect("vm-b in the registry");
    assert_eq!(vm_b.ip_assignment, Some(IpAssignment::Declared));

    let vm_a = workloads
        .guest_record
        .iter()
        .find(|g| g.vmid == 110)
        .expect("vm-a in the registry");
    assert_eq!(vm_a.ip_assignment, Some(IpAssignment::Declared));
}
