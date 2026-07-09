#![allow(clippy::expect_used, clippy::panic)]

use super::resolve_pull_url;

#[test]
fn override_wins_over_site() {
    let url = resolve_pull_url(
        Some("http://127.0.0.1"),
        Some("http://192.168.89.114".to_owned()),
    )
    .expect("override resolves");
    assert_eq!(url, "http://127.0.0.1");
}

#[test]
fn falls_back_to_site_when_no_override() {
    let url = resolve_pull_url(None, Some("http://192.168.89.114".to_owned()))
        .expect("site url resolves");
    assert_eq!(url, "http://192.168.89.114");
}

#[test]
fn errors_when_neither_present() {
    assert!(resolve_pull_url(None, None).is_err());
}
