#![allow(clippy::expect_used, clippy::panic)]

use rollcall_inventory::{DevicesFile, WorkloadsFile};

use crate::reconcile::{CheckState, model_token, reconcile};
use crate::schema::{
    AccessState, AddressFacts, FactsFile, GpuFacts, GuestFacts, GuestKind, HostFacts, HostReport,
};

fn devices(toml: &str) -> DevicesFile {
    ::toml::from_str(toml).expect("valid devices TOML")
}

fn workloads(toml: &str) -> WorkloadsFile {
    ::toml::from_str(toml).expect("valid workloads TOML")
}

fn facts_for(id: &str, facts: HostFacts) -> FactsFile {
    FactsFile {
        collected_at: "t".to_owned(),
        hosts: vec![HostReport {
            id: id.to_owned(),
            access: AccessState::Ok,
            error: None,
            facts: Some(facts),
        }],
    }
}

const NODE: &str = r#"
    [[device]]
    id = "node"
    name = "Node"
    role = "r"
    ip_lan = "192.0.2.5"
    ssh = "root@192.0.2.5"
    motherboard = "Vendor Board-1"
    cpu_model = "EPYC 1234X"
    ram_gb = 512
    fabric_hca = "Example HCA H3"
    power_typical_w = 100
    power_peak_w = 200

    [[device.accelerator]]
    model = "Example GPU G2 24 GB"
    count = 2
    power_each_w = 450

    [[device.accelerator]]
    model = "Example FPGA F200"
    count = 1
    power_each_w = 225
"#;

fn matching_facts() -> HostFacts {
    HostFacts {
        motherboard: Some("Vendor Board-1".to_owned()),
        cpu_model: Some("AMD EPYC 1234X 64-Core Processor".to_owned()),
        ram_installed_gb: Some(512),
        gpus: vec![
            GpuFacts {
                name: "Example GPU G2".to_owned(),
                memory_mib: 24564,
            },
            GpuFacts {
                name: "Example GPU G2".to_owned(),
                memory_mib: 24564,
            },
        ],
        pci_devices: vec![
            "Network controller: Example HCA H3".to_owned(),
            "Processing accelerators: Example Accel F200".to_owned(),
        ],
        addresses: vec![AddressFacts {
            interface: "vmbr0".to_owned(),
            addresses: vec!["192.0.2.5/24".to_owned()],
        }],
        ..HostFacts::default()
    }
}

#[test]
fn matching_device_reconciles_clean() {
    let results = reconcile(
        &devices(NODE),
        &workloads("workload = []\nguest = []"),
        &facts_for("node", matching_facts()),
    );
    let drifts: Vec<_> = results
        .iter()
        .filter(|r| r.state == CheckState::Drift)
        .collect();
    assert!(drifts.is_empty(), "unexpected drift: {drifts:?}");
    assert!(
        results
            .iter()
            .any(|r| r.check == "motherboard" && r.state == CheckState::Ok)
    );
    assert!(
        results
            .iter()
            .filter(|r| r.check == "accelerator")
            .all(|r| r.state == CheckState::Ok)
    );
}

#[test]
fn absent_accelerator_is_drift() {
    let mut facts = matching_facts();
    facts.pci_devices.pop();
    let results = reconcile(
        &devices(NODE),
        &workloads("workload = []\nguest = []"),
        &facts_for("node", facts),
    );
    let f200 = results
        .iter()
        .find(|r| r.check == "accelerator" && r.detail.contains("F200"))
        .expect("F200 check present");
    assert_eq!(f200.state, CheckState::Drift);
    assert!(
        f200.detail.contains("observed 0"),
        "detail: {}",
        f200.detail
    );
}

#[test]
fn ram_mismatch_is_drift() {
    let mut facts = matching_facts();
    facts.ram_installed_gb = Some(256);
    let results = reconcile(
        &devices(NODE),
        &workloads("workload = []\nguest = []"),
        &facts_for("node", facts),
    );
    let ram = results
        .iter()
        .find(|r| r.check == "ram_gb")
        .expect("ram check");
    assert_eq!(ram.state, CheckState::Drift);
}

#[test]
fn no_ssh_and_failed_sweeps_are_unverified_not_drift() {
    let file = FactsFile {
        collected_at: "t".to_owned(),
        hosts: vec![HostReport {
            id: "node".to_owned(),
            access: AccessState::NoSshConfigured,
            error: None,
            facts: None,
        }],
    };
    let results = reconcile(
        &devices(NODE),
        &workloads("workload = []\nguest = []"),
        &file,
    );
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].state, CheckState::Unverified);
    assert!(!results.iter().any(|r| r.state == CheckState::Drift));
}

#[test]
fn guest_drift_reports_both_directions_and_name_mismatches() {
    let placement = workloads(
        r#"
        workload = []
        [[guest]]
        host = "node"
        guests = [
          "VM100 web (running)",
          "CT200 db (running)",
          "VM101 gone",
        ]
        "#,
    );
    let mut facts = matching_facts();
    facts.guests = vec![
        GuestFacts {
            kind: GuestKind::Vm,
            vmid: 100,
            name: "web-renamed".to_owned(),
            status: "running".to_owned(),
        },
        GuestFacts {
            kind: GuestKind::Ct,
            vmid: 200,
            name: "db".to_owned(),
            status: "running".to_owned(),
        },
        GuestFacts {
            kind: GuestKind::Vm,
            vmid: 300,
            name: "surprise".to_owned(),
            status: "stopped".to_owned(),
        },
    ];
    let results = reconcile(&devices(NODE), &placement, &facts_for("node", facts));
    let guests = results
        .iter()
        .find(|r| r.check == "guests")
        .expect("guests check");
    assert_eq!(guests.state, CheckState::Drift);
    assert!(guests.detail.contains("VM101 gone declared but absent"));
    assert!(
        guests
            .detail
            .contains("VM300 surprise (stopped) present but undeclared")
    );
    assert!(guests.detail.contains("VM100 declared as \"web\""));
    assert!(
        !guests.detail.contains("CT200"),
        "matching guest must not appear"
    );
}

#[test]
fn model_token_extraction() {
    assert_eq!(model_token("Example GPU G2 24 GB"), Some("G2"));
    assert_eq!(model_token("Example GPU G1 32 GB"), Some("G1"));
    assert_eq!(model_token("Example FPGA F200"), Some("F200"));
    assert_eq!(model_token("Mystery Accelerator"), None);
}
