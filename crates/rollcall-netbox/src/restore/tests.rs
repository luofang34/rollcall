#![allow(clippy::expect_used, clippy::panic)]

use serde_json::json;

use crate::restore::{RestoreReport, value_of};

#[test]
fn value_of_reduces_enum_and_passes_strings() {
    let obj = json!({"status": {"value": "active", "label": "Active"}, "plain": "x"});
    assert_eq!(value_of(&obj, "status"), json!("active"));
    assert_eq!(value_of(&obj, "plain"), json!("x"));
    assert_eq!(value_of(&obj, "missing"), json!(null));
}

#[test]
fn restore_report_accumulates() {
    let mut r = RestoreReport::default();
    r.created += 3;
    r.existing += 1;
    assert_eq!(
        r,
        RestoreReport {
            created: 3,
            existing: 1
        }
    );
}
