#![allow(clippy::expect_used, clippy::panic)]

use crate::parse::parse_battery;
use crate::schema::GuestKind;

/// Battery output for a hypervisor host, abbreviated where repetition adds
/// nothing.
const COMPUTE_B_BATTERY: &str = r#"=====SYS=====
hostname=compute-b
os=Debian GNU/Linux 13 (trixie)
kernel=6.17.2-1-pve
pve=pve-manager/9.1.1/42db4a6cf33dac83 (running kernel: 6.17.2-1-pve)
=====BOARD=====
board_vendor=Example Board
board_product=X1
=====CPU=====
Model name:                              Example EPYC Z1 64-Core Processor
Thread(s) per core:                      2
Core(s) per socket:                      64
Socket(s):                               1
NUMA node(s):                            1
=====MEMBANKS=====
	Size: 128 GB
	Size: No Module Installed
	Size: 128 GB
	Size: No Module Installed
	Size: 128 GB
	Size: No Module Installed
	Size: 128 GB
	Size: No Module Installed
=====GPU=====
Example GPU G2, 24564 MiB
Example GPU G2, 24564 MiB
=====GPUTOPO=====
	GPU0	GPU1	NIC0	CPU Affinity	NUMA Affinity	GPU NUMA ID
GPU0	 X 	PHB	NODE	0-127	0		N/A
GPU1	PHB	 X 	NODE	0-127	0		N/A
NIC0	NODE	NODE	 X
=====PCI=====
01:00.0 VGA compatible controller: Example Corp [Example GPU G2] (rev a1)
02:00.0 VGA compatible controller: Example Corp [Example GPU G2] (rev a1)
41:00.0 Network controller: Example HCA H3 [Example HCA H3]
41:00.1 Network controller: Example HCA H3 Virtual Function
41:00.2 Network controller: Example HCA H3 Virtual Function
=====IB=====
/sys/class/infiniband/ibp65s0/ports/1|40 Gb/sec (4X QDR)|4: ACTIVE|InfiniBand
/sys/class/infiniband/ibp65s0/ports/2|40 Gb/sec (4X QDR)|4: ACTIVE|Ethernet
=====ADDR=====
lo               UNKNOWN        127.0.0.1/8 ::1/128
ibp65s0          UP             198.51.100.5/24 fe80::1/64
vmbr0            UP             192.0.2.5/24 fe80::2/64
=====DISK=====
sda        4T Example HDD 4TB
nvme0n1  1.8T Example NVMe 2TB
=====VMS=====
107 gateway running 2048 20.00 1234
211 vm-a running 16384 64.00 5678
213 vm-b stopped 8192 32.00 0
=====CTS=====
160        stopped                 ct-a
163        running                 ct-b
"#;

#[test]
fn parses_the_live_compute_b_battery() {
    let facts = parse_battery(COMPUTE_B_BATTERY);
    assert_eq!(facts.hostname.as_deref(), Some("compute-b"));
    assert_eq!(
        facts.pve_version.as_deref().map(|v| &v[..11]),
        Some("pve-manager")
    );
    assert_eq!(facts.motherboard.as_deref(), Some("Example Board X1"));
    assert_eq!(
        facts.cpu_model.as_deref(),
        Some("Example EPYC Z1 64-Core Processor")
    );
    assert_eq!(facts.sockets, Some(1));
    assert_eq!(facts.threads_per_core, Some(2));
    assert_eq!(facts.numa_nodes, Some(1));
    assert_eq!(facts.ram_installed_gb, Some(512));
    assert_eq!(facts.dimms_populated, Some(4));
    assert_eq!(facts.dimm_slots_empty, Some(4));
    assert_eq!(facts.gpus.len(), 2);
    assert_eq!(facts.gpus[0].memory_mib, 24564);
    assert_eq!(facts.nvlink, Some(false), "PHB topology has no NVLink");
    assert_eq!(facts.ib_ports.len(), 2);
    assert!(
        facts
            .pci_devices
            .iter()
            .any(|d| d.contains("[Example HCA H3]"))
    );
    assert!(
        facts
            .pci_devices
            .iter()
            .any(|d| d.starts_with("2x ") && d.contains("Virtual Function")),
        "repeated VF lines collapse with a count: {:?}",
        facts.pci_devices
    );
    assert!(
        facts
            .addresses
            .iter()
            .any(|a| a.interface == "vmbr0" && a.addresses.iter().any(|x| x == "192.0.2.5/24"))
    );
    assert_eq!(facts.disks.len(), 2);

    assert_eq!(facts.guests.len(), 5);
    let ct163 = facts
        .guests
        .iter()
        .find(|g| g.vmid == 163)
        .expect("ct163 parsed");
    assert_eq!(ct163.kind, GuestKind::Ct);
    assert_eq!(ct163.name, "ct-b");
    assert_eq!(ct163.status, "running");
    let vm213 = facts
        .guests
        .iter()
        .find(|g| g.vmid == 213)
        .expect("vm213 parsed");
    assert_eq!(vm213.kind, GuestKind::Vm);
    assert_eq!(vm213.status, "stopped");
}

#[test]
fn empty_battery_yields_empty_facts_not_errors() {
    let facts = parse_battery("");
    assert!(facts.hostname.is_none());
    assert!(facts.gpus.is_empty());
    assert!(
        facts.nvlink.is_none(),
        "no topo output means unknown, not false"
    );
    assert!(facts.guests.is_empty());
    assert!(facts.ram_installed_gb.is_none());
}

#[test]
fn nvlink_is_detected_from_nv_cells() {
    let raw = "=====GPUTOPO=====\nGPU0\t X \tNV2\nGPU1\tNV2\t X \n";
    assert_eq!(parse_battery(raw).nvlink, Some(true));
}

/// RouterOS battery output for an edge router, abbreviated.
const EDGE_BATTERY: &str = r#"=====RESOURCE=====
                   uptime: 3w6d46m21s
                  version: 7.20.5 (stable)
              free-memory: 3776.6MiB
             total-memory: 4096.0MiB
                      cpu: ARM64
                cpu-count: 4
        architecture-name: arm64
               board-name: Example Router R1
                 platform: Example
=====ROUTERBOARD=====
       routerboard: yes
             model: Example Router R1
          revision: r4
     serial-number: REDACTED000
  current-firmware: 7.18.2
=====IDENTITY=====
  name: edge
=====ADDRESS=====
Columns: ADDRESS, NETWORK, INTERFACE
 # ADDRESS            NETWORK        INTERFACE
 0 203.0.113.1/24     203.0.113.0    bridge-fabric-sw
 1 198.51.100.26/24   198.51.100.0   sfp-sfpplus1
;;; secured WAP
 7 198.51.100.64/24   198.51.100.0   bridge-b
 9 192.0.2.1/24       192.0.2.0      bridge-fabric-sw
;;; IPoIB mgmt
10 198.51.100.1/24    198.51.100.0   bridge-mgmt
"#;

#[test]
fn parses_the_live_edge_routeros_battery() {
    let facts = crate::parse::parse_routeros_battery(EDGE_BATTERY);
    assert_eq!(facts.hostname.as_deref(), Some("edge"));
    assert_eq!(facts.os.as_deref(), Some("RouterOS 7.20.5 (stable)"));
    assert_eq!(facts.motherboard.as_deref(), Some("Example Router R1"));
    assert_eq!(facts.cpu_model.as_deref(), Some("ARM64"));
    assert_eq!(facts.ram_installed_gb, Some(4));

    let fabric = facts
        .addresses
        .iter()
        .find(|a| a.interface == "bridge-fabric-sw")
        .expect("bridge-fabric-sw parsed");
    assert_eq!(fabric.addresses, ["203.0.113.1/24", "192.0.2.1/24"]);
    let mgmt = facts
        .addresses
        .iter()
        .find(|a| a.interface == "bridge-mgmt")
        .expect("bridge-mgmt parsed");
    assert_eq!(mgmt.addresses, ["198.51.100.1/24"]);
    // Comment and header lines are not addresses.
    assert!(
        facts
            .addresses
            .iter()
            .all(|a| !a.interface.starts_with(";;;"))
    );
    // The serial number is deliberately not recorded anywhere.
    let json = serde_json::to_string(&facts).expect("facts serialize");
    assert!(!json.contains("REDACTED000"));
}
