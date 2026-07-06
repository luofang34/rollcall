//! Power economics: per-device draw, monthly energy, and cost, joined with
//! the newest snapshot to distinguish all-on from live figures.

use rollcall_inventory::{DevicesFile, SiteFile};
use rollcall_status::{ProbeState, Snapshot};

/// Power economics for one device.
#[derive(Debug, Clone)]
pub struct PowerRow {
    /// Device display name.
    pub name: String,
    /// Modeled typical draw in watts.
    pub typical_w: f64,
    /// Modeled peak draw in watts.
    pub peak_w: f64,
    /// Energy at typical draw over one accounting month.
    pub kwh_per_month: f64,
    /// Cost of that energy at the site tariff.
    pub usd_per_month: f64,
    /// Whether the device counts as live (see [`power_model`]).
    pub is_up: bool,
}

/// The fleet-wide power model.
#[derive(Debug, Clone)]
pub struct PowerModel {
    /// One row per device, in `devices.toml` order.
    pub rows: Vec<PowerRow>,
    /// Sum of typical draws with everything on.
    pub all_on_typical_w: f64,
    /// Sum of typical draws over live devices only.
    pub live_typical_w: f64,
    /// Sum of peak draws — the sizing envelope.
    pub peak_w: f64,
}

/// Builds the model. A device counts as live unless a probe with the same id
/// reports down; unprobed devices count as live.
pub fn power_model(devices: &DevicesFile, site: &SiteFile, snapshot: &Snapshot) -> PowerModel {
    let mut rows = Vec::with_capacity(devices.device.len());
    let (mut all_on, mut live, mut peak) = (0.0, 0.0, 0.0);
    for dev in &devices.device {
        let down = snapshot
            .results
            .iter()
            .any(|r| r.id == dev.id && r.state == ProbeState::Down);
        let kwh = kwh_per_month(dev.power_typical_w, site);
        rows.push(PowerRow {
            name: dev.name.clone(),
            typical_w: dev.power_typical_w,
            peak_w: dev.power_peak_w,
            kwh_per_month: kwh,
            usd_per_month: kwh * site.power.tariff_usd_per_kwh,
            is_up: !down,
        });
        all_on += dev.power_typical_w;
        peak += dev.power_peak_w;
        if !down {
            live += dev.power_typical_w;
        }
    }
    PowerModel {
        rows,
        all_on_typical_w: all_on,
        live_typical_w: live,
        peak_w: peak,
    }
}

/// Monthly energy for a wattage at the site's accounting-month length.
pub fn kwh_per_month(watts: f64, site: &SiteFile) -> f64 {
    watts / 1000.0 * site.power.hours_per_month as f64
}

/// Monthly cost for a wattage at the site tariff.
pub fn usd_per_month(watts: f64, site: &SiteFile) -> f64 {
    kwh_per_month(watts, site) * site.power.tariff_usd_per_kwh
}

#[cfg(test)]
mod tests;
