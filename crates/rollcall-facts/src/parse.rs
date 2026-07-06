//! Pure parsing of the command battery's sectioned output into [`HostFacts`].
//! Every section is optional: a missing tool yields empty fields, never a
//! parse failure — absence of evidence is recorded, not invented.

use std::collections::BTreeMap;

use crate::schema::{AddressFacts, GpuFacts, GuestFacts, GuestKind, HostFacts};

/// Section sentinel emitted by the battery script.
pub const SECTION_MARKER: &str = "=====";

/// Parses the raw battery output.
pub fn parse_battery(raw: &str) -> HostFacts {
    let sections = split_sections(raw);
    let get = |name: &str| sections.get(name).map(String::as_str).unwrap_or("");

    let sys = parse_key_values(get("SYS"));
    let board = parse_key_values(get("BOARD"));
    let cpu = parse_lscpu(get("CPU"));
    let (ram_gb, dimms, empty_slots) = parse_membanks(get("MEMBANKS"));

    HostFacts {
        hostname: sys.get("hostname").cloned(),
        os: sys.get("os").cloned(),
        kernel: sys.get("kernel").cloned(),
        pve_version: sys.get("pve").cloned(),
        motherboard: parse_motherboard(&board),
        cpu_model: cpu.model,
        sockets: cpu.sockets,
        cores_per_socket: cpu.cores_per_socket,
        threads_per_core: cpu.threads_per_core,
        numa_nodes: cpu.numa_nodes,
        ram_installed_gb: ram_gb,
        dimms_populated: dimms,
        dimm_slots_empty: empty_slots,
        gpus: parse_gpus(get("GPU")),
        nvlink: parse_nvlink(get("GPUTOPO")),
        pci_devices: dedup_counted(get("PCI")),
        ib_ports: nonempty_lines(get("IB")),
        addresses: parse_addresses(get("ADDR")),
        disks: nonempty_lines(get("DISK")),
        guests: parse_guests(get("VMS"), get("CTS")),
    }
}

fn split_sections(raw: &str) -> BTreeMap<String, String> {
    let mut sections = BTreeMap::new();
    let mut current: Option<String> = None;
    let mut body = String::new();
    for line in raw.lines() {
        let trimmed = line.trim();
        if let Some(name) = trimmed
            .strip_prefix(SECTION_MARKER)
            .and_then(|rest| rest.strip_suffix(SECTION_MARKER))
        {
            if let Some(prev) = current.take() {
                sections.insert(prev, std::mem::take(&mut body));
            }
            current = Some(name.to_owned());
        } else if current.is_some() {
            body.push_str(line);
            body.push('\n');
        }
    }
    if let Some(prev) = current {
        sections.insert(prev, body);
    }
    sections
}

fn parse_key_values(section: &str) -> BTreeMap<String, String> {
    section
        .lines()
        .filter_map(|line| line.split_once('='))
        .filter(|(_, v)| !v.trim().is_empty())
        .map(|(k, v)| (k.trim().to_owned(), v.trim().to_owned()))
        .collect()
}

fn parse_motherboard(board: &BTreeMap<String, String>) -> Option<String> {
    match (board.get("board_vendor"), board.get("board_product")) {
        (Some(vendor), Some(product)) => Some(format!("{vendor} {product}")),
        (Some(one), None) | (None, Some(one)) => Some(one.clone()),
        (None, None) => None,
    }
}

#[derive(Default)]
struct CpuFacts {
    model: Option<String>,
    sockets: Option<i64>,
    cores_per_socket: Option<i64>,
    threads_per_core: Option<i64>,
    numa_nodes: Option<i64>,
}

fn parse_lscpu(section: &str) -> CpuFacts {
    let mut cpu = CpuFacts::default();
    for line in section.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();
        match key.trim() {
            "Model name" => cpu.model = Some(value.to_owned()),
            "Socket(s)" => cpu.sockets = value.parse().ok(),
            "Core(s) per socket" => cpu.cores_per_socket = value.parse().ok(),
            "Thread(s) per core" => cpu.threads_per_core = value.parse().ok(),
            "NUMA node(s)" => cpu.numa_nodes = value.parse().ok(),
            _ => {}
        }
    }
    cpu
}

/// Sums `dmidecode -t 17` "Size:" lines. Returns (installed GB, populated
/// DIMMs, empty slots).
fn parse_membanks(section: &str) -> (Option<i64>, Option<i64>, Option<i64>) {
    let mut total_gb = 0i64;
    let mut populated = 0i64;
    let mut empty = 0i64;
    let mut saw_any = false;
    for line in section.lines() {
        let Some(size) = line.trim().strip_prefix("Size:") else {
            continue;
        };
        saw_any = true;
        let size = size.trim();
        if size == "No Module Installed" || size == "None" {
            empty += 1;
        } else if let Some((amount, unit)) = size.split_once(' ') {
            if let Ok(amount) = amount.parse::<i64>() {
                populated += 1;
                total_gb += match unit {
                    "GB" => amount,
                    "MB" => amount / 1024,
                    "TB" => amount * 1024,
                    _ => 0,
                };
            }
        }
    }
    if saw_any {
        (Some(total_gb), Some(populated), Some(empty))
    } else {
        (None, None, None)
    }
}

fn parse_gpus(section: &str) -> Vec<GpuFacts> {
    section
        .lines()
        .filter_map(|line| {
            let (name, rest) = line.split_once(',')?;
            let mib = rest.trim().strip_suffix("MiB").map(str::trim)?;
            Some(GpuFacts {
                name: name.trim().to_owned(),
                memory_mib: mib.parse().ok()?,
            })
        })
        .collect()
}

/// NVLink shows up in `nvidia-smi topo -m` as cell values `NV1`, `NV2`, …
fn parse_nvlink(section: &str) -> Option<bool> {
    if section.trim().is_empty() {
        return None;
    }
    let linked = section.split_whitespace().any(|word| {
        word.strip_prefix("NV")
            .is_some_and(|rest| !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit()))
    });
    Some(linked)
}

/// Deduplicates repeated lines (SR-IOV virtual functions), keeping counts.
fn dedup_counted(section: &str) -> Vec<String> {
    let mut counts: Vec<(String, usize)> = Vec::new();
    for line in section.lines().map(str::trim).filter(|l| !l.is_empty()) {
        // Drop the PCI address so identical devices collapse.
        let desc = line.split_once(' ').map_or(line, |(_, rest)| rest);
        match counts.iter_mut().find(|(seen, _)| seen == desc) {
            Some((_, n)) => *n += 1,
            None => counts.push((desc.to_owned(), 1)),
        }
    }
    counts
        .into_iter()
        .map(
            |(desc, n)| {
                if n > 1 { format!("{n}x {desc}") } else { desc }
            },
        )
        .collect()
}

fn nonempty_lines(section: &str) -> Vec<String> {
    section
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect()
}

fn parse_addresses(section: &str) -> Vec<AddressFacts> {
    section
        .lines()
        .filter_map(|line| {
            let mut tokens = line.split_whitespace();
            let interface = tokens.next()?.to_owned();
            let _state = tokens.next()?;
            Some(AddressFacts {
                interface,
                addresses: tokens.map(str::to_owned).collect(),
            })
        })
        .collect()
}

/// Parses the RouterOS battery (`/system resource print` and friends).
/// The serial number is deliberately not recorded.
pub fn parse_routeros_battery(raw: &str) -> HostFacts {
    let sections = split_sections(raw);
    let get = |name: &str| sections.get(name).map(String::as_str).unwrap_or("");

    let resource = parse_colon_pairs(get("RESOURCE"));
    let board = parse_colon_pairs(get("ROUTERBOARD"));
    let identity = parse_colon_pairs(get("IDENTITY"));

    HostFacts {
        hostname: identity.get("name").cloned(),
        os: resource
            .get("version")
            .map(|version| format!("RouterOS {version}")),
        motherboard: board
            .get("model")
            .or_else(|| resource.get("board-name"))
            .cloned(),
        cpu_model: resource.get("cpu").cloned(),
        ram_installed_gb: resource
            .get("total-memory")
            .and_then(|v| parse_memory_to_gb(v)),
        addresses: parse_routeros_addresses(get("ADDRESS")),
        ..HostFacts::default()
    }
}

/// RouterOS `print` output: right-aligned `key: value` pairs.
fn parse_colon_pairs(section: &str) -> BTreeMap<String, String> {
    section
        .lines()
        .filter_map(|line| line.split_once(':'))
        .filter(|(_, v)| !v.trim().is_empty())
        .map(|(k, v)| (k.trim().to_owned(), v.trim().to_owned()))
        .collect()
}

/// `4096.0MiB` → 4 GB; `2.0GiB` → 2 GB.
fn parse_memory_to_gb(value: &str) -> Option<i64> {
    let value = value.trim();
    if let Some(mib) = value.strip_suffix("MiB") {
        return mib
            .trim()
            .parse::<f64>()
            .ok()
            .map(|m| (m / 1024.0).round() as i64);
    }
    if let Some(gib) = value.strip_suffix("GiB") {
        return gib.trim().parse::<f64>().ok().map(|g| g.round() as i64);
    }
    None
}

/// `/ip address print` table rows: `<index> <cidr> <network> <interface>`,
/// with `Columns:`/`Flags:` headers and `;;;` comment lines skipped.
/// Addresses group per interface.
fn parse_routeros_addresses(section: &str) -> Vec<AddressFacts> {
    let mut grouped: Vec<AddressFacts> = Vec::new();
    for line in section.lines() {
        let tokens: Vec<&str> = line.split_whitespace().collect();
        let [index, cidr, .., interface] = tokens[..] else {
            continue;
        };
        if !index.chars().all(|c| c.is_ascii_digit()) || !cidr.contains('/') {
            continue;
        }
        match grouped.iter_mut().find(|a| a.interface == interface) {
            Some(entry) => entry.addresses.push(cidr.to_owned()),
            None => grouped.push(AddressFacts {
                interface: interface.to_owned(),
                addresses: vec![cidr.to_owned()],
            }),
        }
    }
    grouped
}

fn parse_guests(vms: &str, cts: &str) -> Vec<GuestFacts> {
    let mut guests = Vec::new();
    for line in vms.lines() {
        // qm list: VMID NAME STATUS MEM(MB) BOOTDISK(GB) PID
        let tokens: Vec<&str> = line.split_whitespace().collect();
        if let [vmid, name, status, ..] = tokens[..] {
            if let Ok(vmid) = vmid.parse() {
                guests.push(GuestFacts {
                    kind: GuestKind::Vm,
                    vmid,
                    name: name.to_owned(),
                    status: status.to_owned(),
                });
            }
        }
    }
    for line in cts.lines() {
        // pct list: VMID Status [Lock] Name — name is the last token.
        let tokens: Vec<&str> = line.split_whitespace().collect();
        if tokens.len() >= 3 {
            if let Ok(vmid) = tokens[0].parse() {
                guests.push(GuestFacts {
                    kind: GuestKind::Ct,
                    vmid,
                    name: (*tokens.last().unwrap_or(&"")).to_owned(),
                    status: tokens[1].to_owned(),
                });
            }
        }
    }
    guests
}

#[cfg(test)]
mod tests;
