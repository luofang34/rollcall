//! Document assembly: preamble, computed sections, and editorial fragments
//! in report order.

use std::collections::BTreeMap;
use std::path::Path;

use crate::editorial::render_fragment_blocking;
use crate::error::ReportError;
use crate::inputs::ReportInputs;
use crate::narrative::{self, NarrateMode, NarrativeProvider};
use crate::power::{PowerModel, power_model, usd_per_month};
use crate::sections;
use crate::tex::{esc, fmt_sep};

/// Renders the complete LaTeX document. Blocks on reading the editorial
/// fragments under `editorial_dir`.
///
/// `mode` and `provider` govern narration: in [`NarrateMode::Auto`] or
/// [`NarrateMode::Draft`] each status-driven prose section is drafted by
/// `provider` from a computed digest, falling back to the authored fragment
/// when there is no provider or it declines; [`NarrateMode::Off`] always
/// renders the authored fragments. [`NarrateMode::Draft`] also writes the
/// drafted prose back into the fragment files.
pub fn render_document_blocking(
    inputs: &ReportInputs,
    editorial_dir: &Path,
    number: &str,
    mode: NarrateMode,
    provider: Option<&dyn NarrativeProvider>,
) -> Result<String, ReportError> {
    let model = power_model(&inputs.devices, &inputs.site, &inputs.snapshot);
    let values = placeholder_values(inputs, &model);
    let digest = narrative::fleet_digest(inputs, &values);
    let fragment = |name: &str| -> Result<String, ReportError> {
        if let Some(prose) = narrative::render_section(mode, provider, name, &digest) {
            // Trailing newline so the prose is its own paragraph: an authored
            // fragment file ends with one, and a section whose note precedes a
            // table needs that break or the table sets inline (overflowing).
            let tex = format!("{}\n", esc(&prose));
            if mode == NarrateMode::Draft {
                narrative::write_fragment_blocking(editorial_dir, name, &tex)?;
            }
            return Ok(tex);
        }
        render_fragment_blocking(editorial_dir, name, &values)
    };

    let exec = fragment("executive-summary.tex")?;
    let status = sections::status_section(&inputs.snapshot, &fragment("status-note.tex")?);
    let devices = sections::devices_section(&inputs.devices);
    let workloads =
        sections::workloads_section(&inputs.workloads, &fragment("workloads-note.tex")?);
    let topology = sections::topology_section(
        &inputs.devices,
        &inputs.workloads,
        &inputs.site,
        &inputs.snapshot,
        &fragment("topology-note.tex")?,
    );
    let power = sections::power_section(&model, &inputs.site, &inputs.thermal);
    let capex = sections::capex_section(&inputs.capex, &model, &inputs.site);
    let architecture = fragment("architecture.tex")?;
    let findings = fragment("findings.tex")?;
    let preamble = preamble(&inputs.site, &inputs.status_date, number);

    Ok(format!(
        "{preamble}\\fleetheading{{Executive Summary}}\n{exec}\n{status}\n{devices}\n{workloads}\n{topology}\n{power}\n{capex}\n\n{architecture}\n\n{findings}\n\\end{{document}}\n"
    ))
}

/// The values the editorial fragments may reference as `@@key@@`, including
/// a `device_usd_per_month:<Name>` entry per device.
fn placeholder_values(inputs: &ReportInputs, model: &PowerModel) -> BTreeMap<String, String> {
    let results = &inputs.snapshot.results;
    let up = results
        .iter()
        .filter(|r| r.state == rollcall_status::ProbeState::Up)
        .count();
    let down: Vec<&str> = results
        .iter()
        .filter(|r| r.state == rollcall_status::ProbeState::Down)
        .map(|r| r.desc.as_str())
        .collect();
    let down_descs = if down.is_empty() {
        "none".to_owned()
    } else {
        esc(&down.join("; "))
    };

    let mut values = BTreeMap::new();
    values.insert("up_count".to_owned(), up.to_string());
    values.insert("probe_total".to_owned(), results.len().to_string());
    values.insert("down_descs".to_owned(), down_descs);
    values.insert(
        "generated_at".to_owned(),
        esc(&inputs.snapshot.generated_at),
    );
    values.insert(
        "all_on_typical_kw".to_owned(),
        format!("{:.2}", model.all_on_typical_w / 1000.0),
    );
    values.insert(
        "all_on_usd_per_month".to_owned(),
        fmt_sep(usd_per_month(model.all_on_typical_w, &inputs.site), 0),
    );
    values.insert(
        "tariff_usd_per_kwh".to_owned(),
        format!("{:.2}", inputs.site.power.tariff_usd_per_kwh),
    );
    for device in &inputs.devices.device {
        values.insert(
            format!("device_usd_per_month:{}", device.name),
            fmt_sep(usd_per_month(device.power_typical_w, &inputs.site), 0),
        );
    }
    values
}

fn preamble(site: &rollcall_inventory::SiteFile, status_date: &str, number: &str) -> String {
    format!(
        "\\documentclass[11pt]{{article}}\n\
         \\usepackage[a4paper, margin=2.2cm, top=3.0cm, bottom=2.6cm]{{geometry}}\n\
         \\usepackage{{fleet-identity}}\n\
         \\usepackage{{tikz}}\n\
         \\usetikzlibrary{{positioning}}\n\
         \\renewcommand{{\\FleetDocType}}{{INFRASTRUCTURE REPORT}}\n\
         \\renewcommand{{\\FleetDocNumber}}{{{number}}}\n\
         \\fleetsectionstyle\n\
         \\fleetpagestyle\n\
         % \\titlerule needs a titlesec context; outside one (plain body, as\n\
         % \\fleetheading uses it) XeTeX hits undefined \\ttl@makeline — same look,\n\
         % plain rule:\n\
         \\renewcommand{{\\fleetheading}}[1]{{%\n\
         \x20 \\par\\vspace{{1.2em}}%\n\
         \x20 {{\\large\\bfseries\\color{{primary}}#1}}\\par\n\
         \x20 \\vspace{{0.2em}}\\textcolor{{lightgray}}{{\\rule{{\\linewidth}}{{1.2pt}}}}\\par\n\
         \x20 \\vspace{{0.6em}}%\n\
         }}\n\
         \n\
         \\begin{{document}}\n\
         \\thispagestyle{{firstpage}}\n\
         \n\
         \\vspace*{{1.2em}}\n\
         {{\\Huge\\bfseries\\color{{primary}} Infrastructure Status Report}}\\\\[0.4em]\n\
         {{\\large\\color{{secondary}} {name} — {description}}}\\\\[0.3em]\n\
         {{\\color{{darkgray}}\\small {operator} \\quad|\\quad {status_date} \\quad|\\quad {number}}}\n\
         \n\
         \\vspace{{0.5em}}\\textcolor{{lightgray}}{{\\rule{{\\linewidth}}{{1.2pt}}}}\\vspace{{1em}}\n\
         \n",
        name = esc(&site.site.name),
        description = esc(&site.site.description),
        operator = esc(&site.site.operator),
    )
}
