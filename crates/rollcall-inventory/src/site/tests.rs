#![allow(clippy::expect_used, clippy::panic)]

use std::path::Path;

use crate::site::load_site_blocking;

/// The fixture site.toml is the schema's golden input.
#[test]
fn parses_fixture_site_file() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/inventory/site.toml");
    let site = load_site_blocking(&path).expect("fixture site.toml must parse");
    assert_eq!(site.site.name, "Example Fleet");
    assert!(site.power.tariff_usd_per_kwh > 0.0);
    assert!(site.power.hours_per_month > 0);
    assert!(site.networks.fabric.contains('/'), "fabric is a CIDR");
}
