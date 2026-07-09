#![allow(clippy::expect_used, clippy::panic)]

use super::{Risk, Severity, assess, parse_risks, render};
use crate::narrative::NarrativeProvider;

/// A provider that returns a fixed JSON risk register.
struct JsonFake(&'static str);
impl NarrativeProvider for JsonFake {
    fn draft(&self, _section: &str, _instruction: &str, _context: &str) -> Option<String> {
        None
    }
    fn draft_json(&self, _instruction: &str, _context: &str, _schema: &str) -> Option<String> {
        Some(self.0.to_owned())
    }
    fn name(&self) -> &str {
        "json-fake"
    }
}

#[test]
fn severity_parses_and_defaults_unknown_to_medium() {
    assert_eq!(Severity::parse("Critical"), Severity::Critical);
    assert_eq!(Severity::parse(" HIGH "), Severity::High);
    assert_eq!(Severity::parse("low"), Severity::Low);
    assert_eq!(Severity::parse("weird"), Severity::Medium);
}

#[test]
fn parse_risks_reads_valid_and_drops_malformed() {
    let json = r#"{"risks":[
        {"title":"Sibyl down","severity":"low","recommendation":"leave it, deliberate"},
        {"title":"","severity":"high","recommendation":"empty title dropped"},
        {"severity":"high","recommendation":"missing title dropped"}
    ]}"#;
    let risks = parse_risks(json);
    assert_eq!(risks.len(), 1);
    assert_eq!(risks[0].title, "Sibyl down");
    assert_eq!(risks[0].severity, Severity::Low);
}

#[test]
fn parse_risks_on_garbage_is_empty() {
    assert!(parse_risks("not json").is_empty());
    assert!(parse_risks(r#"{"nope":1}"#).is_empty());
}

#[test]
fn assess_sorts_by_severity() {
    let json = r#"{"risks":[
        {"title":"no collectors","severity":"medium","recommendation":"deploy Redfish"},
        {"title":"capex estimates","severity":"low","recommendation":"gather invoices"},
        {"title":"switch has no creds","severity":"high","recommendation":"add credentials"}
    ]}"#;
    let context = "no hardware collectors; capex all estimates; switch has no creds";
    let severities: Vec<Severity> = assess(&JsonFake(json), context)
        .iter()
        .map(|risk| risk.severity)
        .collect();
    assert_eq!(
        severities,
        [Severity::High, Severity::Medium, Severity::Low]
    );
}

#[test]
fn assess_drops_a_risk_with_a_fabricated_figure() {
    let json = r#"{"risks":[
        {"title":"99 disks failing","severity":"critical","recommendation":"replace them"},
        {"title":"switch has no creds","severity":"high","recommendation":"add credentials"}
    ]}"#;
    // The context never mentions 99, so that risk is a fabrication.
    let risks = assess(&JsonFake(json), "switch has no creds");
    assert_eq!(risks.len(), 1);
    assert_eq!(risks[0].title, "switch has no creds");
}

#[test]
fn assess_with_empty_context_is_empty() {
    let json = r#"{"risks":[{"title":"x","severity":"high","recommendation":"y"}]}"#;
    assert!(assess(&JsonFake(json), "").is_empty());
}

#[test]
fn render_is_blank_when_empty_and_a_section_otherwise() {
    assert_eq!(render(&[]), "");
    let risks = [Risk {
        title: "Sibyl deliberately down".to_owned(),
        severity: Severity::Low,
        recommendation: "monitor; no action".to_owned(),
    }];
    let tex = render(&risks);
    assert!(tex.contains(r"\section{Risk Register}"), "{tex}");
    assert!(tex.contains("Sibyl deliberately down"), "{tex}");
    assert!(tex.contains(r"\textbf{LOW}"), "{tex}");
}
