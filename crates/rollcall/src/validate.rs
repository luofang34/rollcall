//! The `validate` subcommand: check the declared inventory for internal
//! allocation collisions (VMID double-booked on a host, IP claimed twice)
//! without needing a fact sweep. This is the collision gate that runs in
//! front of every allocation; CI runs it, and so does `reconcile` before it
//! diffs against live.

use std::fmt::Write as _;
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::Result;
use clap::Args;

use crate::declared::{DeclaredSource, load_declared_blocking};
use crate::output::print_to_stdout;
use crate::repo::discover_repo_root_blocking;

/// Arguments for `rollcall validate`.
#[derive(Debug, Args)]
pub struct ValidateArgs {
    /// Fleet repo root; defaults to the nearest ancestor of the current
    /// directory containing inventory/probes.toml.
    #[arg(long, value_name = "DIR")]
    repo: Option<PathBuf>,

    /// Where declared inventory is read from: the NetBox escrow (default) or
    /// the TOML files.
    #[arg(long, value_enum, default_value_t = DeclaredSource::default())]
    declared: DeclaredSource,
}

/// Runs the subcommand.
pub fn run_blocking(args: &ValidateArgs) -> Result<ExitCode> {
    let repo = match &args.repo {
        Some(path) => path.clone(),
        None => discover_repo_root_blocking()?,
    };
    let (devices, workloads) = load_declared_blocking(&repo, args.declared)?;

    let conflicts = rollcall_inventory::validate_registry(&devices, &workloads);
    print_to_stdout(&render(workloads.guest_record.len(), &conflicts));
    if conflicts.is_empty() {
        Ok(ExitCode::SUCCESS)
    } else {
        Ok(ExitCode::FAILURE)
    }
}

fn render(guest_count: usize, conflicts: &[rollcall_inventory::RegistryConflict]) -> String {
    let mut out = String::new();
    if conflicts.is_empty() {
        writeln!(
            out,
            "registry ok: {guest_count} declared guests, no VMID or IP collisions"
        )
        .ok();
    } else {
        for conflict in conflicts {
            writeln!(out, "COLLISION {conflict}").ok();
        }
        writeln!(
            out,
            "\n{} collision(s) across {guest_count} declared guests",
            conflicts.len()
        )
        .ok();
    }
    out
}
