#![allow(clippy::expect_used, clippy::panic)]

use serde_json::json;

use crate::canonical::canonicalize_objects;

#[test]
fn strips_volatile_keys_recursively_and_sorts_by_id() {
    let raw = vec![
        json!({
            "id": 7,
            "name": "compute-b",
            "url": "http://x/api/dcim/devices/7/",
            "display": "compute-b",
            "created": "2026-07-05T01:00:00Z",
            "last_updated": "2026-07-05T02:00:00Z",
            "site": {"id": 1, "name": "site-a", "url": "http://x/api/dcim/sites/1/", "display": "site-a"},
            "tags": [{"id": 3, "display": "t"}]
        }),
        json!({"id": 2, "name": "store", "url": "http://x/2/"}),
    ];
    let canonical = canonicalize_objects(raw);

    assert_eq!(canonical[0]["id"], 2, "sorted by id");
    assert_eq!(canonical[1]["id"], 7);
    let device = &canonical[1];
    assert!(device.get("url").is_none());
    assert!(device.get("created").is_none());
    assert!(device.get("last_updated").is_none());
    assert!(
        device["site"].get("url").is_none(),
        "nested brief objects stripped"
    );
    assert!(
        device["tags"][0].get("display").is_none(),
        "arrays stripped"
    );
    assert_eq!(device["site"]["name"], "site-a", "data fields survive");
}

#[test]
fn canonicalization_is_idempotent_and_deterministic() {
    let raw = vec![
        json!({"id": 5, "b": 1, "a": 2, "url": "x"}),
        json!({"id": 1, "nested": {"display": "d", "keep": true}}),
    ];
    let once = canonicalize_objects(raw);
    let twice = canonicalize_objects(once.clone());
    assert_eq!(once, twice, "idempotent");

    let bytes_a = serde_json::to_string(&once).expect("serializes");
    let bytes_b = serde_json::to_string(&twice).expect("serializes");
    assert_eq!(
        bytes_a, bytes_b,
        "byte-deterministic (serde_json sorts keys)"
    );
}

#[test]
fn objects_without_id_sort_last_not_panic() {
    let raw = vec![json!({"name": "no-id"}), json!({"id": 9})];
    let canonical = canonicalize_objects(raw);
    assert_eq!(canonical[0]["id"], 9);
    assert_eq!(canonical[1]["name"], "no-id");
}
