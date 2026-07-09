//! The `report` subcommand: render the branded PDF report from the declared
//! data files and the newest status snapshot.

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::{Args, ValueEnum};
use rollcall_report::NarrativeProvider;

use crate::output::print_to_stdout;
use crate::repo::discover_repo_root_blocking;

/// How `report` narrates its prose sections.
#[derive(Debug, Clone, Copy, ValueEnum)]
enum NarrateArg {
    /// Draft prose with an on-device model if one is available, else render
    /// the authored editorial fragments.
    Auto,
    /// Never draft; render the authored editorial fragments only.
    Off,
    /// Draft prose and write it back into the editorial fragments for review
    /// and commit, then render it.
    Draft,
}

impl From<NarrateArg> for rollcall_report::NarrateMode {
    fn from(arg: NarrateArg) -> Self {
        match arg {
            NarrateArg::Auto => Self::Auto,
            NarrateArg::Off => Self::Off,
            NarrateArg::Draft => Self::Draft,
        }
    }
}

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

    /// Narrate the prose sections with an on-device model. `auto` (default)
    /// uses one if available and otherwise renders the authored fragments;
    /// `off` renders only the authored fragments; `draft` also writes the
    /// drafted prose back into the fragments for review.
    #[arg(long, value_enum, default_value = "auto")]
    narrate: NarrateArg,

    /// Write the LaTeX under reports/src/ and stop before XeLaTeX — for
    /// freshness diffs in CI and byte-level comparisons.
    #[arg(long)]
    tex_only: bool,
}

/// Picks an on-device narrative provider for `mode`, announcing the choice or
/// the (non-fatal) reason narration is unavailable.
fn resolve_provider(mode: rollcall_report::NarrateMode) -> Option<rollcall_report::AppleFm> {
    if mode == rollcall_report::NarrateMode::Off {
        return None;
    }
    match rollcall_report::AppleFm::detect() {
        Some(provider) => {
            tracing::info!("narrate: drafting prose with {}", provider.name());
            Some(provider)
        }
        None => {
            tracing::warn!(
                "narrate: no on-device provider (fm) — install fm on an Apple-silicon Mac with \
                 Apple Intelligence, or pass --narrate off; rendering the authored prose"
            );
            None
        }
    }
}

/// Runs the subcommand.
pub fn run_blocking(args: &ReportArgs) -> Result<ExitCode> {
    let repo = match &args.repo {
        Some(path) => path.clone(),
        None => discover_repo_root_blocking()?,
    };
    let inputs = rollcall_report::load_inputs_blocking(&repo)?;

    let mode: rollcall_report::NarrateMode = args.narrate.into();
    let provider = resolve_provider(mode);
    let tex = rollcall_report::render_document_blocking(
        &inputs,
        &repo.join("reports").join("editorial"),
        &args.number,
        mode,
        provider
            .as_ref()
            .map(|p| p as &dyn rollcall_report::NarrativeProvider),
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
