//! Schema and loader for `finance/capex.toml`: the capital-expenditure
//! ledger (estimates until invoices land).

use std::path::Path;

use serde::Deserialize;

use crate::error::InventoryError;
use crate::load;

/// Top-level shape of `finance/capex.toml`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapexFile {
    /// Ledger line items.
    pub item: Vec<CapexItem>,
}

/// One capex line item.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapexItem {
    /// Device id the item belongs to (or `"site"`).
    pub device: String,
    /// Item description.
    pub desc: String,
    /// Cost in whole USD.
    pub usd: i64,
    /// `"estimate"` until replaced by an `"invoice"` value.
    pub basis: String,
}

/// Loads `capex.toml`.
pub fn load_capex_blocking(path: &Path) -> Result<CapexFile, InventoryError> {
    load::load_toml_blocking(path)
}

#[cfg(test)]
mod tests;
