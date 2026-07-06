#![allow(clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;

use crate::editorial::render_fragment_blocking;
use crate::error::ReportError;

fn write_fragment(dir: &std::path::Path, text: &str) {
    std::fs::write(dir.join("frag.tex"), text).expect("write fragment");
}

fn values(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect()
}

#[test]
fn substitutes_placeholders() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_fragment(dir.path(), "up: @@up@@ of @@total@@\n");
    let out = render_fragment_blocking(
        dir.path(),
        "frag.tex",
        &values(&[("up", "11"), ("total", "13")]),
    )
    .expect("fragment renders");
    assert_eq!(out, "up: 11 of 13\n");
}

#[test]
fn unknown_placeholder_is_an_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_fragment(dir.path(), "@@nope@@");
    let err = render_fragment_blocking(dir.path(), "frag.tex", &values(&[]))
        .expect_err("unknown key must fail");
    match err {
        ReportError::UnknownPlaceholder { key, .. } => assert_eq!(key, "nope"),
        other => panic!("expected UnknownPlaceholder, got {other:?}"),
    }
}

#[test]
fn unterminated_placeholder_is_an_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_fragment(dir.path(), "text @@key without close\n");
    let err = render_fragment_blocking(dir.path(), "frag.tex", &values(&[("key", "v")]))
        .expect_err("unterminated marker must fail");
    assert!(matches!(err, ReportError::UnterminatedPlaceholder { .. }));
}

#[test]
fn fragment_without_placeholders_passes_through() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_fragment(dir.path(), "plain \\LaTeX{} text\n");
    let out =
        render_fragment_blocking(dir.path(), "frag.tex", &values(&[])).expect("fragment renders");
    assert_eq!(out, "plain \\LaTeX{} text\n");
}
