//! Declared-vs-actual comparison: every check lands as ok, drift, or
//! unverified. Reconciliation emits findings; it never applies changes.

use rollcall_inventory::{Device, DevicesFile, GuestPlacement, WorkloadsFile};

use crate::schema::{AccessState, FactsFile, HostFacts, HostReport};

/// Outcome of one check on one device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckState {
    /// Declared and observed agree.
    Ok,
    /// Declared and observed disagree.
    Drift,
    /// No evidence available (no access, host down, field not observable).
    Unverified,
}

impl std::fmt::Display for CheckState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            CheckState::Ok => "ok",
            CheckState::Drift => "DRIFT",
            CheckState::Unverified => "unverified",
        };
        f.pad(s)
    }
}

/// One check result.
#[derive(Debug, Clone)]
pub struct CheckResult {
    /// Device id the check ran against.
    pub device: String,
    /// Check name (`motherboard`, `ram_gb`, `guests`, …).
    pub check: String,
    /// Outcome.
    pub state: CheckState,
    /// Human-readable evidence for the outcome.
    pub detail: String,
}

/// Compares a fact sweep against the declared inventory.
pub fn reconcile(
    devices: &DevicesFile,
    workloads: &WorkloadsFile,
    facts: &FactsFile,
) -> Vec<CheckResult> {
    let mut results = Vec::new();
    for device in &devices.device {
        let report = facts.hosts.iter().find(|h| h.id == device.id);
        let placement = workloads.guest.iter().find(|g| g.host == device.id);
        results.extend(reconcile_device(device, report, placement));
    }
    results
}

fn reconcile_device(
    device: &Device,
    report: Option<&HostReport>,
    placement: Option<&GuestPlacement>,
) -> Vec<CheckResult> {
    let mk = |check: &str, state: CheckState, detail: String| CheckResult {
        device: device.id.clone(),
        check: check.to_owned(),
        state,
        detail,
    };

    let facts = match report {
        Some(HostReport {
            access: AccessState::Ok,
            facts: Some(facts),
            ..
        }) => facts,
        Some(HostReport {
            access,
            error: Some(error),
            ..
        }) if *access == AccessState::Failed => {
            return vec![mk(
                "access",
                CheckState::Unverified,
                format!("sweep failed: {error}"),
            )];
        }
        Some(HostReport {
            access: AccessState::NoSshConfigured,
            ..
        }) => {
            return vec![mk(
                "access",
                CheckState::Unverified,
                "no ssh target configured".to_owned(),
            )];
        }
        _ => {
            return vec![mk(
                "access",
                CheckState::Unverified,
                "device absent from the fact sweep".to_owned(),
            )];
        }
    };

    let mut results = Vec::new();
    let mut check = |name: &str,
                     declared: Option<String>,
                     observed: Option<String>,
                     matches: bool| {
        let (state, detail) = match (declared, observed) {
            (None, _) => return,
            (Some(d), None) => (
                CheckState::Unverified,
                format!("declared {d:?}, not observable"),
            ),
            (Some(d), Some(o)) if matches => (CheckState::Ok, format!("{d:?} ≙ {o:?}")),
            (Some(d), Some(o)) => (CheckState::Drift, format!("declared {d:?}, observed {o:?}")),
        };
        results.push(mk(name, state, detail));
    };

    let m = facts.motherboard.clone();
    check(
        "motherboard",
        device.motherboard.clone(),
        m.clone(),
        device.motherboard.as_deref() == m.as_deref(),
    );
    let cpu = facts.cpu_model.clone();
    check(
        "cpu_model",
        device.cpu_model.clone(),
        cpu.clone(),
        matches_contains(cpu.as_deref(), device.cpu_model.as_deref()),
    );
    let ram = facts.ram_installed_gb;
    check(
        "ram_gb",
        device.ram_gb.map(|v| format!("{v} GB")),
        ram.map(|v| format!("{v} GB")),
        device.ram_gb == ram,
    );
    let pci_haystack = facts.pci_devices.join("; ");
    check(
        "fabric_hca",
        device.fabric_hca.clone(),
        (!pci_haystack.is_empty()).then(|| pci_haystack.clone()),
        matches_contains(Some(&pci_haystack), device.fabric_hca.as_deref()),
    );
    check(
        "ip_lan",
        device.ip_lan.clone(),
        Some(all_addresses(facts)),
        device
            .ip_lan
            .as_deref()
            .is_some_and(|ip| has_address(facts, ip)),
    );
    if let Some(fabric) = &device.ip_fabric {
        check(
            "ip_fabric",
            Some(fabric.clone()),
            Some(all_addresses(facts)),
            has_address(facts, fabric),
        );
    }
    results.extend(reconcile_accelerators(device, facts, &mk));
    if let Some(placement) = placement {
        results.extend(reconcile_guests(placement, facts, &mk));
    }
    results
}

fn matches_contains(haystack: Option<&str>, needle: Option<&str>) -> bool {
    match (haystack, needle) {
        (Some(h), Some(n)) => h.contains(n),
        _ => false,
    }
}

fn all_addresses(facts: &HostFacts) -> String {
    facts
        .addresses
        .iter()
        .flat_map(|a| a.addresses.iter())
        .filter_map(|cidr| cidr.split('/').next())
        .collect::<Vec<_>>()
        .join(", ")
}

fn has_address(facts: &HostFacts, wanted: &str) -> bool {
    facts
        .addresses
        .iter()
        .flat_map(|a| a.addresses.iter())
        .any(|cidr| cidr.split('/').next() == Some(wanted))
}

/// Extracts the model-number token used to match a declared accelerator
/// against observed device names: the first token mixing letters and
/// digits (`V100`, `U200`), else the first token containing a digit
/// (`4090`).
pub fn model_token(declared_model: &str) -> Option<&str> {
    let tokens: Vec<&str> = declared_model.split_whitespace().collect();
    tokens
        .iter()
        .find(|t| {
            t.chars().any(|c| c.is_ascii_alphabetic()) && t.chars().any(|c| c.is_ascii_digit())
        })
        .or_else(|| {
            tokens
                .iter()
                .find(|t| t.chars().any(|c| c.is_ascii_digit()))
        })
        .copied()
}

fn reconcile_accelerators(
    device: &Device,
    facts: &HostFacts,
    mk: &dyn Fn(&str, CheckState, String) -> CheckResult,
) -> Vec<CheckResult> {
    let mut results = Vec::new();
    let haystack: Vec<&str> = facts
        .gpus
        .iter()
        .map(|g| g.name.as_str())
        .chain(facts.pci_devices.iter().map(String::as_str))
        .collect();
    for accel in &device.accelerator {
        let Some(token) = model_token(&accel.model) else {
            results.push(mk(
                "accelerator",
                CheckState::Unverified,
                format!("no matchable model token in {:?}", accel.model),
            ));
            continue;
        };
        // GPUs are counted per unit; a PCI-only device (FPGA) counts by line.
        let gpu_matches = facts.gpus.iter().filter(|g| g.name.contains(token)).count() as i64;
        let observed = if gpu_matches > 0 {
            gpu_matches
        } else {
            i64::from(haystack.iter().any(|h| h.contains(token)))
        };
        let (state, detail) = if observed == accel.count {
            (CheckState::Ok, format!("{}x {token} observed", accel.count))
        } else {
            (
                CheckState::Drift,
                format!("declared {}x {token}, observed {observed}", accel.count),
            )
        };
        results.push(mk("accelerator", state, detail));
    }
    results
}

fn reconcile_guests(
    placement: &GuestPlacement,
    facts: &HostFacts,
    mk: &dyn Fn(&str, CheckState, String) -> CheckResult,
) -> Vec<CheckResult> {
    let mut drifts = Vec::new();
    let declared: Vec<(String, String)> = placement
        .guests
        .iter()
        .filter_map(|entry| {
            let mut tokens = entry.split_whitespace();
            let id = tokens.next()?.to_owned();
            let name = tokens.next().unwrap_or("").to_owned();
            Some((id, name))
        })
        .collect();

    for (id, name) in &declared {
        let observed = facts
            .guests
            .iter()
            .find(|g| format!("{}{}", g.kind.prefix(), g.vmid) == *id);
        match observed {
            None => drifts.push(format!("{id} {name} declared but absent")),
            Some(g) if !name.is_empty() && g.name != *name => {
                drifts.push(format!(
                    "{id} declared as {name:?}, hypervisor says {:?}",
                    g.name
                ));
            }
            Some(_) => {}
        }
    }
    for guest in &facts.guests {
        let id = format!("{}{}", guest.kind.prefix(), guest.vmid);
        if !declared.iter().any(|(d, _)| *d == id) {
            drifts.push(format!(
                "{id} {} ({}) present but undeclared",
                guest.name, guest.status
            ));
        }
    }

    let result = if drifts.is_empty() {
        mk(
            "guests",
            CheckState::Ok,
            format!("{} declared guests all placed as observed", declared.len()),
        )
    } else {
        mk("guests", CheckState::Drift, drifts.join("; "))
    };
    vec![result]
}

#[cfg(test)]
mod tests;
