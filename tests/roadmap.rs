//! Phase 1 exit criteria, as tests:
//! "This repo's own roadmap.yaml parses and validates, and the parser
//!  returns the correct phase list and progress % for it."

use orchestrator::model::{Maturity, Status};
use orchestrator::{parser, registry};

/// This repo's own roadmap — the reference instance we dogfood on.
const OWN_ROADMAP: &str = include_str!("../roadmap.yaml");

#[test]
fn own_roadmap_parses_and_validates() {
    let roadmap = parser::parse_str(OWN_ROADMAP).expect("own roadmap must parse and validate");

    assert_eq!(roadmap.schema_version, 1);
    assert_eq!(roadmap.project.slug, "orchestrator");
    assert_eq!(roadmap.project.repo, "ct-commits/Orchestrator");
    assert_eq!(roadmap.project.maturity, Maturity::Idea);
}

#[test]
fn returns_the_correct_phase_list() {
    let roadmap = parser::parse_str(OWN_ROADMAP).unwrap();

    let names: Vec<&str> = roadmap.phases.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "Schema & parser",
            "Static portfolio report",
            "Tauri shell",
            "Git & GitHub ingestion",
            "Token & cost ledger",
            "Working-tool cutline",
        ]
    );

    // Ids are sequential 1..=6 and Phase 1 is the one in progress.
    let ids: Vec<i64> = roadmap.phases.iter().map(|p| p.id).collect();
    assert_eq!(ids, [1, 2, 3, 4, 5, 6]);
    assert_eq!(roadmap.phases[0].status, Status::InProgress);
}

#[test]
fn computes_progress_correctly() {
    let roadmap = parser::parse_str(OWN_ROADMAP).unwrap();
    let prog = roadmap.progress();

    // Nothing is `done` yet: 0 of 6 committed phases.
    assert_eq!(prog.done, 0);
    assert_eq!(prog.total, 6);
    assert_eq!(prog.percent, 0.0);
}

#[test]
fn parked_items_do_not_count_toward_progress() {
    let roadmap = parser::parse_str(OWN_ROADMAP).unwrap();

    assert_eq!(roadmap.parked.len(), 1);
    assert_eq!(roadmap.parked[0].name, "Orchestration hooks");
    // total reflects committed phases only, not parked ideas.
    assert_eq!(roadmap.progress().total, roadmap.phases.len());
}

#[test]
fn progress_reflects_done_phases() {
    // Same shape as the real roadmap, but with phases already completed,
    // to prove the percentage tracks `status: done`.
    let yaml = r#"
schema_version: 1
project:
  name: Demo
  slug: demo
  repo: acme/demo
  maturity: prototype
phases:
  - id: 1
    name: One
    status: done
    goal: g
    exit_criteria: e
  - id: 2
    name: Two
    status: done
    goal: g
    exit_criteria: e
  - id: 3
    name: Three
    status: in_progress
    goal: g
    exit_criteria: e
  - id: 4
    name: Four
    status: todo
    goal: g
    exit_criteria: e
"#;
    let prog = parser::parse_str(yaml).unwrap().progress();
    assert_eq!(prog.done, 2);
    assert_eq!(prog.total, 4);
    assert_eq!(prog.percent, 50.0);
}

#[test]
fn rejects_documents_that_violate_the_schema() {
    // Unknown top-level field (additionalProperties: false) must fail.
    let bad = r#"
schema_version: 1
project:
  name: Demo
  slug: demo
  repo: acme/demo
  maturity: prototype
phases:
  - id: 1
    name: One
    status: todo
    goal: g
    exit_criteria: e
surprise: not allowed
"#;
    assert!(matches!(
        parser::parse_str(bad),
        Err(parser::Error::Invalid(_))
    ));
}

#[test]
fn rejects_bad_enum_values() {
    // `maturity: legendary` is not in the enum.
    let bad = r#"
schema_version: 1
project:
  name: Demo
  slug: demo
  repo: acme/demo
  maturity: legendary
phases:
  - id: 1
    name: One
    status: todo
    goal: g
    exit_criteria: e
"#;
    assert!(matches!(
        parser::parse_str(bad),
        Err(parser::Error::Invalid(_))
    ));
}

#[test]
fn registry_schema_initialises() {
    // The registry opens and its tables exist.
    let conn = registry::open_in_memory().expect("registry must initialise");
    let count: i64 = conn
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name IN ('project','ingest_cache')",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 2);
}
