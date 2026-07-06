//! Locating the fleet repo root.

use std::path::PathBuf;

use anyhow::{Context, Result, bail};

/// Walks up from the current directory to the nearest ancestor containing
/// `inventory/probes.toml`.
pub fn discover_repo_root_blocking() -> Result<PathBuf> {
    let cwd = std::env::current_dir().context("reading current directory")?;
    for dir in cwd.ancestors() {
        if dir.join("inventory").join("probes.toml").is_file() {
            return Ok(dir.to_path_buf());
        }
    }
    bail!(
        "no inventory/probes.toml found in {} or any parent; pass --repo",
        cwd.display()
    );
}
