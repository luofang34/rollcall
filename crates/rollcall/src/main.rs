//! Reconcile a declared fleet against observed reality: evidence sweeps,
//! declared-vs-actual reconciliation, and a NetBox git-escrow, for a small
//! on-prem fleet.

mod declared;
mod facts;
mod netbox;
mod output;
mod reconcile;
mod repo;
mod report;
mod status;
mod validate;

use std::process::ExitCode;

use clap::{Parser, Subcommand};
use tracing::error;

/// Reconcile a declared fleet against observed reality.
#[derive(Debug, Parser)]
#[command(name = "rollcall", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Probe every entry in inventory/probes.toml and write the dated
    /// snapshot under status/.
    Status(status::StatusArgs),
    /// Render the branded PDF report from the declared data and the newest
    /// snapshot.
    Report(report::ReportArgs),
    /// Sweep every device with a read-only SSH command battery and write
    /// the dated hardware evidence under facts/.
    Facts(facts::FactsArgs),
    /// Diff the newest fact sweep against the declared inventory; every
    /// check reports ok, drift, or unverified.
    Reconcile(reconcile::ReconcileArgs),
    /// Check the declared inventory for allocation collisions (a VMID
    /// double-booked on a host, an IP claimed twice) without a fact sweep.
    Validate(validate::ValidateArgs),
    /// Git-escrow operations against the NetBox DCIM/IPAM source of truth
    /// (pull the canonical committed export).
    Netbox(netbox::NetboxArgs),
}

fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();
    let result = match cli.command {
        Command::Status(args) => status::run_blocking(&args),
        Command::Report(args) => report::run_blocking(&args),
        Command::Facts(args) => facts::run_blocking(&args),
        Command::Reconcile(args) => reconcile::run_blocking(&args),
        Command::Validate(args) => validate::run_blocking(&args),
        Command::Netbox(args) => netbox::run_blocking(&args),
    };
    match result {
        Ok(code) => code,
        Err(err) => {
            error!("{err:#}");
            ExitCode::FAILURE
        }
    }
}
