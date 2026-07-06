//! Probe execution: ICMP echo and HTTP GET with a shared timeout.

use std::process::Command;
use std::time::Duration;

use rollcall_inventory::{Probe, ProbeKind};
use thiserror::Error;
use tracing::debug;

use crate::snapshot::{ProbeResult, ProbeState};

/// Seconds allowed for each probe before its target is judged unreachable.
pub const TIMEOUT_S: u64 = 6;

/// Errors from a broken probing environment. Unreachable targets are not
/// errors — they come back as `down`/`unverifiable` results.
#[derive(Debug, Error)]
pub enum ProbeError {
    /// The system `ping` binary could not be spawned.
    #[error("failed to spawn ping for {target}")]
    PingSpawn {
        /// Target that was being pinged.
        target: String,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },
    /// A probe worker thread panicked (a bug in a dependency, not a finding).
    #[error("probe worker for {id} panicked")]
    Worker {
        /// Probe whose worker died.
        id: String,
    },
}

/// Runs every probe concurrently and returns results in input order.
pub fn run_probes_blocking(probes: &[Probe]) -> Result<Vec<ProbeResult>, ProbeError> {
    let outcomes: Vec<Result<(bool, String), ProbeError>> = std::thread::scope(|scope| {
        let handles: Vec<_> = probes
            .iter()
            .map(|probe| scope.spawn(move || run_one_blocking(probe)))
            .collect();
        handles
            .into_iter()
            .zip(probes)
            .map(|(handle, probe)| {
                handle.join().unwrap_or(Err(ProbeError::Worker {
                    id: probe.id.clone(),
                }))
            })
            .collect()
    });

    let mut results = Vec::with_capacity(probes.len());
    for (probe, outcome) in probes.iter().zip(outcomes) {
        let (ok, detail) = outcome?;
        // A fabric-only host that fails from the LAN is unverifiable, not down.
        let state = if ok {
            ProbeState::Up
        } else if probe.lan_reachable {
            ProbeState::Down
        } else {
            ProbeState::Unverifiable
        };
        results.push(ProbeResult {
            id: probe.id.clone(),
            desc: probe.desc.clone(),
            state,
            detail,
        });
    }
    Ok(results)
}

fn run_one_blocking(probe: &Probe) -> Result<(bool, String), ProbeError> {
    debug!(id = probe.id, "probing");
    match &probe.kind {
        ProbeKind::Icmp { target } => probe_icmp_blocking(target),
        ProbeKind::Http {
            url,
            expect,
            insecure,
        } => Ok(probe_http_blocking(url, expect, *insecure)),
    }
}

/// Pings the target once; up means an echo reply arrived within the timeout.
pub fn probe_icmp_blocking(target: &str) -> Result<(bool, String), ProbeError> {
    // macOS ping takes -W in milliseconds, Linux ping in seconds.
    let wait = if cfg!(target_os = "macos") {
        (TIMEOUT_S * 1000).to_string()
    } else {
        TIMEOUT_S.to_string()
    };
    let output = Command::new("ping")
        .args(["-c", "1", "-W", &wait, target])
        .output()
        .map_err(|source| ProbeError::PingSpawn {
            target: target.to_owned(),
            source,
        })?;
    if output.status.success() {
        Ok((true, "icmp reply".to_owned()))
    } else {
        Ok((false, "no icmp reply".to_owned()))
    }
}

/// Fetches the URL once, without following redirects; up means the response
/// status is listed in `expect`. A 301/302 from the service proves it is up,
/// while following it could cross into a different listener (e.g. a
/// self-signed HTTPS one) and mask that.
pub fn probe_http_blocking(url: &str, expect: &[u16], insecure: bool) -> (bool, String) {
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(TIMEOUT_S)))
        .max_redirects(0)
        .http_status_as_error(false)
        .tls_config(
            ureq::tls::TlsConfig::builder()
                .disable_verification(insecure)
                .build(),
        )
        .build();
    let agent = ureq::Agent::new_with_config(config);
    match agent.get(url).call() {
        Ok(response) => {
            let code = response.status().as_u16();
            (expect.contains(&code), format!("HTTP {code}"))
        }
        Err(err) => (false, format!("unreachable ({err})")),
    }
}

#[cfg(test)]
mod tests;
