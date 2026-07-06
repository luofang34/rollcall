#![allow(clippy::expect_used, clippy::panic)]

use std::path::Path;

use crate::capex::load_capex_blocking;

/// The fixture capex.toml is the schema's golden input.
#[test]
fn parses_fixture_capex_file() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/finance/capex.toml");
    let capex = load_capex_blocking(&path).expect("fixture capex.toml must parse");
    assert!(!capex.item.is_empty());
    for item in &capex.item {
        assert!(item.usd > 0, "{}: non-positive cost", item.desc);
        assert!(
            matches!(item.basis.as_str(), "estimate" | "invoice"),
            "{}: basis must be estimate or invoice, got {:?}",
            item.desc,
            item.basis
        );
    }
}
