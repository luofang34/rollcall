//! The `report` subcommand: render the branded PDF report from the declared
//! data files and the newest status snapshot.

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::Args;

use crate::output::print_to_stdout;
use crate::repo::discover_repo_root_blocking;

/// Arguments for `rollcall report`.
#[derive(Debug, Args)]
pub struct ReportArgs {
    /// Fleet repo root; defaults to the nearest ancestor of the current
    /// directory containing inventory/probes.toml.
    #[arg(long, value_name = "DIR")]
    repo: Option<PathBuf>,

    /// Report number embedded in the document and output filenames.
    #[arg(long, default_value = "INF-0001")]
    number: String,

    /// Write the LaTeX under reports/src/ and stop before XeLaTeX — for
    /// freshness diffs in CI and byte-level comparisons.
    #[arg(long)]
    tex_only: bool,
}

/// Runs the subcommand.
pub fn run_blocking(args: &ReportArgs) -> Result<ExitCode> {
    let repo = match &args.repo {
        Some(path) => path.clone(),
        None => discover_repo_root_blocking()?,
    };
    let inputs = rollcall_report::load_inputs_blocking(&repo)?;
    let tex = rollcall_report::render_document_blocking(
        &inputs,
        &repo.join("reports").join("editorial"),
        &args.number,
    )?;
    let tex_path =
        rollcall_report::write_tex_blocking(&repo, &args.number, &inputs.status_date, &tex)
            .context("writing report LaTeX")?;
    if args.tex_only {
        print_to_stdout(&format!("wrote {}\n", tex_path.display()));
        return Ok(ExitCode::SUCCESS);
    }
    let pdf_path = rollcall_report::compile_pdf_blocking(&repo, &tex_path)?;
    print_to_stdout(&format!("wrote {}\n", pdf_path.display()));
    Ok(ExitCode::SUCCESS)
}
