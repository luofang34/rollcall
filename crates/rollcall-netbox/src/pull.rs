//! Pulling the escrow: paginated reads of every owned endpoint with the
//! read-only reconciler token, canonicalized into one document.

use std::path::Path;
use std::time::Duration;

use serde_json::Value;
use tracing::info;

use crate::canonical::canonicalize_objects;
use crate::error::NetboxError;

/// The endpoints NetBox owns as source of truth. The escrow key is the
/// endpoint path with `/` replaced by `.` — stable, greppable.
/// Deliberately excluded: `users/*` (credentials never land in git).
pub const ENDPOINTS: &[&str] = &[
    "dcim/sites",
    "dcim/manufacturers",
    "dcim/device-types",
    "dcim/device-roles",
    "dcim/devices",
    "dcim/interfaces",
    "dcim/inventory-items",
    "ipam/prefixes",
    "ipam/ip-addresses",
    "ipam/vlans",
    "virtualization/cluster-types",
    "virtualization/clusters",
    "virtualization/virtual-machines",
    "virtualization/interfaces",
    "extras/custom-fields",
    "extras/custom-field-choice-sets",
];

/// Reads the API token from a file, trimmed. The token must be the
/// read-only reconciler token — never an admin one.
pub fn read_token_blocking(path: &Path) -> Result<String, NetboxError> {
    let text = std::fs::read_to_string(path).map_err(|source| NetboxError::TokenRead {
        path: path.to_path_buf(),
        source,
    })?;
    let token = text.trim().to_owned();
    if token.is_empty() {
        return Err(NetboxError::TokenEmpty {
            path: path.to_path_buf(),
        });
    }
    Ok(token)
}

/// Pulls every owned endpoint and returns the canonical escrow document.
/// Blocks on the NetBox API.
pub fn pull_blocking(base_url: &str, token: &str) -> Result<Value, NetboxError> {
    let agent = ureq::Agent::new_with_config(
        ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(30)))
            .http_status_as_error(false)
            .build(),
    );
    let base = base_url.trim_end_matches('/');
    let mut document = serde_json::Map::new();
    for endpoint in ENDPOINTS {
        let objects = fetch_all_blocking(&agent, base, token, endpoint)?;
        info!(endpoint, count = objects.len(), "pulled");
        document.insert(
            endpoint.replace('/', "."),
            Value::Array(canonicalize_objects(objects)),
        );
    }
    Ok(Value::Object(document))
}

/// Fetches one endpoint, following pagination until exhausted.
fn fetch_all_blocking(
    agent: &ureq::Agent,
    base: &str,
    token: &str,
    endpoint: &str,
) -> Result<Vec<Value>, NetboxError> {
    let mut url = format!("{base}/api/{endpoint}/?limit=500");
    let mut objects = Vec::new();
    loop {
        let mut response = agent
            .get(&url)
            .header("Authorization", &format!("Token {token}"))
            .header("Accept", "application/json")
            .call()
            .map_err(|source| NetboxError::Http {
                url: url.clone(),
                source: Box::new(source),
            })?;
        let status = response.status().as_u16();
        if status != 200 {
            return Err(NetboxError::UnexpectedResponse {
                url,
                detail: format!(
                    "HTTP {status} (is the reconciler token authorized for this endpoint?)"
                ),
            });
        }
        let text = response
            .body_mut()
            .read_to_string()
            .map_err(|source| NetboxError::Http {
                url: url.clone(),
                source: Box::new(source),
            })?;
        let body: Value = serde_json::from_str(&text).map_err(|source| NetboxError::Parse {
            url: url.clone(),
            source,
        })?;
        let results = body
            .get("results")
            .and_then(Value::as_array)
            .ok_or_else(|| NetboxError::UnexpectedResponse {
                url: url.clone(),
                detail: "no results array".to_owned(),
            })?;
        objects.extend(results.iter().cloned());
        match body.get("next").and_then(Value::as_str) {
            Some(next) => url = next.to_owned(),
            None => return Ok(objects),
        }
    }
}

#[cfg(test)]
mod tests;
