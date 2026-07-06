//! Canonicalization: the escrow must be deterministic — two pulls with no
//! intervening edits produce identical bytes, so a git diff of the export is
//! exactly the set of NetBox edits and nothing else.

use serde_json::Value;

/// Keys that churn without a data change and are stripped recursively —
/// timestamps and hyperlink decorations NetBox attaches to every object and
/// every nested brief-object.
const VOLATILE_KEYS: &[&str] = &["url", "display", "display_url", "created", "last_updated"];

/// Canonicalizes one endpoint's object list: volatile keys stripped
/// recursively, objects sorted by `id`. Key order inside objects is already
/// deterministic (serde_json maps are sorted).
pub fn canonicalize_objects(mut objects: Vec<Value>) -> Vec<Value> {
    for object in &mut objects {
        strip_volatile(object);
    }
    objects.sort_by_key(|o| o.get("id").and_then(Value::as_i64).unwrap_or(i64::MAX));
    objects
}

fn strip_volatile(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for key in VOLATILE_KEYS {
                map.remove(*key);
            }
            for nested in map.values_mut() {
                strip_volatile(nested);
            }
        }
        Value::Array(items) => {
            for item in items {
                strip_volatile(item);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests;
