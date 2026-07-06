#![allow(clippy::expect_used, clippy::panic)]

use std::io::{Read, Write};
use std::net::TcpListener;

use rollcall_inventory::{Probe, ProbeKind};

use crate::probe::{probe_http_blocking, probe_icmp_blocking, run_probes_blocking};
use crate::snapshot::ProbeState;

/// Serves one HTTP response on an ephemeral port and returns the bound URL.
fn serve_once(status_line: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
    let addr = listener.local_addr().expect("local addr");
    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        // Drain the request head before answering so the client is not
        // racing a closed write side.
        let mut buf = [0u8; 1024];
        let _bytes = stream.read(&mut buf).expect("read request");
        let response = format!(
            "HTTP/1.1 {status_line}\r\nLocation: http://127.0.0.1:1/\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        );
        stream
            .write_all(response.as_bytes())
            .expect("write response");
    });
    format!("http://{addr}/")
}

#[test]
fn http_matching_expect_is_up() {
    let url = serve_once("200 OK");
    assert_eq!(
        probe_http_blocking(&url, &[200], false),
        (true, "HTTP 200".to_owned())
    );
}

/// A 301 must be reported as-is, not followed: the Location header above
/// points at a dead port, so following it would report unreachable.
#[test]
fn http_redirect_is_reported_not_followed() {
    let url = serve_once("301 Moved Permanently");
    assert_eq!(
        probe_http_blocking(&url, &[200, 301, 302], false),
        (true, "HTTP 301".to_owned())
    );
}

#[test]
fn http_unexpected_status_is_not_ok() {
    let url = serve_once("500 Internal Server Error");
    assert_eq!(
        probe_http_blocking(&url, &[200], false),
        (false, "HTTP 500".to_owned())
    );
}

#[test]
fn http_connection_refused_is_unreachable() {
    // Bind then drop, so the port is known-closed.
    let addr = {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
        listener.local_addr().expect("local addr")
    };
    let (ok, detail) = probe_http_blocking(&format!("http://{addr}/"), &[200], false);
    assert!(!ok);
    assert!(detail.starts_with("unreachable ("), "detail was: {detail}");
}

#[test]
fn icmp_loopback_is_up() {
    let (ok, detail) = probe_icmp_blocking("127.0.0.1").expect("ping spawns");
    assert!(ok);
    assert_eq!(detail, "icmp reply");
}

#[test]
fn failed_probe_state_depends_on_lan_reachable() {
    let addr = {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
        listener.local_addr().expect("local addr")
    };
    let url = format!("http://{addr}/");
    let mk = |id: &str, lan_reachable: bool| Probe {
        id: id.to_owned(),
        desc: id.to_owned(),
        lan_reachable,
        kind: ProbeKind::Http {
            url: url.clone(),
            expect: vec![200],
            insecure: false,
        },
    };
    let results =
        run_probes_blocking(&[mk("lan", true), mk("fabric-only", false)]).expect("probes run");
    assert_eq!(results[0].state, ProbeState::Down);
    assert_eq!(results[1].state, ProbeState::Unverifiable);
}
