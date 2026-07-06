//! Schema and loader for `thermal/thermal.toml`: cooling-model assumptions.
//! Heat load itself derives from the device power figures.

use std::path::Path;

use serde::Deserialize;

use crate::error::InventoryError;
use crate::load;

/// Top-level shape of `thermal/thermal.toml`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThermalFile {
    /// Cooling-model constants.
    pub cooling: Cooling,
    /// Model caveats.
    pub assumptions: Assumptions,
}

/// Cooling-model constants.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cooling {
    /// Cooling method description.
    pub method: String,
    /// BTU/h of heat per watt of electrical load.
    #[serde(deserialize_with = "load::int_or_float")]
    pub btu_per_watt_hour: f64,
    /// BTU/h in one ton of refrigeration.
    #[serde(deserialize_with = "load::int_or_float")]
    pub ton_refrigeration_btu_h: f64,
}

/// Model caveats.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assumptions {
    /// Free-form caveat note.
    pub note: String,
}

/// Loads `thermal.toml`.
pub fn load_thermal_blocking(path: &Path) -> Result<ThermalFile, InventoryError> {
    load::load_toml_blocking(path)
}

#[cfg(test)]
mod tests;
