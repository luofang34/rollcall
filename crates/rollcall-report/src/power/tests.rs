#![allow(clippy::expect_used, clippy::panic)]

use rollcall_status::{ProbeResult, ProbeState, Snapshot};

use crate::power::{power_model, usd_per_month};

fn site(tariff: f64, hours: i64) -> rollcall_inventory::SiteFile {
    toml::from_str(&format!(
        r#"
        [site]
        name = "t"
        operator = "t"
        description = "t"
        [power]
        tariff_usd_per_kwh = {tariff}
        hours_per_month = {hours}
        [networks]
        lan = "l"
        fabric = "f"
        wan = "w"
        "#
    ))
    .expect("valid site TOML")
}

fn devices() -> rollcall_inventory::DevicesFile {
    toml::from_str(
        r#"
        [[device]]
        id = "a"
        name = "A"
        role = "r"
        ip_lan = "1"
        power_typical_w = 100
        power_peak_w = 200

        [[device]]
        id = "b"
        name = "B"
        role = "r"
        ip_lan = "2"
        power_typical_w = 300
        power_peak_w = 400
        "#,
    )
    .expect("valid devices TOML")
}

fn snapshot_with_b_down() -> Snapshot {
    Snapshot {
        generated_at: "t".to_owned(),
        results: vec![
            ProbeResult {
                id: "a".to_owned(),
                desc: "A".to_owned(),
                state: ProbeState::Up,
                detail: "d".to_owned(),
            },
            ProbeResult {
                id: "b".to_owned(),
                desc: "B".to_owned(),
                state: ProbeState::Down,
                detail: "d".to_owned(),
            },
        ],
    }
}

#[test]
fn down_devices_are_excluded_from_live_but_not_all_on() {
    let model = power_model(&devices(), &site(0.5, 100), &snapshot_with_b_down());
    assert_eq!(model.all_on_typical_w, 400.0);
    assert_eq!(model.live_typical_w, 100.0);
    assert_eq!(model.peak_w, 600.0);
    assert!(model.rows[0].is_up);
    assert!(!model.rows[1].is_up);
    // 300 W at 100 h/mo = 30 kWh; at $0.5/kWh = $15.
    assert_eq!(model.rows[1].kwh_per_month, 30.0);
    assert_eq!(model.rows[1].usd_per_month, 15.0);
}

#[test]
fn unprobed_devices_count_as_live() {
    let snapshot = Snapshot {
        generated_at: "t".to_owned(),
        results: vec![],
    };
    let model = power_model(&devices(), &site(0.5, 100), &snapshot);
    assert_eq!(model.live_typical_w, model.all_on_typical_w);
}

#[test]
fn usd_per_month_is_kwh_times_tariff() {
    assert_eq!(usd_per_month(1000.0, &site(0.25, 720)), 180.0);
}
