//! The `reconcile` subcommand: diff the newest fact sweep against the
//! declared inventory and report every check as ok, drift, or unverified.

use std::fmt::Write as _;
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::Result;
use clap::Args;
use rollcall_facts::{CheckState, load_newest_facts_blocking, reconcile};

use crate::declared::{DeclaredSource, load_declared_blocking};
use crate::output::print_to_stdout;
use crate::repo::discover_repo_root_blocking;

/// Arguments for `rollcall reconcile`.
#[derive(Debug, Args)]
pub struct ReconcileArgs {
    /// Fleet repo root; defaults to the nearest ancestor of the current
    /// directory containing inventory/probes.toml.
    #[arg(long, value_name = "DIR")]
    repo: Option<PathBuf>,

    /// Where declared inventory is read from: the NetBox escrow (default) or
    /// the TOML files.
    #[arg(long, value_enum, default_value_t = DeclaredSource::default())]
    declared: DeclaredSource,

    /// Exit non-zero when any check drifts. Without it drift is a finding,
    /// not a command failure (strict suits CI gating).
    #[arg(long)]
    strict: bool,
}

/// Runs the subcommand.
pub fn run_blocking(args: &ReconcileArgs) -> Result<ExitCode> {
    let repo = match &args.repo {
        Some(path) => path.clone(),
        None => discover_repo_root_blocking()?,
    };
    let (devices, workloads) = load_declared_blocking(&repo, args.declared)?;

    // A declared allocation collision means the inventory contradicts itself;
    // there is nothing coherent to diff against live. Fail before the sweep,
    // regardless of --strict.
    let conflicts = rollcall_inventory::validate_registry(&devices, &workloads);
    if !conflicts.is_empty() {
        let mut report = String::new();
        for conflict in &conflicts {
            writeln!(report, "COLLISION {conflict}").ok();
        }
        writeln!(
            report,
            "\n{} declared allocation collision(s)",
            conflicts.len()
        )
        .ok();
        print_to_stdout(&report);
        return Ok(ExitCode::FAILURE);
    }

    let (facts, facts_path) = load_newest_facts_blocking(&repo.join("facts"))?;

    let results = reconcile(&devices, &workloads, &facts);
    let mut report = String::new();
    for r in &results {
        writeln!(
            report,
            "{:<8} {:<12} {:<10} {}",
            r.device, r.check, r.state, r.detail
        )
        .ok();
    }
    let count = |s: CheckState| results.iter().filter(|r| r.state == s).count();
    let (ok, drift, unverified) = (
        count(CheckState::Ok),
        count(CheckState::Drift),
        count(CheckState::Unverified),
    );
    writeln!(
        report,
        "\n{ok} ok, {drift} drift, {unverified} unverified (facts: {})",
        facts_path.display()
    )
    .ok();
    print_to_stdout(&report);

    if args.strict && drift > 0 {
        return Ok(ExitCode::FAILURE);
    }
    Ok(ExitCode::SUCCESS)
}
