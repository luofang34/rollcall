//! A prioritized risk register: a computed problem/gap context turned into
//! severity-ranked, actionable items by a narrative provider, rendered as a
//! styled table. Present only when narration is active and the model returns
//! grounded risks — the deterministic tables remain the authoritative record;
//! this section is the model's assessment layered on top.

use rollcall_status::ProbeState;

use crate::inputs::ReportInputs;
use crate::narrative::{NarrativeProvider, is_grounded};
use crate::tex::esc;

/// One assessed risk: a statement, its severity, and a recommended action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Risk {
    /// Short risk statement.
    pub title: String,
    /// Assessed severity.
    pub severity: Severity,
    /// Recommended action.
    pub recommendation: String,
}

/// Risk severity, ordered by urgency.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// Highest urgency.
    Critical,
    /// High urgency.
    High,
    /// Medium urgency.
    Medium,
    /// Low urgency.
    Low,
}

impl Severity {
    /// Sort rank — lower sorts first (most urgent).
    fn rank(self) -> u8 {
        match self {
            Self::Critical => 0,
            Self::High => 1,
            Self::Medium => 2,
            Self::Low => 3,
        }
    }

    /// Parses a model-provided severity word; unknowns default to medium.
    fn parse(word: &str) -> Self {
        match word.trim().to_ascii_lowercase().as_str() {
            "critical" => Self::Critical,
            "high" => Self::High,
            "low" => Self::Low,
            _ => Self::Medium,
        }
    }

    /// A colored LaTeX badge for the table.
    fn badge(self) -> &'static str {
        match self {
            Self::Critical => r"\textcolor{accent}{\textbf{CRITICAL}}",
            Self::High => r"\textcolor{accent}{\textbf{HIGH}}",
            Self::Medium => r"\textcolor{secondary}{\textbf{MEDIUM}}",
            Self::Low => r"\textcolor{darkgray}{\textbf{LOW}}",
        }
    }
}

/// The JSON schema the model fills — a list of `{title, severity,
/// recommendation}`, as `fm schema object` produces it.
const RISK_SCHEMA: &str = r##"{"properties":{"risks":{"items":{"$ref":"#/$defs/Risk"},"type":"array"}},"$defs":{"Risk":{"title":"Risk","additionalProperties":false,"properties":{"recommendation":{"type":"string"},"title":{"type":"string"},"severity":{"description":"one of: critical, high, medium, low","type":"string"}},"type":"object","x-order":["title","severity","recommendation"],"required":["title","severity","recommendation"]}},"x-order":["risks"],"type":"object","title":"RiskRegister","additionalProperties":false,"required":["risks"]}"##;

const INSTRUCTION: &str = "You are a senior SRE. From ONLY these observed facts, produce a \
    prioritized risk register: for each, a short title, a severity (critical, high, medium, or \
    low), and a specific recommendation. A deliberately-down node is expected — rate it low or \
    omit it. Invent nothing; do not repeat the same risk.";

/// Generates the prioritized risk register. Empty when there is no provider,
/// no problems to assess, or the model returns nothing usable.
#[must_use]
pub fn generate_blocking(
    provider: Option<&dyn NarrativeProvider>,
    inputs: &ReportInputs,
) -> Vec<Risk> {
    match provider {
        Some(provider) => assess(provider, &problem_context(inputs)),
        None => Vec::new(),
    }
}

/// Assesses a computed problem `context` into a sorted, ground-checked risk
/// register. Split from [`generate_blocking`] so it is testable with a fake
/// provider and a plain context string.
fn assess(provider: &dyn NarrativeProvider, context: &str) -> Vec<Risk> {
    if context.is_empty() {
        return Vec::new();
    }
    let Some(json) = provider.draft_json(INSTRUCTION, context, RISK_SCHEMA) else {
        return Vec::new();
    };
    let mut risks: Vec<Risk> = parse_risks(&json)
        .into_iter()
        .filter(|risk| is_grounded(&format!("{} {}", risk.title, risk.recommendation), context))
        .collect();
    risks.sort_by_key(|risk| risk.severity.rank());
    risks
}

/// True when a probe's target device (joined by id) is declared
/// `expected_offline` — a down state for it is by design, not a fault.
fn is_expected_offline(inputs: &ReportInputs, probe_id: &str) -> bool {
    inputs
        .devices
        .device
        .iter()
        .any(|device| device.id == probe_id && device.expected_offline)
}

/// Builds the computed problem/gap context from real data. The model ranks and
/// phrases; every line here is a fact rollcall observed or declared.
fn problem_context(inputs: &ReportInputs) -> String {
    let mut items: Vec<String> = Vec::new();
    for result in &inputs.snapshot.results {
        match result.state {
            ProbeState::Down if is_expected_offline(inputs, &result.id) => items.push(format!(
                "{} is down, but deliberately offline — expected, not an incident",
                result.desc
            )),
            ProbeState::Down => {
                items.push(format!("{} is DOWN ({})", result.desc, result.detail));
            }
            ProbeState::Unverifiable => {
                items.push(format!("{} is UNVERIFIABLE from the LAN", result.desc));
            }
            ProbeState::Up => {}
        }
    }
    if inputs
        .devices
        .device
        .iter()
        .any(|device| device.power_estimate)
    {
        items.push("Power draw is TDP-modeled, not metered (no PDU metering).".to_owned());
    }
    if inputs
        .capex
        .item
        .iter()
        .any(|line| line.basis == "estimate")
    {
        items.push("Capex is seeded with estimates; no invoices recorded yet.".to_owned());
    }
    for device in &inputs.devices.device {
        if device.source_of_truth.as_deref().unwrap_or("none") == "none" {
            items.push(format!(
                "{} is not registered in the Backstage catalog.",
                device.name
            ));
        }
        if device.ssh.is_none() {
            items.push(format!(
                "{} has no sweep credentials — its hardware facts are unverified.",
                device.name
            ));
        }
    }
    items.join("\n")
}

/// Parses the model's JSON into risks, dropping any malformed entry.
fn parse_risks(json: &str) -> Vec<Risk> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else {
        return Vec::new();
    };
    let Some(array) = value.get("risks").and_then(serde_json::Value::as_array) else {
        return Vec::new();
    };
    array
        .iter()
        .filter_map(|item| {
            let title = item.get("title")?.as_str()?.trim().to_owned();
            let recommendation = item.get("recommendation")?.as_str()?.trim().to_owned();
            let severity = Severity::parse(item.get("severity")?.as_str()?);
            (!title.is_empty()).then_some(Risk {
                title,
                severity,
                recommendation,
            })
        })
        .collect()
}

/// Renders the risk register as a styled section, or an empty string when
/// there are no risks (so the caller can splice it unconditionally).
#[must_use]
pub fn render(risks: &[Risk]) -> String {
    if risks.is_empty() {
        return String::new();
    }
    let rows = risks
        .iter()
        .map(|risk| {
            format!(
                "{} & {} & {} \\\\",
                esc(&risk.title),
                risk.severity.badge(),
                esc(&risk.recommendation)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "\n\\section{{Risk Register}}\nPrioritized from the observed status and declared gaps — the model ranks and recommends; the facts are rollcall's.\n\n\\begin{{tabularx}}{{\\textwidth}}{{X l X}}\n\\toprule\n\\textbf{{Risk}} & \\textbf{{Severity}} & \\textbf{{Recommendation}} \\\\\n\\midrule\n{rows}\n\\bottomrule\n\\end{{tabularx}}\n"
    )
}

#[cfg(test)]
mod tests;
