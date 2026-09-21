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

    // Ids are sequential 1..=6.
    let ids: Vec<i64> = roadmap.phases.iter().map(|p| p.id).collect();
    assert_eq!(ids, [1, 2, 3, 4, 5, 6]);
}

#[test]
fn computes_progress_correctly() {
    let roadmap = parser::parse_str(OWN_ROADMAP).unwrap();
    let prog = roadmap.progress();

    // Consistent with the live phase list, so this doesn't break each time
    // a phase is completed: done == phases marked done, total == all phases.
    let expected_done = roadmap
        .phases
        .iter()
        .filter(|p| p.status == Status::Done)
        .count();
    assert_eq!(prog.done, expected_done);
    assert_eq!(prog.total, 6);
    assert_eq!(prog.percent, (expected_done as f64 / 6.0) * 100.0);
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
fn report_renders_accurate_progress_bars() {
    use orchestrator::report::{self, Entry};

    let roadmap = parser::parse_str(OWN_ROADMAP).unwrap();
    let prog = roadmap.progress();
    let html = report::render(&[Entry {
        roadmap: &roadmap,
        repo_path: "/tmp/orchestrator",
    }]);

    // A self-contained document: no framework, no network references.
    assert!(html.starts_with("<!DOCTYPE html>"));
    assert!(!html.contains("http://") && !html.contains("https://"));
    assert!(!html.contains("<script"));

    // The bar width matches the computed progress, and the project shows.
    let pct = prog.percent.round() as i64;
    assert!(html.contains(&format!("width:{pct}%")));
    assert!(html.contains(&format!("{}/{} phases", prog.done, prog.total)));
    assert!(html.contains("Orchestrator"));
    assert!(html.contains("Schema &amp; parser")); // HTML-escaped phase name
}

#[test]
fn report_handles_an_empty_portfolio() {
    use orchestrator::report;
    let html = report::render(&[]);
    assert!(html.contains("No projects registered yet."));
    assert!(html.starts_with("<!DOCTYPE html>"));
}

#[test]
fn registry_upsert_and_list_round_trips() {
    let conn = registry::open_in_memory().unwrap();

    let insert = |slug: &str, name: &str| {
        registry::upsert_project(
            &conn,
            &registry::UpsertProject {
                slug,
                name,
                repo: Some("acme/demo"),
                repo_path: "/repos/demo",
                maturity: "idea",
                created: None,
            },
        )
        .unwrap()
    };

    let id1 = insert("demo", "Demo");
    // Re-registering the same slug updates in place (same id), no duplicate.
    let id2 = insert("demo", "Demo Renamed");
    assert_eq!(id1, id2);

    let projects = registry::list_projects(&conn).unwrap();
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].name, "Demo Renamed");
    assert_eq!(projects[0].repo.as_deref(), Some("acme/demo"));
}

#[test]
fn project_view_mirrors_the_roadmap() {
    use orchestrator::view::ProjectView;

    let roadmap = parser::parse_str(OWN_ROADMAP).unwrap();
    let v = ProjectView::from_roadmap(&roadmap, "/repos/orchestrator");

    assert_eq!(v.name, "Orchestrator");
    assert_eq!(v.slug, "orchestrator");
    assert_eq!(v.repo_path, "/repos/orchestrator");
    assert_eq!(v.maturity, "idea");
    assert_eq!(v.phases.len(), roadmap.phases.len());
    assert_eq!(v.phases[0].status, "done"); // Phase 1 is done
    assert_eq!(v.progress.total, 6);
    assert_eq!(v.parked, vec!["Orchestration hooks".to_string()]);

    // Serializes to the JSON shape the UI consumes.
    let json = serde_json::to_value(&v).unwrap();
    assert_eq!(json["progress"]["total"], 6);
    assert_eq!(json["phases"][0]["status"], "done");
}

#[test]
fn portfolio_reads_registered_roadmaps_from_disk() {
    // A registered repo path whose roadmap.yaml is read back through the
    // view layer — the same path the Tauri command drives.
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("roadmap.yaml"),
        r#"
schema_version: 1
project: {name: Demo, slug: demo, repo: acme/demo, maturity: prototype}
phases:
  - {id: 1, name: One, status: done, goal: g, exit_criteria: e}
  - {id: 2, name: Two, status: todo, goal: g, exit_criteria: e}
"#,
    )
    .unwrap();

    let conn = registry::open_in_memory().unwrap();
    registry::upsert_project(
        &conn,
        &registry::UpsertProject {
            slug: "demo",
            name: "Demo",
            repo: Some("acme/demo"),
            repo_path: &dir.path().to_string_lossy(),
            maturity: "prototype",
            created: None,
        },
    )
    .unwrap();

    let views = orchestrator::view::portfolio(&conn).unwrap();
    assert_eq!(views.len(), 1);
    assert_eq!(views[0].name, "Demo");
    assert_eq!(views[0].progress.done, 1);
    assert_eq!(views[0].progress.total, 2);
    assert_eq!(views[0].progress.percent, 50.0);
}

#[test]
fn parses_gh_pr_json() {
    use orchestrator::ingest;
    let json = r#"[
        {"number": 3, "title": "Phase 3: Tauri shell", "url": "https://github.com/ct-commits/Orchestrator/pull/3", "mergedAt": "2026-09-21T10:00:00Z"},
        {"number": 2, "title": "Phase 2", "url": "https://github.com/ct-commits/Orchestrator/pull/2", "mergedAt": "2026-09-21T09:00:00Z"}
    ]"#;
    let prs = ingest::parse_prs(json).unwrap();
    assert_eq!(prs.len(), 2);
    assert_eq!(prs[0].number, 3);
    assert_eq!(prs[0].merged_at.as_deref(), Some("2026-09-21T10:00:00Z"));
    assert!(prs[0].url.ends_with("/pull/3"));
}

#[test]
fn parses_gh_blocker_json() {
    use orchestrator::ingest;
    let json = r#"[{"number": 7, "title": "DB migration blocks build", "url": "https://x/issues/7", "closedAt": "2026-09-20T12:00:00Z"}]"#;
    let issues = ingest::parse_blockers(json).unwrap();
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].number, 7);
    assert_eq!(issues[0].closed_at.as_deref(), Some("2026-09-20T12:00:00Z"));
}

#[test]
fn parses_git_log_unit_separated() {
    use orchestrator::ingest;
    // hash \x1f subject \x1f date, one commit per line
    let text = "abcdef1234567\u{1f}Add parser\u{1f}2026-09-21\n0011223344556\u{1f}Fix loop\u{1f}2026-09-20";
    let commits = ingest::parse_git_log(text);
    assert_eq!(commits.len(), 2);
    assert_eq!(commits[0].hash, "abcdef123"); // truncated to 9
    assert_eq!(commits[0].subject, "Add parser");
    assert_eq!(commits[1].date, "2026-09-20");
}

#[test]
fn delivery_cache_round_trips_through_the_view() {
    use orchestrator::ingest::{DeliverySummary, PrRef};

    let conn = registry::open_in_memory().unwrap();
    registry::upsert_project(
        &conn,
        &registry::UpsertProject {
            slug: "demo",
            name: "Demo",
            repo: Some("acme/demo"),
            repo_path: "/repos/demo",
            maturity: "idea",
            created: None,
        },
    )
    .unwrap();

    // Nothing cached yet.
    assert!(orchestrator::view::delivery(&conn, "demo").unwrap().is_none());

    let summary = DeliverySummary {
        source: "github".into(),
        repo_url: Some("https://github.com/acme/demo".into()),
        merged_prs: vec![PrRef {
            number: 1,
            title: "First".into(),
            url: "https://github.com/acme/demo/pull/1".into(),
            merged_at: Some("2026-09-01T00:00:00Z".into()),
        }],
        resolved_blockers: vec![],
        commits: vec![],
        fetched_at: "2026-09-21T10:00:00Z".into(),
        note: None,
    };
    let payload = serde_json::to_string(&summary).unwrap();
    registry::put_cache(&conn, "demo", "delivery", &payload, &summary.fetched_at).unwrap();

    let got = orchestrator::view::delivery(&conn, "demo").unwrap().unwrap();
    assert_eq!(got, summary);
    assert_eq!(got.merged_prs[0].number, 1);
}

#[test]
fn registry_creates_missing_parent_dirs() {
    // The default registry lives in the per-user data dir, which may not
    // exist yet — open() must create the whole parent chain.
    let dir = tempfile::tempdir().unwrap();
    let nested = dir.path().join("orchestrator").join("registry.db");
    assert!(!nested.parent().unwrap().exists());

    let conn = registry::open(&nested).unwrap();
    drop(conn);
    assert!(nested.exists());
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
