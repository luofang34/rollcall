//! Loading everything a report consumes: the declared data files and the
//! newest status snapshot.

use std::path::{Path, PathBuf};

use rollcall_inventory::{
    CapexFile, DevicesFile, SiteFile, ThermalFile, WorkloadsFile, load_capex_blocking,
    load_devices_blocking, load_site_blocking, load_thermal_blocking, load_workloads_blocking,
};
use rollcall_status::Snapshot;

use crate::error::ReportError;

/// Everything a report derives from.
#[derive(Debug, Clone)]
pub struct ReportInputs {
    /// Site identity and cost constants.
    pub site: SiteFile,
    /// Device inventory.
    pub devices: DevicesFile,
    /// Workloads and guest placement.
    pub workloads: WorkloadsFile,
    /// Cooling model.
    pub thermal: ThermalFile,
    /// Capex ledger.
    pub capex: CapexFile,
    /// The newest status snapshot.
    pub snapshot: Snapshot,
    /// The snapshot's date (its filename stem), used in the report title and
    /// output filenames.
    pub status_date: String,
}

/// Loads all report inputs from a fleet repo checkout.
pub fn load_inputs_blocking(repo: &Path) -> Result<ReportInputs, ReportError> {
    let inventory = repo.join("inventory");
    let (snapshot, status_date) = newest_snapshot_blocking(&repo.join("status"))?;
    Ok(ReportInputs {
        site: load_site_blocking(&inventory.join("site.toml"))?,
        devices: load_devices_blocking(&inventory.join("devices.toml"))?,
        workloads: load_workloads_blocking(&inventory.join("workloads.toml"))?,
        thermal: load_thermal_blocking(&repo.join("thermal").join("thermal.toml"))?,
        capex: load_capex_blocking(&repo.join("finance").join("capex.toml"))?,
        snapshot,
        status_date,
    })
}

/// Picks the lexicographically last `status/*.json` — dates are ISO, so that
/// is the newest snapshot.
fn newest_snapshot_blocking(dir: &Path) -> Result<(Snapshot, String), ReportError> {
    let entries = std::fs::read_dir(dir).map_err(|source| {
        // A status dir that never existed means no probe has run — point the
        // operator at the fix, not at the ENOENT.
        if source.kind() == std::io::ErrorKind::NotFound {
            ReportError::NoSnapshot {
                dir: dir.to_path_buf(),
            }
        } else {
            ReportError::ListSnapshots {
                dir: dir.to_path_buf(),
                source,
            }
        }
    })?;
    let mut paths: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    paths.sort();
    let Some(path) = paths.pop() else {
        return Err(ReportError::NoSnapshot {
            dir: dir.to_path_buf(),
        });
    };
    let text = std::fs::read_to_string(&path).map_err(|source| ReportError::ReadSnapshot {
        path: path.clone(),
        source,
    })?;
    let snapshot = serde_json::from_str(&text).map_err(|source| ReportError::ParseSnapshot {
        path: path.clone(),
        source,
    })?;
    let status_date = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    Ok((snapshot, status_date))
}
