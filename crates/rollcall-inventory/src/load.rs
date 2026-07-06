//! Shared file-reading and TOML-parsing plumbing for the schema loaders.

use std::path::Path;

use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::error::InventoryError;

pub(crate) fn read_file_blocking(path: &Path) -> Result<String, InventoryError> {
    std::fs::read_to_string(path).map_err(|source| InventoryError::Read {
        path: path.to_path_buf(),
        source,
    })
}

pub(crate) fn load_toml_blocking<T: DeserializeOwned>(path: &Path) -> Result<T, InventoryError> {
    let text = read_file_blocking(path)?;
    toml::from_str(&text).map_err(|source| InventoryError::Parse {
        path: path.to_path_buf(),
        source: Box::new(source),
    })
}

/// Accepts a TOML integer or float for an `f64` field: the data files write
/// whole watt/BTU figures as integers (`power_peak_w = 60`).
pub(crate) fn int_or_float<'de, D: serde::Deserializer<'de>>(de: D) -> Result<f64, D::Error> {
    #[derive(serde::Deserialize)]
    #[serde(untagged)]
    enum Num {
        Int(i64),
        Float(f64),
    }
    Ok(match Num::deserialize(de)? {
        Num::Int(v) => v as f64,
        Num::Float(v) => v,
    })
}
