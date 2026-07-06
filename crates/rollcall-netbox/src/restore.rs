//! Restore: recreate the escrow's objects in a (possibly empty) NetBox,
//! idempotently, so a rebuilt database converges to the committed escrow.
//! This is the write path — it needs an admin token — and the proof that
//! the NetBox database is not a pet: `restore(escrow) → pull → logical
//! no-op`.
//!
//! Objects are created in dependency order; references (device_type, role,
//! site, cluster, assigned interface, …) are re-resolved to the freshly
//! assigned ids as each tier is created. Idempotent: every object is
//! GET-by-natural-key first, so a partial restore can be re-run.

use std::collections::HashMap;
use std::time::Duration;

use serde_json::{Map, Value, json};
use tracing::info;

use crate::error::NetboxError;

/// A live restore session against one NetBox instance.
pub struct Restore {
    agent: ureq::Agent,
    base: String,
    token: String,
    /// (endpoint, natural key) → freshly assigned id.
    ids: HashMap<(String, String), i64>,
}

/// Summary of what a restore created or found already present.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct RestoreReport {
    /// Objects newly created.
    pub created: usize,
    /// Objects already present (idempotent skips).
    pub existing: usize,
}

impl Restore {
    /// Opens a restore session. The token must be able to write.
    pub fn new(base_url: &str, token: &str) -> Self {
        Self {
            agent: ureq::Agent::new_with_config(
                ureq::Agent::config_builder()
                    .timeout_global(Some(Duration::from_secs(30)))
                    .http_status_as_error(false)
                    .build(),
            ),
            base: base_url.trim_end_matches('/').to_owned(),
            token: token.to_owned(),
            ids: HashMap::new(),
        }
    }

    /// Recreates every object in the escrow, in dependency order.
    pub fn run_blocking(&mut self, escrow: &Value) -> Result<RestoreReport, NetboxError> {
        let mut report = RestoreReport::default();

        // Tier 1: no references.
        self.restore_simple(
            escrow,
            "dcim.sites",
            &["name", "slug"],
            &["name", "slug"],
            "slug",
            &mut report,
        )?;
        self.restore_simple(
            escrow,
            "dcim.manufacturers",
            &["name", "slug"],
            &["name", "slug"],
            "slug",
            &mut report,
        )?;
        self.restore_simple(
            escrow,
            "dcim.device-roles",
            &["name", "slug"],
            &["name", "slug"],
            "slug",
            &mut report,
        )?;
        self.restore_choice_sets(escrow, &mut report)?;
        self.restore_custom_fields(escrow, &mut report)?;
        self.restore_cluster_types(escrow, &mut report)?;

        // Tier 2: reference tier 1.
        self.restore_device_types(escrow, &mut report)?;
        self.restore_prefixes(escrow, &mut report)?;
        self.restore_vlans(escrow, &mut report)?;

        // Tier 3: devices + clusters reference tier 2.
        self.restore_devices(escrow, &mut report)?;
        self.restore_clusters(escrow, &mut report)?;

        // Tier 4: things owned by devices/clusters.
        self.restore_device_interfaces(escrow, &mut report)?;
        self.restore_inventory_items(escrow, &mut report)?;
        self.restore_vms(escrow, &mut report)?;
        self.restore_vm_interfaces(escrow, &mut report)?;

        // Tier 5: IPs (bind to interfaces), then the primary_ip back-refs.
        self.restore_ip_addresses(escrow, &mut report)?;
        self.restore_primary_ips(escrow, &mut report)?;

        Ok(report)
    }

    // ---- generic helpers ---------------------------------------------------

    fn objects<'a>(escrow: &'a Value, endpoint: &str) -> &'a [Value] {
        escrow
            .get(endpoint)
            .and_then(Value::as_array)
            .map_or(&[], Vec::as_slice)
    }

    /// GET-by-natural-key, else POST. Records and returns the object id.
    fn ensure(
        &mut self,
        endpoint: &str,
        query: &[(&str, String)],
        payload: Value,
        natural: String,
        report: &mut RestoreReport,
    ) -> Result<i64, NetboxError> {
        let key = (endpoint.to_owned(), natural);
        if let Some(id) = self.ids.get(&key) {
            return Ok(*id);
        }
        let qs: String = query
            .iter()
            .map(|(k, v)| format!("{k}={}", urlencode(v)))
            .collect::<Vec<_>>()
            .join("&");
        // Escrow keys are dot-separated (`dcim.sites`); API paths need slashes.
        let api = endpoint.replace('.', "/");
        let found = self.get_json(&format!("/api/{api}/?{qs}"))?;
        let id = if found.get("count").and_then(Value::as_i64).unwrap_or(0) > 0 {
            report.existing += 1;
            found["results"][0]["id"].as_i64().unwrap_or(0)
        } else {
            let created = self.post_json(&format!("/api/{api}/"), &payload)?;
            report.created += 1;
            created.get("id").and_then(Value::as_i64).unwrap_or(0)
        };
        self.ids.insert(key, id);
        Ok(id)
    }

    fn ref_id(&self, endpoint: &str, natural: &str) -> Option<i64> {
        self.ids
            .get(&(endpoint.to_owned(), natural.to_owned()))
            .copied()
    }

    // ---- per-type restore --------------------------------------------------

    fn restore_simple(
        &mut self,
        escrow: &Value,
        endpoint: &str,
        query_fields: &[&str],
        payload_fields: &[&str],
        natural_field: &str,
        report: &mut RestoreReport,
    ) -> Result<(), NetboxError> {
        for o in Self::objects(escrow, endpoint).to_vec() {
            let query: Vec<(&str, String)> = query_fields
                .iter()
                .map(|f| (*f, string_of(&o, f)))
                .collect();
            let mut payload = Map::new();
            for f in payload_fields {
                if let Some(v) = o.get(*f) {
                    payload.insert((*f).to_owned(), v.clone());
                }
            }
            self.ensure(
                endpoint,
                &query,
                Value::Object(payload),
                string_of(&o, natural_field),
                report,
            )?;
        }
        Ok(())
    }

    fn restore_choice_sets(
        &mut self,
        escrow: &Value,
        report: &mut RestoreReport,
    ) -> Result<(), NetboxError> {
        for o in Self::objects(escrow, "extras.custom-field-choice-sets").to_vec() {
            let name = string_of(&o, "name");
            let payload = json!({
                "name": name,
                "extra_choices": o.get("extra_choices").cloned().unwrap_or(json!([])),
            });
            self.ensure(
                "extras.custom-field-choice-sets",
                &[("name", name.clone())],
                payload,
                name,
                report,
            )?;
        }
        Ok(())
    }

    fn restore_custom_fields(
        &mut self,
        escrow: &Value,
        report: &mut RestoreReport,
    ) -> Result<(), NetboxError> {
        for o in Self::objects(escrow, "extras.custom-fields").to_vec() {
            let name = string_of(&o, "name");
            let mut payload = json!({
                "name": name,
                "type": value_of(&o, "type"),
                "object_types": o.get("object_types").cloned().unwrap_or(json!([])),
            });
            if let Some(cs) = o.get("choice_set").filter(|v| v.is_object()) {
                let cs_name = string_of(cs, "name");
                if let Some(id) = self.ref_id("extras.custom-field-choice-sets", &cs_name) {
                    payload["choice_set"] = json!(id);
                }
            }
            self.ensure(
                "extras.custom-fields",
                &[("name", name.clone())],
                payload,
                name,
                report,
            )?;
        }
        Ok(())
    }

    fn restore_cluster_types(
        &mut self,
        escrow: &Value,
        report: &mut RestoreReport,
    ) -> Result<(), NetboxError> {
        self.restore_simple(
            escrow,
            "virtualization.cluster-types",
            &["slug"],
            &["name", "slug"],
            "slug",
            report,
        )
    }

    fn restore_device_types(
        &mut self,
        escrow: &Value,
        report: &mut RestoreReport,
    ) -> Result<(), NetboxError> {
        for o in Self::objects(escrow, "dcim.device-types").to_vec() {
            let slug = string_of(&o, "slug");
            let mfg = o
                .get("manufacturer")
                .map(|m| string_of(m, "slug"))
                .unwrap_or_default();
            let mfg_id = self.ref_id("dcim.manufacturers", &mfg);
            let payload = json!({
                "manufacturer": mfg_id,
                "model": string_of(&o, "model"),
                "slug": slug,
            });
            self.ensure(
                "dcim.device-types",
                &[("slug", slug.clone())],
                payload,
                slug,
                report,
            )?;
        }
        Ok(())
    }

    fn restore_prefixes(
        &mut self,
        escrow: &Value,
        report: &mut RestoreReport,
    ) -> Result<(), NetboxError> {
        for o in Self::objects(escrow, "ipam.prefixes").to_vec() {
            let prefix = string_of(&o, "prefix");
            let mut payload =
                json!({"prefix": prefix, "description": string_of(&o, "description")});
            if let Some(site) = o.get("site").filter(|v| v.is_object()) {
                if let Some(id) = self.ref_id("dcim.sites", &string_of(site, "slug")) {
                    payload["site"] = json!(id);
                }
            }
            self.ensure(
                "ipam.prefixes",
                &[("prefix", prefix.clone())],
                payload,
                prefix,
                report,
            )?;
        }
        Ok(())
    }

    fn restore_vlans(
        &mut self,
        escrow: &Value,
        report: &mut RestoreReport,
    ) -> Result<(), NetboxError> {
        for o in Self::objects(escrow, "ipam.vlans").to_vec() {
            let vid = o.get("vid").and_then(Value::as_i64).unwrap_or(0);
            let mut payload = json!({
                "vid": vid, "name": string_of(&o, "name"),
                "description": string_of(&o, "description"),
            });
            if let Some(site) = o.get("site").filter(|v| v.is_object()) {
                if let Some(id) = self.ref_id("dcim.sites", &string_of(site, "slug")) {
                    payload["site"] = json!(id);
                }
            }
            self.ensure(
                "ipam.vlans",
                &[("vid", vid.to_string())],
                payload,
                vid.to_string(),
                report,
            )?;
        }
        Ok(())
    }

    fn restore_devices(
        &mut self,
        escrow: &Value,
        report: &mut RestoreReport,
    ) -> Result<(), NetboxError> {
        for o in Self::objects(escrow, "dcim.devices").to_vec() {
            let name = string_of(&o, "name");
            let dt = o
                .get("device_type")
                .map(|v| string_of(v, "slug"))
                .unwrap_or_default();
            let role = o
                .get("role")
                .map(|v| string_of(v, "slug"))
                .unwrap_or_default();
            let site = o
                .get("site")
                .map(|v| string_of(v, "slug"))
                .unwrap_or_default();
            let payload = json!({
                "name": name,
                "device_type": self.ref_id("dcim.device-types", &dt),
                "role": self.ref_id("dcim.device-roles", &role),
                "site": self.ref_id("dcim.sites", &site),
                "status": value_of(&o, "status"),
                "custom_fields": o.get("custom_fields").cloned().unwrap_or(json!({})),
                "comments": string_of(&o, "comments"),
            });
            self.ensure(
                "dcim.devices",
                &[("name", name.clone())],
                payload,
                name,
                report,
            )?;
        }
        Ok(())
    }

    fn restore_clusters(
        &mut self,
        escrow: &Value,
        report: &mut RestoreReport,
    ) -> Result<(), NetboxError> {
        for o in Self::objects(escrow, "virtualization.clusters").to_vec() {
            let name = string_of(&o, "name");
            let ctype = o
                .get("type")
                .map(|v| string_of(v, "slug"))
                .unwrap_or_default();
            let mut payload = json!({
                "name": name,
                "type": self.ref_id("virtualization.cluster-types", &ctype),
            });
            // scope is a site in our data.
            if o.get("scope_type").and_then(Value::as_str) == Some("dcim.site") {
                // single-site fleet: bind to the one site by its slug.
                if let Some((_, id)) = self.ids.iter().find(|((e, _), _)| e == "dcim.sites") {
                    payload["scope_type"] = json!("dcim.site");
                    payload["scope_id"] = json!(id);
                }
            }
            self.ensure(
                "virtualization.clusters",
                &[("name", name.clone())],
                payload,
                name,
                report,
            )?;
        }
        Ok(())
    }

    fn restore_device_interfaces(
        &mut self,
        escrow: &Value,
        report: &mut RestoreReport,
    ) -> Result<(), NetboxError> {
        for o in Self::objects(escrow, "dcim.interfaces").to_vec() {
            let name = string_of(&o, "name");
            let dev = o
                .get("device")
                .map(|v| string_of(v, "name"))
                .unwrap_or_default();
            let Some(dev_id) = self.ref_id("dcim.devices", &dev) else {
                continue;
            };
            let payload = json!({"device": dev_id, "name": name, "type": "other"});
            let natural = format!("{dev}/{name}");
            self.ensure_scoped(
                "dcim.interfaces",
                &[("device_id", dev_id.to_string()), ("name", name)],
                payload,
                natural,
                report,
            )?;
        }
        Ok(())
    }

    fn restore_inventory_items(
        &mut self,
        escrow: &Value,
        report: &mut RestoreReport,
    ) -> Result<(), NetboxError> {
        for o in Self::objects(escrow, "dcim.inventory-items").to_vec() {
            let name = string_of(&o, "name");
            let dev = o
                .get("device")
                .map(|v| string_of(v, "name"))
                .unwrap_or_default();
            let Some(dev_id) = self.ref_id("dcim.devices", &dev) else {
                continue;
            };
            let payload = json!({"device": dev_id, "name": name, "description": string_of(&o, "description")});
            let natural = format!("{dev}/{name}");
            self.ensure_scoped(
                "dcim.inventory-items",
                &[("device_id", dev_id.to_string()), ("name", name)],
                payload,
                natural,
                report,
            )?;
        }
        Ok(())
    }

    fn restore_vms(
        &mut self,
        escrow: &Value,
        report: &mut RestoreReport,
    ) -> Result<(), NetboxError> {
        for o in Self::objects(escrow, "virtualization.virtual-machines").to_vec() {
            let name = string_of(&o, "name");
            let cluster = o
                .get("cluster")
                .map(|v| string_of(v, "name"))
                .unwrap_or_default();
            let Some(cid) = self.ref_id("virtualization.clusters", &cluster) else {
                continue;
            };
            let payload = json!({
                "name": name, "cluster": cid, "status": value_of(&o, "status"),
                "custom_fields": o.get("custom_fields").cloned().unwrap_or(json!({})),
                "comments": string_of(&o, "comments"),
            });
            let natural = format!("{cluster}/{name}");
            self.ensure_scoped(
                "virtualization.virtual-machines",
                &[("cluster_id", cid.to_string()), ("name", name)],
                payload,
                natural,
                report,
            )?;
        }
        Ok(())
    }

    fn restore_vm_interfaces(
        &mut self,
        escrow: &Value,
        report: &mut RestoreReport,
    ) -> Result<(), NetboxError> {
        // vm natural key here is name; map vm name -> cluster for the scoped key.
        let vm_cluster: HashMap<String, String> =
            Self::objects(escrow, "virtualization.virtual-machines")
                .iter()
                .map(|vm| {
                    (
                        string_of(vm, "name"),
                        vm.get("cluster")
                            .map(|c| string_of(c, "name"))
                            .unwrap_or_default(),
                    )
                })
                .collect();
        for o in Self::objects(escrow, "virtualization.interfaces").to_vec() {
            let name = string_of(&o, "name");
            let vm = o
                .get("virtual_machine")
                .map(|v| string_of(v, "name"))
                .unwrap_or_default();
            let cluster = vm_cluster.get(&vm).cloned().unwrap_or_default();
            let Some(vm_id) = self.ref_id(
                "virtualization.virtual-machines",
                &format!("{cluster}/{vm}"),
            ) else {
                continue;
            };
            let payload = json!({"virtual_machine": vm_id, "name": name});
            let natural = format!("{vm}/{name}");
            self.ensure_scoped(
                "virtualization.interfaces",
                &[("virtual_machine_id", vm_id.to_string()), ("name", name)],
                payload,
                natural,
                report,
            )?;
        }
        Ok(())
    }

    fn restore_ip_addresses(
        &mut self,
        escrow: &Value,
        report: &mut RestoreReport,
    ) -> Result<(), NetboxError> {
        let iface_key = crate::logical::iface_natural_keys(escrow);
        for o in Self::objects(escrow, "ipam.ip-addresses").to_vec() {
            let addr = string_of(&o, "address");
            let mut payload = json!({"address": addr, "description": string_of(&o, "description")});
            if let (Some(t), Some(old_id)) = (
                o.get("assigned_object_type").and_then(Value::as_str),
                o.get("assigned_object_id").and_then(Value::as_i64),
            ) {
                if let Some(natural) = iface_key.get(&(t.to_owned(), old_id)) {
                    let iface_endpoint = if t == "dcim.interface" {
                        "dcim.interfaces"
                    } else {
                        "virtualization.interfaces"
                    };
                    if let Some(new_id) = self.ref_id(iface_endpoint, natural) {
                        payload["assigned_object_type"] = json!(t);
                        payload["assigned_object_id"] = json!(new_id);
                    }
                }
            }
            self.ensure(
                "ipam.ip-addresses",
                &[("address", addr.clone())],
                payload,
                addr,
                report,
            )?;
        }
        Ok(())
    }

    /// Second pass: set device/VM primary_ip4 and device oob_ip, now that the
    /// IPs exist.
    fn restore_primary_ips(
        &mut self,
        escrow: &Value,
        _report: &mut RestoreReport,
    ) -> Result<(), NetboxError> {
        for (endpoint, id_field) in [
            ("dcim.devices", "name"),
            ("virtualization.virtual-machines", "name"),
        ] {
            for o in Self::objects(escrow, endpoint).to_vec() {
                let natural = if endpoint == "dcim.devices" {
                    string_of(&o, id_field)
                } else {
                    let cluster = o
                        .get("cluster")
                        .map(|c| string_of(c, "name"))
                        .unwrap_or_default();
                    format!("{cluster}/{}", string_of(&o, "name"))
                };
                let Some(obj_id) = self.ref_id(endpoint, &natural) else {
                    continue;
                };
                let mut patch = Map::new();
                for (field, key) in [("primary_ip4", "address"), ("oob_ip", "address")] {
                    if let Some(ip) = o.get(field).filter(|v| v.is_object()) {
                        let addr = string_of(ip, key);
                        if let Some(ip_id) = self.ref_id("ipam.ip-addresses", &addr) {
                            patch.insert(field.to_owned(), json!(ip_id));
                        }
                    }
                }
                if !patch.is_empty() {
                    let api = endpoint.replace('.', "/");
                    self.patch_json(&format!("/api/{api}/{obj_id}/"), &Value::Object(patch))?;
                }
            }
        }
        Ok(())
    }

    /// Like `ensure` but the GET uses an explicit scoped query while the
    /// idempotency key is the caller's natural string.
    fn ensure_scoped(
        &mut self,
        endpoint: &str,
        query: &[(&str, String)],
        payload: Value,
        natural: String,
        report: &mut RestoreReport,
    ) -> Result<i64, NetboxError> {
        self.ensure(endpoint, query, payload, natural, report)
    }

    // ---- HTTP --------------------------------------------------------------

    fn get_json(&self, path: &str) -> Result<Value, NetboxError> {
        let url = format!("{}{path}", self.base);
        let mut resp = self
            .agent
            .get(&url)
            .header("Authorization", &format!("Token {}", self.token))
            .call()
            .map_err(|source| NetboxError::Http {
                url: url.clone(),
                source: Box::new(source),
            })?;
        self.read_json(&url, &mut resp)
    }

    fn post_json(&self, path: &str, payload: &Value) -> Result<Value, NetboxError> {
        let url = format!("{}{path}", self.base);
        let mut resp = self
            .agent
            .post(&url)
            .header("Authorization", &format!("Token {}", self.token))
            .header("Content-Type", "application/json")
            .send(payload.to_string().as_bytes())
            .map_err(|source| NetboxError::Http {
                url: url.clone(),
                source: Box::new(source),
            })?;
        if resp.status().as_u16() >= 300 {
            let body = resp.body_mut().read_to_string().unwrap_or_default();
            return Err(NetboxError::UnexpectedResponse {
                url,
                detail: format!("POST {} -> {}", resp.status(), truncate(&body)),
            });
        }
        self.read_json(&url, &mut resp)
    }

    fn patch_json(&self, path: &str, payload: &Value) -> Result<Value, NetboxError> {
        let url = format!("{}{path}", self.base);
        let mut resp = self
            .agent
            .patch(&url)
            .header("Authorization", &format!("Token {}", self.token))
            .header("Content-Type", "application/json")
            .send(payload.to_string().as_bytes())
            .map_err(|source| NetboxError::Http {
                url: url.clone(),
                source: Box::new(source),
            })?;
        if resp.status().as_u16() >= 300 {
            let body = resp.body_mut().read_to_string().unwrap_or_default();
            return Err(NetboxError::UnexpectedResponse {
                url,
                detail: format!("PATCH {} -> {}", resp.status(), truncate(&body)),
            });
        }
        self.read_json(&url, &mut resp)
    }

    fn read_json(
        &self,
        url: &str,
        resp: &mut ureq::http::Response<ureq::Body>,
    ) -> Result<Value, NetboxError> {
        let text = resp
            .body_mut()
            .read_to_string()
            .map_err(|source| NetboxError::Http {
                url: url.to_owned(),
                source: Box::new(source),
            })?;
        serde_json::from_str(&text).map_err(|source| NetboxError::Parse {
            url: url.to_owned(),
            source,
        })
    }
}

/// Deletes every object the escrow owns, in reverse dependency order. Used by
/// the restore drill to prove a from-empty rebuild.
pub fn wipe_blocking(base_url: &str, token: &str) -> Result<usize, NetboxError> {
    let session = Restore::new(base_url, token);
    // Reverse of creation order; children before parents.
    const ORDER: &[&str] = &[
        "ipam/ip-addresses",
        "virtualization/interfaces",
        "virtualization/virtual-machines",
        "virtualization/clusters",
        "virtualization/cluster-types",
        "dcim/inventory-items",
        "dcim/interfaces",
        "dcim/devices",
        "dcim/device-types",
        "dcim/device-roles",
        "dcim/manufacturers",
        "ipam/vlans",
        "ipam/prefixes",
        "dcim/sites",
        "extras/custom-fields",
        "extras/custom-field-choice-sets",
    ];
    let mut deleted = 0;
    for endpoint in ORDER {
        loop {
            let list = session.get_json(&format!("/api/{endpoint}/?limit=200"))?;
            let results = list
                .get("results")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            if results.is_empty() {
                break;
            }
            info!(endpoint, count = results.len(), "wiping");
            for o in &results {
                if let Some(id) = o.get("id").and_then(Value::as_i64) {
                    session.delete_one(&format!("/api/{endpoint}/{id}/"))?;
                    deleted += 1;
                }
            }
        }
    }
    Ok(deleted)
}

impl Restore {
    fn delete_one(&self, path: &str) -> Result<(), NetboxError> {
        let url = format!("{}{path}", self.base);
        let resp = self
            .agent
            .delete(&url)
            .header("Authorization", &format!("Token {}", self.token))
            .call()
            .map_err(|source| NetboxError::Http {
                url: url.clone(),
                source: Box::new(source),
            })?;
        if resp.status().as_u16() >= 300 {
            return Err(NetboxError::UnexpectedResponse {
                url,
                detail: format!("DELETE -> {}", resp.status()),
            });
        }
        Ok(())
    }
}

fn string_of(v: &Value, key: &str) -> String {
    v.get(key).and_then(Value::as_str).unwrap_or("").to_owned()
}

/// A `{value, label}` enum field reduced to its value; a plain string passes
/// through.
fn value_of(v: &Value, key: &str) -> Value {
    match v.get(key) {
        Some(Value::Object(m)) => m.get("value").cloned().unwrap_or(Value::Null),
        Some(other) => other.clone(),
        None => Value::Null,
    }
}

fn urlencode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn truncate(s: &str) -> String {
    s.chars().take(300).collect()
}

#[cfg(test)]
mod tests;
