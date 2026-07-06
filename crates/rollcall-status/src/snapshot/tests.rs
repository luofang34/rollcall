#![allow(clippy::expect_used, clippy::panic)]

use crate::snapshot::{ProbeState, local_stamp};

#[test]
fn state_serializes_lowercase_and_matches_display() {
    for state in [ProbeState::Up, ProbeState::Down, ProbeState::Unverifiable] {
        let json = serde_json::to_string(&state).expect("state serializes");
        assert_eq!(json, format!("\"{state}\""));
    }
}

/// generated_at must keep the shape of the committed snapshots
/// (2026-07-01T23:06:47.941802-04:00): RFC 3339 local time, microsecond
/// fraction, numeric offset.
#[test]
fn stamp_matches_snapshot_timestamp_shape() {
    let stamp = local_stamp();
    let (date_part, rest) = stamp
        .generated_at
        .split_once('T')
        .expect("timestamp has a T separator");
    assert_eq!(date_part, stamp.date);
    assert_eq!(stamp.date.len(), "2026-07-01".len());

    let (clock, fraction_offset) = rest.split_once('.').expect("timestamp has a fraction");
    assert_eq!(clock.len(), "23:06:47".len());
    let offset_sign = fraction_offset
        .find(['+', '-'])
        .expect("timestamp has a numeric offset");
    assert_eq!(offset_sign, 6, "fraction is microsecond-precision");
    assert_eq!(fraction_offset.len(), "941802-04:00".len());
}
