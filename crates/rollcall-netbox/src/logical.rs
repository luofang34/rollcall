//! Id-independent logical view of the escrow, for the restore drill.
//!
//! A rebuild reassigns every NetBox internal id, so `restore → pull` can
//! never be byte-identical. What must hold is *logical* equality: the same
//! objects identified by natural key, with references expressed by natural
//! key rather than autoincrement id. [`logical_view`] reduces an escrow to
//! that form so the drill can assert `before == after`.

use std::collections::BTreeMap;

use serde_json::{Map, Value};

/// Reference fields whose brief-object value collapses to a natural-key
/// string. Order-independent; applied wherever the key appears.
const REF_NATURAL_KEY: &[(&str, &str)] = &[
    ("manufacturer", "slug"),
    ("device_type", "slug"),
    ("role", "slug"),
    ("site", "slug"),
    ("cluster", "name"),
    ("type", "slug"),
    ("device", "name"),
    ("virtual_machine", "name"),
    ("choice_set", "name"),
    ("primary_ip4", "address"),
    ("primary_ip6", "address"),
    ("oob_ip", "address"),
    ("status", "value"),
];

/// Reduces an escrow document to its id-independent logical form: a map from
/// endpoint to a natural-key-sorted list of objects, each with ids and
/// volatile fields removed and reference brief-objects collapsed to their
/// natural key. IP assignments are resolved to the assigned object's natural
/// key so they survive an interface-id reassignment.
pub fn logical_view(escrow: &Value) -> Value {
    let iface_key = build_iface_key_index(escrow);
    let scope_key = build_scope_key_index(escrow);
    let mut out = Map::new();
    for (endpoint, objects) in escrow.as_object().into_iter().flatten() {
        let Some(list) = objects.as_array() else {
            continue;
        };
        let mut logical: Vec<Value> = list
            .iter()
            .map(|o| logical_object(endpoint, o, &iface_key, &scope_key))
            .collect();
        logical.sort_by_key(natural_key);
        out.insert(endpoint.clone(), Value::Array(logical));
    }
    Value::Object(out)
}

fn logical_object(
    endpoint: &str,
    object: &Value,
    iface_key: &BTreeMap<(String, i64), String>,
    scope_key: &BTreeMap<(String, i64), String>,
) -> Value {
    let mut map = object.as_object().cloned().unwrap_or_default();
    map.remove("id");
    for key in ["url", "display", "display_url", "created", "last_updated"] {
        map.remove(key);
    }
    for (field, natural) in REF_NATURAL_KEY {
        if let Some(v) = map.get(*field) {
            if v.is_object() {
                let key = v.get(*natural).cloned().unwrap_or(Value::Null);
                map.insert((*field).to_owned(), key);
            }
        }
    }
    // A generic scope (cluster → site): replace the raw scope_id with the
    // scoped object's natural key, which survives a rebuild.
    if let (Some(t), Some(id)) = (
        map.get("scope_type").and_then(Value::as_str),
        map.get("scope_id").and_then(Value::as_i64),
    ) {
        let key = scope_key.get(&(t.to_owned(), id)).cloned();
        map.remove("scope_id");
        map.insert("scope".to_owned(), key.map_or(Value::Null, Value::String));
    }
    // An IP's assignment: replace the (type, interface id) pair with the
    // interface's natural key, which survives a rebuild.
    if endpoint == "ipam.ip-addresses" {
        let resolved = map
            .get("assigned_object_type")
            .and_then(Value::as_str)
            .zip(map.get("assigned_object_id").and_then(Value::as_i64))
            .and_then(|(t, id)| iface_key.get(&(t.to_owned(), id)).cloned());
        map.remove("assigned_object_id");
        map.insert(
            "assigned_object".to_owned(),
            resolved.map_or(Value::Null, Value::String),
        );
    }
    // Recurse into any remaining nested objects/arrays to drop their ids too.
    for value in map.values_mut() {
        strip_ids(value);
    }
    Value::Object(map)
}

fn strip_ids(value: &mut Value) {
    match value {
        Value::Object(map) => {
            map.remove("id");
            for key in ["url", "display", "created", "last_updated"] {
                map.remove(key);
            }
            for v in map.values_mut() {
                strip_ids(v);
            }
        }
        Value::Array(items) => items.iter_mut().for_each(strip_ids),
        _ => {}
    }
}

/// (assigned_object_type, interface id) → "device_or_vm_name/iface_name".
/// Public so restore can rebind IPs to freshly created interfaces by the
/// same natural key.
pub fn iface_natural_keys(escrow: &Value) -> BTreeMap<(String, i64), String> {
    build_iface_key_index(escrow)
}

/// (scope_type, scoped object id) → natural key. Today the only scope is
/// `dcim.site`; extend the map if clusters gain other scope types.
fn build_scope_key_index(escrow: &Value) -> BTreeMap<(String, i64), String> {
    let mut index = BTreeMap::new();
    for site in escrow
        .get("dcim.sites")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if let (Some(id), Some(slug)) = (
            site.get("id").and_then(Value::as_i64),
            site.get("slug").and_then(Value::as_str),
        ) {
            index.insert(("dcim.site".to_owned(), id), slug.to_owned());
        }
    }
    index
}

fn build_iface_key_index(escrow: &Value) -> BTreeMap<(String, i64), String> {
    let mut index = BTreeMap::new();
    for (endpoint, parent, kind) in [
        ("dcim.interfaces", "device", "dcim.interface"),
        (
            "virtualization.interfaces",
            "virtual_machine",
            "virtualization.vminterface",
        ),
    ] {
        for iface in escrow
            .get(endpoint)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let (Some(id), Some(name), Some(parent_name)) = (
                iface.get("id").and_then(Value::as_i64),
                iface.get("name").and_then(Value::as_str),
                iface
                    .get(parent)
                    .and_then(|p| p.get("name"))
                    .and_then(Value::as_str),
            ) else {
                continue;
            };
            index.insert((kind.to_owned(), id), format!("{parent_name}/{name}"));
        }
    }
    index
}

/// A stable natural-key string for sorting logical objects within an
/// endpoint — whichever identifying fields the object type carries.
fn natural_key(object: &Value) -> String {
    for field in ["address", "prefix", "vid", "slug", "name", "model"] {
        if let Some(v) = object.get(field) {
            if !v.is_null() {
                return format!("{field}={v}");
            }
        }
    }
    object.to_string()
}

#[cfg(test)]
mod tests;
