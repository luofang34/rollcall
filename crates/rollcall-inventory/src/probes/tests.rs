#![allow(clippy::expect_used, clippy::panic)]

use std::path::Path;

use crate::error::InventoryError;
use crate::probes::{Probe, ProbeKind, load_probes_blocking, parse_probes};

fn parse(text: &str) -> Result<Vec<Probe>, InventoryError> {
    parse_probes(text, Path::new("test-probes.toml"))
}

/// The fixture probes.toml is the schema's golden input: an edit that breaks
/// the schema (or a schema change that breaks the file) fails this test.
#[test]
fn parses_fixture_probes_file() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/inventory/probes.toml");
    let probes = load_probes_blocking(&path).expect("fixture probes.toml must parse");
    assert!(!probes.is_empty());

    let edge = probes
        .iter()
        .find(|p| p.id == "edge")
        .expect("edge probe present");
    assert!(edge.lan_reachable);
    match &edge.kind {
        ProbeKind::Icmp { target } => assert_eq!(target, "192.0.2.1"),
        other => panic!("edge should be icmp, got {other:?}"),
    }

    let fabric = probes
        .iter()
        .find(|p| p.id == "fabric-svc")
        .expect("fabric-svc probe present");
    assert!(!fabric.lan_reachable, "fabric-svc is fabric-only");

    let gateway = probes
        .iter()
        .find(|p| p.id == "gateway")
        .expect("gateway probe present");
    match &gateway.kind {
        ProbeKind::Http {
            expect, insecure, ..
        } => {
            assert_eq!(expect, &[200, 301, 302]);
            assert!(!insecure);
        }
        other => panic!("gateway should be http, got {other:?}"),
    }
}

#[test]
fn http_defaults_apply() {
    let probes = parse(
        r#"
        [[probe]]
        id = "svc"
        desc = "some service"
        kind = "http"
        url = "http://127.0.0.1:1/"
        "#,
    )
    .expect("valid probe TOML");
    let probe = &probes[0];
    assert!(probe.lan_reachable, "lan_reachable defaults to true");
    match &probe.kind {
        ProbeKind::Http {
            expect, insecure, ..
        } => {
            assert_eq!(expect, &[200], "expect defaults to [200]");
            assert!(!insecure, "insecure defaults to false");
        }
        other => panic!("expected http probe, got {other:?}"),
    }
}

#[test]
fn unknown_kind_is_rejected() {
    let err = parse(
        r#"
        [[probe]]
        id = "svc"
        desc = "some service"
        kind = "carrier-pigeon"
        url = "http://127.0.0.1:1/"
        "#,
    )
    .expect_err("unknown kind must not parse");
    match err {
        InventoryError::UnknownProbeKind { kind, .. } => assert_eq!(kind, "carrier-pigeon"),
        other => panic!("expected UnknownProbeKind, got {other:?}"),
    }
}

#[test]
fn missing_kind_is_rejected() {
    let err = parse(
        r#"
        [[probe]]
        id = "svc"
        desc = "some service"
        url = "http://127.0.0.1:1/"
        "#,
    )
    .expect_err("entry without kind must not parse");
    assert!(matches!(err, InventoryError::MissingProbeKind { .. }));
}

/// A typo in an optional key must be an error, not a silently applied
/// default: `exepct = [301]` falling back to expect = [200] would report a
/// healthy 301-answering service as down.
#[test]
fn misspelled_optional_key_is_rejected() {
    let err = parse(
        r#"
        [[probe]]
        id = "svc"
        desc = "some service"
        kind = "http"
        url = "http://127.0.0.1:1/"
        exepct = [301]
        "#,
    )
    .expect_err("misspelled key must not parse");
    assert!(matches!(err, InventoryError::InvalidProbe { .. }));

    let err = parse(
        r#"
        [[probe]]
        id = "fabric"
        desc = "fabric-only host"
        kind = "icmp"
        target = "198.51.100.1"
        lan-reachable = false
        "#,
    )
    .expect_err("hyphenated key must not parse");
    assert!(matches!(err, InventoryError::InvalidProbe { .. }));
}

/// Ids key snapshot entries and downstream indexes; a copy-pasted entry
/// with a stale id must fail loudly instead of shadowing a row later.
#[test]
fn duplicate_id_is_rejected() {
    let err = parse(
        r#"
        [[probe]]
        id = "svc"
        desc = "the host"
        kind = "icmp"
        target = "192.0.2.2"

        [[probe]]
        id = "svc"
        desc = "the service"
        kind = "http"
        url = "http://192.0.2.2/"
        "#,
    )
    .expect_err("duplicate id must not parse");
    match err {
        InventoryError::DuplicateProbeId { id, .. } => assert_eq!(id, "svc"),
        other => panic!("expected DuplicateProbeId, got {other:?}"),
    }
}
