#![allow(clippy::expect_used, clippy::panic)]

use serde_json::json;

use crate::logical::logical_view;

/// Two escrows that differ only in NetBox-assigned ids (a device and its
/// device_type/site/status references, an IP bound to an interface) must
/// have identical logical views — the property the restore drill relies on.
#[test]
fn id_reassignment_does_not_change_the_logical_view() {
    let before = json!({
        "dcim.devices": [{
            "id": 7, "name": "Compute-B",
            "device_type": {"id": 5, "slug": "example-board-x1"},
            "site": {"id": 1, "slug": "example-fleet"},
            "status": {"value": "active", "label": "Active"},
            "custom_fields": {"ram_gb": 512}
        }],
        "dcim.interfaces": [{"id": 10, "name": "lan", "device": {"id": 7, "name": "Compute-B"}}],
        "ipam.ip-addresses": [{
            "id": 99, "address": "192.0.2.5/24",
            "assigned_object_type": "dcim.interface", "assigned_object_id": 10
        }]
    });
    // Same fleet, every id shifted by a rebuild.
    let after = json!({
        "dcim.devices": [{
            "id": 41, "name": "Compute-B",
            "device_type": {"id": 88, "slug": "example-board-x1"},
            "site": {"id": 3, "slug": "example-fleet"},
            "status": {"value": "active", "label": "Active"},
            "custom_fields": {"ram_gb": 512}
        }],
        "dcim.interfaces": [{"id": 200, "name": "lan", "device": {"id": 41, "name": "Compute-B"}}],
        "ipam.ip-addresses": [{
            "id": 500, "address": "192.0.2.5/24",
            "assigned_object_type": "dcim.interface", "assigned_object_id": 200
        }]
    });
    assert_eq!(
        logical_view(&before),
        logical_view(&after),
        "logical views must match despite reassigned ids"
    );
}

/// A genuine content change (an IP moved to a different interface) must show
/// up as a logical difference.
#[test]
fn a_real_change_is_visible_in_the_logical_view() {
    let base = json!({
        "dcim.interfaces": [
            {"id": 10, "name": "lan", "device": {"id": 7, "name": "Compute-B"}},
            {"id": 11, "name": "fabric", "device": {"id": 7, "name": "Compute-B"}}
        ],
        "ipam.ip-addresses": [{
            "id": 99, "address": "192.0.2.5/24",
            "assigned_object_type": "dcim.interface", "assigned_object_id": 10
        }]
    });
    let moved = json!({
        "dcim.interfaces": [
            {"id": 10, "name": "lan", "device": {"id": 7, "name": "Compute-B"}},
            {"id": 11, "name": "fabric", "device": {"id": 7, "name": "Compute-B"}}
        ],
        "ipam.ip-addresses": [{
            "id": 99, "address": "192.0.2.5/24",
            "assigned_object_type": "dcim.interface", "assigned_object_id": 11
        }]
    });
    assert_ne!(
        logical_view(&base),
        logical_view(&moved),
        "an IP rebound to a different interface must be a logical difference"
    );
}

/// A description edit is a logical difference (drift detection still works on
/// the logical view).
#[test]
fn description_edit_is_visible() {
    let a = json!({"ipam.vlans": [{"id": 1, "vid": 30, "name": "x", "description": "before"}]});
    let b = json!({"ipam.vlans": [{"id": 1, "vid": 30, "name": "x", "description": "after"}]});
    assert_ne!(logical_view(&a), logical_view(&b));
}
