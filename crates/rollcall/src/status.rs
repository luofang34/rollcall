//! The `status` subcommand: probe the fleet, print the outcome, write the
//! dated snapshot consumed by the report builder.

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::Args;
use rollcall_status::{ProbeState, Snapshot, local_stamp, write_snapshot_blocking};

use crate::output::print_to_stdout;
use crate::repo::discover_repo_root_blocking;

/// Arguments for `rollcall status`.
#[derive(Debug, Args)]
pub struct StatusArgs {
    /// Fleet repo root; defaults to the nearest ancestor of the current
    /// directory containing inventory/probes.toml.
    #[arg(long, value_name = "DIR")]
    repo: Option<PathBuf>,

    /// Exit non-zero when any probe is down. Without it a down host is a
    /// finding, not a command failure (the default suits `make status`;
    /// strict suits cron/CI wiring that should page).
    #[arg(long)]
    strict: bool,
}

/// Runs the subcommand.
pub fn run_blocking(args: &StatusArgs) -> Result<ExitCode> {
    let repo = match &args.repo {
        Some(path) => path.clone(),
        None => discover_repo_root_blocking()?,
    };
    let probes_path = repo.join("inventory").join("probes.toml");
    let probes = rollcall_inventory::load_probes_blocking(&probes_path)?;

    let stamp = local_stamp();
    let snapshot = Snapshot {
        generated_at: stamp.generated_at,
        results: rollcall_status::run_probes_blocking(&probes)?,
    };
    print_to_stdout(&render_report(&snapshot));

    let path = write_snapshot_blocking(&repo.join("status"), &stamp.date, &snapshot)
        .context("writing status snapshot")?;
    print_to_stdout(&format!("wrote {}\n", path.display()));

    let any_down = snapshot.results.iter().any(|r| r.state == ProbeState::Down);
    if args.strict && any_down {
        return Ok(ExitCode::FAILURE);
    }
    Ok(ExitCode::SUCCESS)
}

fn render_report(snapshot: &Snapshot) -> String {
    use std::fmt::Write as _;

    let mut report = String::new();
    for result in &snapshot.results {
        writeln!(
            report,
            "{:<16} {:<12} {}",
            result.id, result.state, result.detail
        )
        .ok();
    }

    let down: Vec<&str> = snapshot
        .results
        .iter()
        .filter(|r| r.state == ProbeState::Down)
        .map(|r| r.id.as_str())
        .collect();
    let up = snapshot
        .results
        .iter()
        .filter(|r| r.state == ProbeState::Up)
        .count();
    let down_list = if down.is_empty() {
        String::new()
    } else {
        format!(", down: {}", down.join(", "))
    };
    writeln!(report, "\n{up}/{} up{}", snapshot.results.len(), down_list).ok();
    report
}
