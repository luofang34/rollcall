//! Sweeping the fleet: run the read-only command battery over SSH on every
//! device that has an `ssh` target.

use std::io::Write as _;
use std::process::{Command, Stdio};

use rollcall_inventory::{CollectorKind, Device};
use tracing::{info, warn};

use crate::parse::{parse_battery, parse_routeros_battery};
use crate::schema::{AccessState, FactsFile, HostReport};

/// The read-only command battery. Every command is observation-only; each
/// section tolerates a missing tool (`2>/dev/null`) so one absent binary
/// does not sink the sweep.
const BATTERY: &str = r#"
if [ "$(id -u)" = "0" ]; then SUDO=""; else SUDO="sudo -n"; fi
echo "=====SYS====="
echo "hostname=$(hostname)"
echo "os=$(. /etc/os-release 2>/dev/null && echo "$PRETTY_NAME")"
echo "kernel=$(uname -r)"
echo "pve=$(pveversion 2>/dev/null)"
echo "=====BOARD====="
echo "board_vendor=$($SUDO dmidecode -s baseboard-manufacturer 2>/dev/null)"
echo "board_product=$($SUDO dmidecode -s baseboard-product-name 2>/dev/null)"
echo "=====CPU====="
lscpu 2>/dev/null
echo "=====MEMBANKS====="
$SUDO dmidecode -t 17 2>/dev/null | grep -E "^[[:space:]]*Size:"
echo "=====GPU====="
nvidia-smi --query-gpu=name,memory.total --format=csv,noheader 2>/dev/null
echo "=====GPUTOPO====="
nvidia-smi topo -m 2>/dev/null
echo "=====PCI====="
lspci 2>/dev/null | grep -iE "nvidia|mellanox|xilinx|infiniband|processing accelerator"
echo "=====IB====="
for p in /sys/class/infiniband/*/ports/*; do
  [ -e "$p/rate" ] && echo "$p|$(cat "$p/rate")|$(cat "$p/state")|$(cat "$p/link_layer")"
done 2>/dev/null
echo "=====ADDR====="
ip -br addr 2>/dev/null | grep -vE "^(veth|fwbr|fwpr|fwln|tap)"
echo "=====DISK====="
lsblk -dn -o NAME,SIZE,MODEL 2>/dev/null | grep -vE "^(loop|zd)"
echo "=====VMS====="
$SUDO qm list 2>/dev/null | tail -n +2
echo "=====CTS====="
$SUDO pct list 2>/dev/null | tail -n +2
"#;

/// The RouterOS battery: `print` commands only (the built-in `read` group
/// cannot mutate anything either way). Passed as a single remote command —
/// RouterOS has no `sh`.
const ROUTEROS_BATTERY: &str = concat!(
    ":put \"=====RESOURCE=====\"; /system resource print; ",
    ":put \"=====ROUTERBOARD=====\"; /system routerboard print; ",
    ":put \"=====IDENTITY=====\"; /system identity print; ",
    ":put \"=====ADDRESS=====\"; /ip address print"
);

/// Sweeps every declared device and stamps the result. Devices without an
/// `ssh` target, and SSH failures, are recorded as unverified — never
/// guessed at.
pub fn collect_fleet_blocking(devices: &[Device], collected_at: String) -> FactsFile {
    let hosts = devices
        .iter()
        .map(|device| {
            let Some(ssh) = &device.ssh else {
                info!(id = device.id, "skipping: no ssh target configured");
                return HostReport {
                    id: device.id.clone(),
                    access: AccessState::NoSshConfigured,
                    error: None,
                    facts: None,
                };
            };
            info!(id = device.id, ssh, "sweeping");
            let outcome = match device.collector {
                CollectorKind::Linux => run_battery_blocking(ssh).map(|raw| parse_battery(&raw)),
                CollectorKind::Routeros => {
                    run_routeros_blocking(ssh).map(|raw| parse_routeros_battery(&raw))
                }
            };
            match outcome {
                Ok(facts) => HostReport {
                    id: device.id.clone(),
                    access: AccessState::Ok,
                    error: None,
                    facts: Some(facts),
                },
                Err(detail) => {
                    warn!(id = device.id, detail, "sweep failed");
                    HostReport {
                        id: device.id.clone(),
                        access: AccessState::Failed,
                        error: Some(detail),
                        facts: None,
                    }
                }
            }
        })
        .collect();
    FactsFile {
        collected_at,
        hosts,
    }
}

/// Runs the battery on one host via `ssh <target> sh -s`, feeding the
/// script over stdin (no quoting hazards, nothing written remotely).
fn run_battery_blocking(ssh_target: &str) -> Result<String, String> {
    let mut child = Command::new("ssh")
        .args([
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectTimeout=8",
            ssh_target,
            "sh",
            "-s",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| format!("failed to spawn ssh: {err}"))?;
    if let Some(stdin) = child.stdin.take() {
        let mut stdin = stdin;
        stdin
            .write_all(BATTERY.as_bytes())
            .map_err(|err| format!("failed to send battery: {err}"))?;
    }
    let output = child
        .wait_with_output()
        .map_err(|err| format!("ssh did not exit cleanly: {err}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "ssh exited {}: {}",
            output.status,
            stderr.lines().last().unwrap_or("no stderr")
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Runs the RouterOS battery as a single remote command argument.
fn run_routeros_blocking(ssh_target: &str) -> Result<String, String> {
    let output = Command::new("ssh")
        .args([
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectTimeout=8",
            ssh_target,
            ROUTEROS_BATTERY,
        ])
        .output()
        .map_err(|err| format!("failed to spawn ssh: {err}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "ssh exited {}: {}",
            output.status,
            stderr.lines().last().unwrap_or("no stderr")
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}
