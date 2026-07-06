//! The `netbox` subcommand: git-escrow operations against the NetBox
//! DCIM/IPAM source of truth.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use clap::{Args, Subcommand};

use crate::output::print_to_stdout;
use crate::repo::discover_repo_root_blocking;

/// Arguments for `rollcall netbox`.
#[derive(Debug, Args)]
pub struct NetboxArgs {
    #[command(subcommand)]
    action: NetboxAction,
}

#[derive(Debug, Subcommand)]
enum NetboxAction {
    /// Pull every NetBox-owned endpoint into the canonical committed
    /// export netbox/declared.json. Reconcile treats the committed export
    /// as declared state, so a NetBox edit becomes declared by landing
    /// this file's diff in git.
    Pull(PullArgs),
    /// Recreate the committed escrow's objects in NetBox, idempotently.
    /// The recovery path after a rebuild — needs an admin (write) token.
    Restore(RestoreArgs),
    /// Prove the NetBox database is not a pet: pull, wipe every owned
    /// object, restore from the just-pulled escrow, pull again, and assert
    /// the two are logically equal (ids differ, content does not). Needs an
    /// admin token. Refuses unless --yes is given.
    Drill(DrillArgs),
}

#[derive(Debug, Args)]
struct PullArgs {
    /// Fleet repo root; defaults to the nearest ancestor of the current
    /// directory containing inventory/probes.toml.
    #[arg(long, value_name = "DIR")]
    repo: Option<PathBuf>,

    /// File holding the read-only reconciler API token.
    #[arg(long, value_name = "FILE")]
    token_file: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct RestoreArgs {
    /// Fleet repo root; defaults to the nearest ancestor of the current
    /// directory containing inventory/probes.toml.
    #[arg(long, value_name = "DIR")]
    repo: Option<PathBuf>,

    /// File holding an admin (write-enabled) API token.
    #[arg(long, value_name = "FILE")]
    admin_token_file: PathBuf,
}

#[derive(Debug, Args)]
struct DrillArgs {
    /// Fleet repo root; defaults to the nearest ancestor of the current
    /// directory containing inventory/probes.toml.
    #[arg(long, value_name = "DIR")]
    repo: Option<PathBuf>,

    /// File holding an admin (write-enabled) API token.
    #[arg(long, value_name = "FILE")]
    admin_token_file: PathBuf,

    /// Required acknowledgement: the drill wipes and rebuilds all NetBox
    /// DCIM/IPAM/virtualization objects.
    #[arg(long)]
    yes: bool,
}

/// Runs the subcommand.
pub fn run_blocking(args: &NetboxArgs) -> Result<ExitCode> {
    match &args.action {
        NetboxAction::Pull(pull) => pull_blocking(pull),
        NetboxAction::Restore(restore) => restore_blocking(restore),
        NetboxAction::Drill(drill) => drill_blocking(drill),
    }
}

fn resolve(repo: &Option<PathBuf>) -> Result<(PathBuf, String)> {
    let repo = match repo {
        Some(path) => path.clone(),
        None => discover_repo_root_blocking()?,
    };
    let site = rollcall_inventory::load_site_blocking(&repo.join("inventory").join("site.toml"))?;
    let url = site
        .services
        .map(|s| s.netbox_url)
        .context("no [services] netbox_url in inventory/site.toml")?;
    Ok((repo, url))
}

fn read_escrow(repo: &Path) -> Result<serde_json::Value> {
    let path = repo.join("netbox").join("declared.json");
    let text = std::fs::read_to_string(&path).with_context(|| {
        format!(
            "reading {} — run `rollcall netbox pull` first",
            path.display()
        )
    })?;
    serde_json::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}

fn restore_blocking(args: &RestoreArgs) -> Result<ExitCode> {
    let (repo, url) = resolve(&args.repo)?;
    let token = rollcall_netbox::read_token_blocking(&args.admin_token_file)?;
    let escrow = read_escrow(&repo)?;
    let mut session = rollcall_netbox::Restore::new(&url, &token);
    let report = session.run_blocking(&escrow)?;
    print_to_stdout(&format!(
        "restore: {} created, {} already present\n",
        report.created, report.existing
    ));
    Ok(ExitCode::SUCCESS)
}

fn drill_blocking(args: &DrillArgs) -> Result<ExitCode> {
    let (_repo, url) = resolve(&args.repo)?;
    if !args.yes {
        print_to_stdout("refusing: the drill wipes and rebuilds all NetBox objects; pass --yes\n");
        return Ok(ExitCode::FAILURE);
    }
    let token = rollcall_netbox::read_token_blocking(&args.admin_token_file)?;

    // Pull the pre-drill state directly from the API (not the committed
    // file) so the drill proves a round-trip of live truth.
    print_to_stdout("1/4 pulling pre-drill state...\n");
    let before = rollcall_netbox::pull_blocking(&url, &token)?;
    let before_logical = rollcall_netbox::logical_view(&before);

    print_to_stdout("2/4 wiping all owned objects...\n");
    let deleted = rollcall_netbox::wipe_blocking(&url, &token)?;
    print_to_stdout(&format!("    deleted {deleted} objects\n"));

    print_to_stdout("3/4 restoring from the pulled escrow...\n");
    let mut session = rollcall_netbox::Restore::new(&url, &token);
    let report = session.run_blocking(&before)?;
    print_to_stdout(&format!("    restore: {} created\n", report.created));

    print_to_stdout("4/4 pulling post-restore state and comparing logically...\n");
    let after = rollcall_netbox::pull_blocking(&url, &token)?;
    let after_logical = rollcall_netbox::logical_view(&after);

    if before_logical == after_logical {
        print_to_stdout(
            "\nDRILL PASSED: restore -> pull is a logical no-op (the NetBox DB is not a pet)\n",
        );
        Ok(ExitCode::SUCCESS)
    } else {
        let (b, a) = first_logical_diff(&before_logical, &after_logical);
        print_to_stdout(&format!(
            "\nDRILL FAILED: logical difference after restore\n  before: {b}\n  after:  {a}\n"
        ));
        Ok(ExitCode::FAILURE)
    }
}

/// First endpoint whose logical object lists differ, with a short sample.
fn first_logical_diff(before: &serde_json::Value, after: &serde_json::Value) -> (String, String) {
    let (bo, ao) = (before.as_object(), after.as_object());
    if let (Some(bo), Some(ao)) = (bo, ao) {
        for (endpoint, b_list) in bo {
            let a_list = ao.get(endpoint);
            if a_list != Some(b_list) {
                let b_len = b_list.as_array().map_or(0, Vec::len);
                let a_len = a_list
                    .and_then(serde_json::Value::as_array)
                    .map_or(0, Vec::len);
                return (
                    format!("{endpoint}: {b_len} objects"),
                    format!("{endpoint}: {a_len} objects"),
                );
            }
        }
    }
    ("<none>".to_owned(), "<none>".to_owned())
}

fn pull_blocking(args: &PullArgs) -> Result<ExitCode> {
    let repo = match &args.repo {
        Some(path) => path.clone(),
        None => discover_repo_root_blocking()?,
    };
    let site = rollcall_inventory::load_site_blocking(&repo.join("inventory").join("site.toml"))?;
    let Some(services) = &site.services else {
        bail!(
            "no [services] netbox_url in inventory/site.toml — declare the NetBox instance first"
        );
    };
    let token_path = match &args.token_file {
        Some(path) => path.clone(),
        None => default_token_path()?,
    };
    let token = rollcall_netbox::read_token_blocking(&token_path)?;

    let document = rollcall_netbox::pull_blocking(&services.netbox_url, &token)?;
    let mut summary = String::new();
    if let Some(map) = document.as_object() {
        use std::fmt::Write as _;
        for (endpoint, objects) in map {
            let count = objects.as_array().map_or(0, Vec::len);
            writeln!(summary, "{endpoint:<40} {count}").ok();
        }
    }
    print_to_stdout(&summary);

    let dir = repo.join("netbox");
    std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    let path = dir.join("declared.json");
    let mut json = serde_json::to_string_pretty(&document).context("serializing escrow")?;
    json.push('\n');
    std::fs::write(&path, json).with_context(|| format!("writing {}", path.display()))?;
    print_to_stdout(&format!(
        "\nwrote {} — commit its diff to make the state declared\n",
        path.display()
    ));
    Ok(ExitCode::SUCCESS)
}

/// `~/.config/fleet/netbox-token`, outside the repo: credentials never land
/// in git (the same rule that keeps `users/*` out of the escrow).
fn default_token_path() -> Result<PathBuf> {
    let home = std::env::var_os("HOME").context("HOME is not set; pass --token-file")?;
    Ok(PathBuf::from(home)
        .join(".config")
        .join("fleet")
        .join("netbox-token"))
}
