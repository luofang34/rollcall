#![allow(clippy::expect_used, clippy::panic)]

use std::path::Path;

use crate::devices::load_devices_blocking;

/// The fixture devices.toml is the schema's golden input.
#[test]
fn parses_fixture_devices_file() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/inventory/devices.toml");
    let devices = load_devices_blocking(&path).expect("fixture devices.toml must parse");
    assert!(!devices.device.is_empty());

    let gpu_node = devices
        .device
        .iter()
        .find(|d| d.id == "compute-a")
        .expect("compute-a present");
    assert_eq!(gpu_node.ram_gb, Some(128));
    assert_eq!(gpu_node.accelerator.len(), 1);
    assert_eq!(gpu_node.accelerator[0].count, 4);
    assert!(
        gpu_node.power_typical_w > 0.0,
        "integer watts coerce to f64"
    );

    for device in &devices.device {
        assert!(
            device.power_peak_w >= device.power_typical_w,
            "{}: peak below typical",
            device.id
        );
    }
}

#[test]
fn expected_offline_defaults_false_and_parses_true() {
    let toml = r#"
        [[device]]
        id = "sibyl"
        name = "Sibyl"
        role = "GPU node"
        power_typical_w = 100
        power_peak_w = 200
        expected_offline = true

        [[device]]
        id = "seshat"
        name = "Seshat"
        role = "host"
        power_typical_w = 100
        power_peak_w = 200
    "#;
    let devices: crate::devices::DevicesFile = ::toml::from_str(toml).expect("parses");
    assert!(devices.device[0].expected_offline);
    assert!(!devices.device[1].expected_offline, "absent -> false");
}
