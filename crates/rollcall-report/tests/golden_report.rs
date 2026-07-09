//! Renders the report from frozen synthetic fixture inputs and compares
//! against the frozen expected LaTeX, byte for byte. The fixture pair pins the
//! renderer's output; a renderer change that alters the document must update
//! the fixtures deliberately (re-render from the fixture repo).

#![allow(clippy::expect_used, clippy::panic)]

use std::path::Path;

use rollcall_report::{NarrateMode, load_inputs_blocking, render_document_blocking};

#[test]
fn fixture_inputs_render_the_expected_document() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let repo = fixtures.join("repo");
    let inputs = load_inputs_blocking(&repo).expect("fixture inputs load");
    // Off: the golden fixture pins the authored render, so narration stays out.
    let rendered = render_document_blocking(
        &inputs,
        &repo.join("reports").join("editorial"),
        "INF-0001",
        NarrateMode::Off,
        None,
    )
    .expect("fixture document renders");

    let expected =
        std::fs::read_to_string(fixtures.join("expected.tex")).expect("expected.tex readable");
    if rendered != expected {
        let diff_line = rendered
            .lines()
            .zip(expected.lines())
            .position(|(a, b)| a != b)
            .map_or_else(
                || "line counts differ".to_owned(),
                |n| format!("line {}", n + 1),
            );
        panic!("rendered document diverges from expected.tex at {diff_line}");
    }
}
