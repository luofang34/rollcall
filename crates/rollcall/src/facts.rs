//! The `facts` subcommand: sweep every device with a read-only command
//! battery over SSH and write the dated evidence under facts/.

use std::fmt::Write as _;
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::Args;
use rollcall_facts::{AccessState, collect_fleet_blocking, write_facts_blocking};
use rollcall_status::local_stamp;

use crate::output::print_to_stdout;
use crate::repo::discover_repo_root_blocking;

/// Arguments for `rollcall facts`.
#[derive(Debug, Args)]
pub struct FactsArgs {
    /// Fleet repo root; defaults to the nearest ancestor of the current
    /// directory containing inventory/probes.toml.
    #[arg(long, value_name = "DIR")]
    repo: Option<PathBuf>,
}

/// Runs the subcommand.
pub fn run_blocking(args: &FactsArgs) -> Result<ExitCode> {
    let repo = match &args.repo {
        Some(path) => path.clone(),
        None => discover_repo_root_blocking()?,
    };
    let devices =
        rollcall_inventory::load_devices_blocking(&repo.join("inventory").join("devices.toml"))?;

    let stamp = local_stamp();
    let facts = collect_fleet_blocking(&devices.device, stamp.generated_at);

    let mut report = String::new();
    for host in &facts.hosts {
        let summary = match (host.access, &host.facts, &host.error) {
            (AccessState::Ok, Some(f), _) => format!(
                "{} | {} | {} GPU(s) | {} guest(s)",
                f.motherboard.as_deref().unwrap_or("board unknown"),
                f.cpu_model.as_deref().unwrap_or("cpu unknown"),
                f.gpus.len(),
                f.guests.len()
            ),
            (AccessState::NoSshConfigured, ..) => "no ssh target configured".to_owned(),
            (_, _, Some(error)) => error.clone(),
            _ => String::new(),
        };
        writeln!(
            report,
            "{:<10} {:<18} {}",
            host.id,
            access_label(host.access),
            summary
        )
        .ok();
    }
    print_to_stdout(&report);

    let path = write_facts_blocking(&repo.join("facts"), &stamp.date, &facts)
        .context("writing fact sweep")?;
    print_to_stdout(&format!("\nwrote {}\n", path.display()));
    Ok(ExitCode::SUCCESS)
}

fn access_label(access: AccessState) -> &'static str {
    match access {
        AccessState::Ok => "ok",
        AccessState::NoSshConfigured => "no-ssh-configured",
        AccessState::Failed => "failed",
    }
}
