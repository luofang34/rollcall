//! Schema and loader for `inventory/probes.toml`.

use std::collections::HashSet;
use std::path::Path;

use serde::Deserialize;
use toml::Value;

use crate::error::InventoryError;
use crate::load;

/// One status probe definition from `inventory/probes.toml`.
#[derive(Debug, Clone)]
pub struct Probe {
    /// Stable identifier, unique within the file; keys snapshot entries.
    pub id: String,
    /// Human-readable description carried into snapshots and reports.
    pub desc: String,
    /// Whether the target is reachable from the LAN the probe runs on.
    /// A failing probe with `lan_reachable = false` is unverifiable, not down.
    pub lan_reachable: bool,
    /// The probe mechanism and its target.
    pub kind: ProbeKind,
}

/// The probe mechanism, selected by each entry's `kind` key.
#[derive(Debug, Clone)]
pub enum ProbeKind {
    /// ICMP echo via the system `ping`.
    Icmp {
        /// Host to ping (IP address or name).
        target: String,
    },
    /// HTTP GET; up when the response status is listed in `expect`.
    /// Redirects are not followed — a 301/302 from the service itself proves
    /// it is up, while following it could cross into a different listener.
    Http {
        /// URL to fetch.
        url: String,
        /// Acceptable response status codes.
        expect: Vec<u16>,
        /// Skip TLS certificate verification (self-signed endpoints).
        insecure: bool,
    },
}

// Kind-specific entry schemas. `deny_unknown_fields` works here where it
// cannot on a flattened, internally tagged enum; entries deserialize into
// these after `kind` is dispatched (and removed) by `parse_probe`.

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct IcmpEntry {
    id: String,
    desc: String,
    #[serde(default = "default_true")]
    lan_reachable: bool,
    target: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct HttpEntry {
    id: String,
    desc: String,
    #[serde(default = "default_true")]
    lan_reachable: bool,
    url: String,
    #[serde(default = "default_expect")]
    expect: Vec<u16>,
    #[serde(default)]
    insecure: bool,
}

fn default_true() -> bool {
    true
}

fn default_expect() -> Vec<u16> {
    vec![200]
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProbesFile {
    probe: Vec<toml::Table>,
}

/// Loads probe definitions from a `probes.toml` file, in file order.
pub fn load_probes_blocking(path: &Path) -> Result<Vec<Probe>, InventoryError> {
    let text = load::read_file_blocking(path)?;
    parse_probes(&text, path)
}

fn parse_probes(text: &str, path: &Path) -> Result<Vec<Probe>, InventoryError> {
    let file: ProbesFile = toml::from_str(text).map_err(|source| InventoryError::Parse {
        path: path.to_path_buf(),
        source: Box::new(source),
    })?;
    let mut seen = HashSet::new();
    let mut probes = Vec::with_capacity(file.probe.len());
    for (index, table) in file.probe.into_iter().enumerate() {
        let probe = parse_probe(table, index, path)?;
        if !seen.insert(probe.id.clone()) {
            return Err(InventoryError::DuplicateProbeId {
                path: path.to_path_buf(),
                id: probe.id,
            });
        }
        probes.push(probe);
    }
    Ok(probes)
}

fn parse_probe(mut table: toml::Table, index: usize, path: &Path) -> Result<Probe, InventoryError> {
    let entry = entry_label(index, &table);
    match table.remove("kind") {
        Some(Value::String(kind)) if kind == "icmp" => {
            let raw: IcmpEntry =
                Value::Table(table)
                    .try_into()
                    .map_err(|source| InventoryError::InvalidProbe {
                        path: path.to_path_buf(),
                        entry,
                        source: Box::new(source),
                    })?;
            Ok(Probe {
                id: raw.id,
                desc: raw.desc,
                lan_reachable: raw.lan_reachable,
                kind: ProbeKind::Icmp { target: raw.target },
            })
        }
        Some(Value::String(kind)) if kind == "http" => {
            let raw: HttpEntry =
                Value::Table(table)
                    .try_into()
                    .map_err(|source| InventoryError::InvalidProbe {
                        path: path.to_path_buf(),
                        entry,
                        source: Box::new(source),
                    })?;
            Ok(Probe {
                id: raw.id,
                desc: raw.desc,
                lan_reachable: raw.lan_reachable,
                kind: ProbeKind::Http {
                    url: raw.url,
                    expect: raw.expect,
                    insecure: raw.insecure,
                },
            })
        }
        Some(other) => Err(InventoryError::UnknownProbeKind {
            path: path.to_path_buf(),
            entry,
            kind: match other {
                Value::String(kind) => kind,
                value => value.to_string(),
            },
        }),
        None => Err(InventoryError::MissingProbeKind {
            path: path.to_path_buf(),
            entry,
        }),
    }
}

fn entry_label(index: usize, table: &toml::Table) -> String {
    match table.get("id").and_then(Value::as_str) {
        Some(id) => format!("probe {} (id {id:?})", index + 1),
        None => format!("probe {}", index + 1),
    }
}

#[cfg(test)]
mod tests;
