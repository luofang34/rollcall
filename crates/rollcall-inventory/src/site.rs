//! Schema and loader for `inventory/site.toml`: site identity, power
//! constants, and network ranges.

use std::path::Path;

use serde::Deserialize;

use crate::error::InventoryError;
use crate::load;

/// Top-level shape of `inventory/site.toml`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SiteFile {
    /// Site identity.
    pub site: SiteIdentity,
    /// Power-cost constants everything cost-related derives from.
    pub power: PowerConstants,
    /// Network ranges.
    pub networks: Networks,
    /// Deployed fleet services the tooling talks to.
    pub services: Option<Services>,
}

/// Deployed fleet services the tooling talks to.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Services {
    /// Base URL of the NetBox instance the escrow pulls from.
    pub netbox_url: String,
}

/// Who and what the site is.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SiteIdentity {
    /// Site name.
    pub name: String,
    /// Operating entity.
    pub operator: String,
    /// One-line description.
    pub description: String,
}

/// Power-cost constants.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PowerConstants {
    /// Electricity tariff in USD per kWh.
    #[serde(deserialize_with = "load::int_or_float")]
    pub tariff_usd_per_kwh: f64,
    /// Accounting-month length in hours.
    pub hours_per_month: i64,
}

/// Network ranges.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Networks {
    /// LAN CIDR.
    pub lan: String,
    /// Storage/observability fabric CIDR.
    pub fabric: String,
    /// WAN description.
    pub wan: String,
    /// Out-of-band management CIDR (BMCs), where declared.
    pub mgmt: Option<String>,
}

/// Loads `site.toml`.
pub fn load_site_blocking(path: &Path) -> Result<SiteFile, InventoryError> {
    load::load_toml_blocking(path)
}

#[cfg(test)]
mod tests;
