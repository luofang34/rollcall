#![allow(clippy::expect_used, clippy::panic)]

use super::{
    NarrateMode, NarrativeProvider, clean, format_digest, instruction_for, render_section,
};

/// A provider that always drafts fixed prose.
struct Fake(&'static str);
impl NarrativeProvider for Fake {
    fn draft(&self, _section: &str, _instruction: &str, _context: &str) -> Option<String> {
        Some(self.0.to_owned())
    }
    fn name(&self) -> &str {
        "fake"
    }
}

/// A provider that always declines (e.g. the model errored / overflowed).
struct Silent;
impl NarrativeProvider for Silent {
    fn draft(&self, _section: &str, _instruction: &str, _context: &str) -> Option<String> {
        None
    }
    fn name(&self) -> &str {
        "silent"
    }
}

#[test]
fn mode_default_is_auto() {
    assert_eq!(NarrateMode::default(), NarrateMode::Auto);
}

#[test]
fn only_status_driven_sections_are_narratable() {
    assert!(instruction_for("executive-summary.tex").is_some());
    assert!(instruction_for("findings.tex").is_some());
    // Structural / unknown sections stay authored.
    assert!(instruction_for("architecture.tex").is_none());
    assert!(instruction_for("nonexistent.tex").is_none());
}

#[test]
fn off_never_narrates_even_with_a_provider() {
    let fake = Fake("drafted");
    assert!(render_section(NarrateMode::Off, Some(&fake), "findings.tex", "digest").is_none());
}

#[test]
fn auto_without_a_provider_falls_back() {
    assert!(render_section(NarrateMode::Auto, None, "findings.tex", "digest").is_none());
}

#[test]
fn auto_narrates_narratable_sections_but_not_authored_ones() {
    let fake = Fake("drafted prose");
    let drafted = render_section(NarrateMode::Auto, Some(&fake), "executive-summary.tex", "d");
    assert_eq!(drafted.as_deref(), Some("drafted prose"));
    // ...never the authored-only architecture note, even with a provider.
    assert!(render_section(NarrateMode::Auto, Some(&fake), "architecture.tex", "d").is_none());
}

#[test]
fn a_declining_provider_falls_back() {
    assert!(render_section(NarrateMode::Auto, Some(&Silent), "findings.tex", "d").is_none());
}

#[test]
fn clean_strips_markdown_and_trims() {
    assert_eq!(clean("  **Bold** and __under__\n\n"), "Bold and under");
}

#[test]
fn digest_states_the_numbers_it_hands_the_model() {
    let hosts = [("seshat".to_owned(), 19), ("yesod".to_owned(), 27)];
    let digest = format_digest(
        "2026-07-09",
        "Bayt al-Hikmah",
        8,
        9,
        &["Sibyl (GPU node)"],
        &hosts,
        46,
        "1.85",
        "1,850",
        "0.30",
    );
    assert!(digest.contains("8 of 9 up"), "{digest}");
    assert!(digest.contains("Down: Sibyl (GPU node)."), "{digest}");
    assert!(digest.contains("- seshat: 19 guests"), "{digest}");
    assert!(digest.contains("guests total 46"), "{digest}");
    assert!(digest.contains("1.85 kW"), "{digest}");
}
