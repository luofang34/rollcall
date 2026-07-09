//! On-device narrative generation for the report's prose sections.
//!
//! By default the report *narrates*: its prose sections are drafted by an
//! on-device language model from a compact, authoritative digest the renderer
//! computes — the model is handed every number, never asked to derive one.
//! With no provider available (a non-Apple host, or Apple Intelligence off)
//! narration is skipped and the committed editorial fragments render as
//! authored, so a report is always produced.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

use rollcall_status::ProbeState;

use crate::error::ReportError;
use crate::inputs::ReportInputs;

/// How the report treats its prose sections.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NarrateMode {
    /// Draft with a provider if one is available, else render the committed
    /// editorial fragments. The default.
    #[default]
    Auto,
    /// Never draft; always render the committed editorial fragments.
    Off,
    /// Draft prose, write it back into the editorial fragments for review and
    /// commit, and render it.
    Draft,
}

/// A source of drafted prose for a report section.
pub trait NarrativeProvider {
    /// Draft prose for `section`, following `instruction`, grounded only in
    /// `context`. Returns `None` to fall back to the authored fragment.
    fn draft(&self, section: &str, instruction: &str, context: &str) -> Option<String>;

    /// A short human-readable name, for the note announcing narration.
    fn name(&self) -> &str;
}

/// Apple's on-device Foundation model, reached through the system `fm` CLI.
/// No linking or FFI: each section is one `fm respond` subprocess.
#[derive(Debug, Clone, Copy)]
pub struct AppleFm;

impl AppleFm {
    /// Detects a usable on-device model: the `fm` CLI must be installed and
    /// report the system model available. Returns `None` otherwise (any
    /// non-Apple host, or Apple Intelligence disabled).
    #[must_use]
    pub fn detect() -> Option<Self> {
        let output = Command::new("fm").arg("available").output().ok()?;
        let seen = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        seen.contains("System model available").then_some(Self)
    }
}

impl NarrativeProvider for AppleFm {
    fn name(&self) -> &str {
        "apple on-device (fm)"
    }

    fn draft(&self, _section: &str, instruction: &str, context: &str) -> Option<String> {
        // --greedy keeps a given digest close to reproducible; the on-device
        // `system` model is free, offline, and private.
        let output = Command::new("fm")
            .args([
                "respond",
                "--greedy",
                "--no-stream",
                "--model",
                "system",
                "--instructions",
                instruction,
                "--text",
                context,
            ])
            .arg("Write the section now, as plain prose.")
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let prose = clean(&String::from_utf8_lossy(&output.stdout));
        (!prose.is_empty()).then_some(prose)
    }
}

/// The instruction for a narratable section, or `None` when the section is
/// authored-only — the architecture note is structural and does not track
/// live status, so it is never model-drafted.
#[must_use]
pub fn instruction_for(fragment: &str) -> Option<&'static str> {
    let instruction = match fragment {
        "executive-summary.tex" => {
            "You are a precise datacenter operations analyst. Using ONLY the digest, write a 3-4 \
             sentence executive summary: probe health, any down hosts, and the modeled power and \
             monthly cost. Plain prose only — no title, no markdown, no lists."
        }
        "status-note.tex" => {
            "Using ONLY the digest, write 1-2 plain sentences interpreting the probe status: what \
             is up, what is down, and any implication. No title, no markdown."
        }
        "workloads-note.tex" => {
            "Using ONLY the digest, write 1-2 plain sentences on how the guests are distributed \
             across the hosts. No title, no markdown."
        }
        "topology-note.tex" => {
            "Using ONLY the digest, write 1-2 plain sentences on the fleet's hosts and their \
             roles. No title, no markdown."
        }
        "findings.tex" => {
            "You are a datacenter operations analyst. Using ONLY the digest, write 2-4 sentences \
             of findings about the fleet's current state and gaps, as flowing prose (not a list). \
             No title, no markdown."
        }
        _ => return None,
    };
    Some(instruction)
}

/// Draft a section if narration is active and a provider yields prose;
/// otherwise `None`, meaning render the authored fragment instead. The
/// `Draft`-mode write-back is the caller's job (it owns the editorial dir).
#[must_use]
pub fn render_section(
    mode: NarrateMode,
    provider: Option<&dyn NarrativeProvider>,
    fragment: &str,
    digest: &str,
) -> Option<String> {
    if mode == NarrateMode::Off {
        return None;
    }
    let instruction = instruction_for(fragment)?;
    provider?.draft(fragment, instruction, digest)
}

/// Writes drafted, LaTeX-ready prose back into an editorial fragment (used in
/// `Draft` mode so the operator can review and commit it).
pub fn write_fragment_blocking(dir: &Path, fragment: &str, tex: &str) -> Result<(), ReportError> {
    let path = dir.join(fragment);
    let body = format!("{}\n", tex.trim_end());
    std::fs::write(&path, body).map_err(|source| ReportError::WriteFragment { path, source })
}

/// Builds the compact, authoritative digest the model narrates from. Every
/// number is computed here — the model is never asked to count.
#[must_use]
pub fn fleet_digest(inputs: &ReportInputs, values: &BTreeMap<String, String>) -> String {
    let results = &inputs.snapshot.results;
    let up = results.iter().filter(|r| r.state == ProbeState::Up).count();
    let down: Vec<&str> = results
        .iter()
        .filter(|r| r.state == ProbeState::Down)
        .map(|r| r.desc.as_str())
        .collect();
    let hosts: Vec<(String, usize)> = inputs
        .devices
        .device
        .iter()
        .map(|device| {
            let guests = inputs
                .workloads
                .guest
                .iter()
                .find(|placement| placement.host == device.id)
                .map_or(0, |placement| placement.guests.len());
            (device.name.clone(), guests)
        })
        .collect();
    let guest_total: usize = hosts.iter().map(|(_, guests)| guests).sum();
    let val = |key: &str| values.get(key).map_or("?", String::as_str);
    format_digest(
        &inputs.status_date,
        &inputs.site.site.name,
        up,
        results.len(),
        &down,
        &hosts,
        guest_total,
        val("all_on_typical_kw"),
        val("all_on_usd_per_month"),
        val("tariff_usd_per_kwh"),
    )
}

/// Formats the digest. Split from [`fleet_digest`] so the wording is unit
/// testable without constructing a full [`ReportInputs`].
#[expect(
    clippy::too_many_arguments,
    reason = "a flat digest of computed scalars"
)]
#[must_use]
fn format_digest(
    date: &str,
    site: &str,
    up: usize,
    total: usize,
    down: &[&str],
    hosts: &[(String, usize)],
    guest_total: usize,
    power_kw: &str,
    usd_per_month: &str,
    tariff: &str,
) -> String {
    let down_line = if down.is_empty() {
        "none".to_owned()
    } else {
        down.join("; ")
    };
    let host_lines: Vec<String> = hosts
        .iter()
        .map(|(name, guests)| format!("- {name}: {guests} guests"))
        .collect();
    format!(
        "Fleet status digest for {site}, {date}.\n\
         Probes: {up} of {total} up. Down: {down_line}.\n\
         Hosts ({host_count}), guests total {guest_total}:\n{hosts}\n\
         Power: all-on typical {power_kw} kW, about ${usd_per_month}/month at ${tariff}/kWh.\n\
         Capex is seeded with estimates pending invoices. NetBox is the source of truth.",
        host_count = hosts.len(),
        hosts = host_lines.join("\n"),
    )
}

/// Strips the light markdown the model sometimes emits despite the
/// instruction, and trims — leaving plain prose ready for LaTeX escaping.
#[must_use]
fn clean(raw: &str) -> String {
    raw.replace("**", "")
        .replace("__", "")
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_owned()
}

#[cfg(test)]
mod tests;
