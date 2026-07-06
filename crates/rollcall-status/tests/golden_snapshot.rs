//! The committed snapshots are the format's golden files: the Rust types
//! must round-trip them without loss, or the report builder's input drifts.

#![allow(clippy::expect_used, clippy::panic)]

use std::path::Path;

use rollcall_status::{ProbeState, Snapshot};

#[test]
fn committed_snapshots_round_trip() {
    let status_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/status");
    let mut checked = 0usize;
    for entry in std::fs::read_dir(&status_dir).expect("status/ exists") {
        let path = entry.expect("readable dir entry").path();
        if path.extension().is_none_or(|e| e != "json") {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("snapshot readable");
        let snapshot: Snapshot =
            serde_json::from_str(&text).expect("snapshot parses into Snapshot");
        assert!(!snapshot.results.is_empty());

        let reserialized = serde_json::to_string(&snapshot).expect("snapshot reserializes");
        let original: serde_json::Value = serde_json::from_str(&text).expect("original is JSON");
        let round_tripped: serde_json::Value =
            serde_json::from_str(&reserialized).expect("reserialized is JSON");
        assert_eq!(original, round_tripped, "lossy round-trip of {path:?}");
        checked += 1;
    }
    assert!(checked >= 1, "no snapshots found in {status_dir:?}");
}

#[test]
fn golden_2026_07_01_states_survive_parsing() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/status/2026-07-01.json");
    let text = std::fs::read_to_string(path).expect("golden snapshot readable");
    let snapshot: Snapshot = serde_json::from_str(&text).expect("golden snapshot parses");
    assert_eq!(snapshot.generated_at, "2026-07-01T23:06:47.941802-04:00");

    let state_of = |id: &str| {
        snapshot
            .results
            .iter()
            .find(|r| r.id == id)
            .unwrap_or_else(|| panic!("probe {id} in golden snapshot"))
            .state
    };
    assert_eq!(state_of("edge"), ProbeState::Up);
    assert_eq!(state_of("compute-a"), ProbeState::Down);
    assert_eq!(state_of("fabric-svc"), ProbeState::Unverifiable);
}
