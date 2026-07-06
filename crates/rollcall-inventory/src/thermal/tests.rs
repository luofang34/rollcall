#![allow(clippy::expect_used, clippy::panic)]

use std::path::Path;

use crate::thermal::load_thermal_blocking;

/// The fixture thermal.toml is the schema's golden input.
#[test]
fn parses_fixture_thermal_file() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/thermal/thermal.toml");
    let thermal = load_thermal_blocking(&path).expect("fixture thermal.toml must parse");
    assert!(thermal.cooling.btu_per_watt_hour > 3.0);
    assert!(
        thermal.cooling.ton_refrigeration_btu_h > 0.0,
        "integer BTU/h coerces to f64"
    );
}
