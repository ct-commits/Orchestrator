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
    // maturity is a manual judgement field that moves over time; just
    // require it to be one of the valid stages rather than pinning a value.
    assert!(matches!(
        roadmap.project.maturity,
        Maturity::Idea | Maturity::Prototype | Maturity::Working | Maturity::Production
    ));
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
    // The view mirrors whatever maturity the roadmap currently declares.
    assert_eq!(v.maturity, roadmap.project.maturity.as_str());
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
fn parses_open_prs_with_null_merged_at() {
    use orchestrator::ingest;
    // Open PRs come back with mergedAt: null — parse_prs handles it.
    let json = r#"[{"number": 9, "title": "WIP feature", "url": "https://x/pull/9", "mergedAt": null}]"#;
    let prs = ingest::parse_prs(json).unwrap();
    assert_eq!(prs.len(), 1);
    assert_eq!(prs[0].number, 9);
    assert_eq!(prs[0].merged_at, None);
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
        open_prs: vec![PrRef {
            number: 2,
            title: "In flight".into(),
            url: "https://github.com/acme/demo/pull/2".into(),
            merged_at: None,
        }],
        last_activity: Some("2026-09-20T08:00:00Z".into()),
        commits: vec![],
        fetched_at: "2026-09-21T10:00:00Z".into(),
        note: None,
    };
    let payload = serde_json::to_string(&summary).unwrap();
    registry::put_cache(&conn, "demo", "delivery", &payload, &summary.fetched_at).unwrap();

    let got = orchestrator::view::delivery(&conn, "demo").unwrap().unwrap();
    assert_eq!(got, summary);
    assert_eq!(got.merged_prs[0].number, 1);
    assert_eq!(got.open_prs[0].number, 2);

    // last_activity also surfaces on the ProjectView (a placeholder here,
    // since /repos/demo has no roadmap) for at-a-glance freshness.
    let views = orchestrator::view::portfolio(&conn).unwrap();
    assert_eq!(views[0].last_activity.as_deref(), Some("2026-09-20T08:00:00Z"));
}

const SAMPLE_EXPORT: &str = r#"{
  "schema": "codeburn.export.v2",
  "generated": "2026-09-21T10:14:23.082Z",
  "currency": {"code": "USD", "rate": 1, "symbol": "$"},
  "projects": [{"Project": "C:\\Codex-Prosjekter\\toolbox", "Cost (USD)": 114.19}],
  "records": [
    {"project": "C:\\Codex-Prosjekter\\toolbox", "cost": 100.0, "inputTokens": 10, "outputTokens": 20, "reasoningTokens": 5, "cacheWriteTokens": 1, "cacheReadTokens": 1000, "provider": "claude", "model": "Opus 4.8"},
    {"project": "C:\\Codex-Prosjekter\\toolbox", "cost": 14.19, "inputTokens": 2, "outputTokens": 3, "reasoningTokens": 0, "cacheWriteTokens": 0, "cacheReadTokens": 500},
    {"project": "C:\\Codex-Prosjekter", "cost": 293.98, "inputTokens": 1, "outputTokens": 1, "reasoningTokens": 0, "cacheWriteTokens": 0, "cacheReadTokens": 0}
  ]
}"#;

#[test]
fn parses_and_aggregates_codeburn_export() {
    use orchestrator::cost;
    let ing = cost::parse_export(SAMPLE_EXPORT).unwrap();
    assert_eq!(ing.generated, "2026-09-21T10:14:23.082Z");
    assert_eq!(ing.projects.len(), 2); // two distinct project paths

    let tb = cost::match_project(&ing.projects, "C:/Codex-Prosjekter/toolbox").unwrap();
    assert_eq!(tb.api_calls, 2); // two records summed
    assert!((tb.cost_usd - 114.19).abs() < 1e-9);
    assert_eq!(tb.tokens.input, 12);
    assert_eq!(tb.tokens.output, 23);
    assert_eq!(tb.tokens.cache_read, 1500);
    assert_eq!(tb.tokens.total(), 12 + 23 + 5 + 1 + 1500);
}

#[test]
fn cost_path_matching_normalizes() {
    use orchestrator::cost;
    let ing = cost::parse_export(SAMPLE_EXPORT).unwrap();

    // Windows verbatim prefix, backslashes, trailing sep, and case all match.
    assert!(cost::match_project(&ing.projects, r"\\?\C:\Codex-Prosjekter\TOOLBOX\").is_some());
    // A registered repo with no CodeBurn bucket → no cost data.
    assert!(cost::match_project(&ing.projects, "C:/Codex-Prosjekter/Orchestrator/Orchestrator").is_none());
}

#[test]
fn rejects_wrong_export_schema() {
    use orchestrator::cost;
    let bad = r#"{"schema": "codeburn.export.v1", "records": []}"#;
    assert!(matches!(
        cost::parse_export(bad),
        Err(orchestrator::cost::Error::Schema { .. })
    ));
}

#[test]
fn cost_rides_along_in_the_project_view() {
    use orchestrator::cost::{CostRecord, TokenTotals};

    // A registered project with a cached cost record surfaces in portfolio().
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("roadmap.yaml"),
        "schema_version: 1\nproject: {name: Demo, slug: demo, repo: acme/demo, maturity: idea}\nphases:\n  - {id: 1, name: One, status: todo, goal: g, exit_criteria: e}\n",
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
            maturity: "idea",
            created: None,
        },
    )
    .unwrap();

    let record = CostRecord {
        cost_usd: 12.5,
        api_calls: 3,
        tokens: TokenTotals { input: 1, output: 2, reasoning: 0, cache_write: 0, cache_read: 7 },
        generated: "2026-09-21T10:00:00Z".into(),
        source_path: dir.path().to_string_lossy().into_owned(),
    };
    registry::put_cache(
        &conn,
        "demo",
        "cost",
        &serde_json::to_string(&record).unwrap(),
        &record.generated,
    )
    .unwrap();

    let views = orchestrator::view::portfolio(&conn).unwrap();
    assert_eq!(views.len(), 1);
    let cost = views[0].cost.as_ref().expect("cost should ride along");
    assert_eq!(cost.cost_usd, 12.5);
    assert_eq!(cost.tokens.total(), 10);
}

#[test]
fn scaffold_slugify_and_remote_parsing() {
    use orchestrator::scaffold::{parse_remote, slugify};

    assert_eq!(slugify("My Project 5"), "my-project-5");
    assert_eq!(slugify("Interaktivt BMC!!"), "interaktivt-bmc");
    assert_eq!(slugify("__weird__"), "weird");
    assert_eq!(slugify("!!!"), "project"); // fallback

    assert_eq!(
        parse_remote("https://github.com/ct-commits/Orchestrator.git"),
        Some("ct-commits/Orchestrator".into())
    );
    assert_eq!(
        parse_remote("git@github.com:ct-commits/Orchestrator.git"),
        Some("ct-commits/Orchestrator".into())
    );
    assert_eq!(
        parse_remote("https://gitlab.com/group/sub\n"),
        Some("group/sub".into())
    );
    assert_eq!(parse_remote("not-a-url"), None);
}

#[test]
fn scaffold_prompt_embeds_schema_and_path() {
    let prompt = orchestrator::scaffold::prompt("C:/repos/foo", Some("acme/foo"));
    assert!(prompt.contains("C:/repos/foo"));
    assert!(prompt.contains("acme/foo"));
    assert!(prompt.contains("roadmap.yaml"));
    // The authoritative schema is embedded so the agent produces valid YAML.
    assert!(prompt.contains("\"$schema\""));
    assert!(prompt.contains("exit_criteria"));
}

#[test]
fn portfolio_emits_a_placeholder_for_a_repo_without_a_roadmap() {
    // A registered repo path with no roadmap.yaml on disk.
    let dir = tempfile::tempdir().unwrap();
    let conn = registry::open_in_memory().unwrap();
    registry::upsert_project(
        &conn,
        &registry::UpsertProject {
            slug: "foo",
            name: "Foo",
            repo: Some("acme/foo"),
            repo_path: &dir.path().to_string_lossy(),
            maturity: "unknown",
            created: None,
        },
    )
    .unwrap();

    let views = orchestrator::view::portfolio(&conn).unwrap();
    assert_eq!(views.len(), 1);
    assert!(!views[0].has_roadmap);
    assert_eq!(views[0].name, "Foo");
    assert!(views[0].phases.is_empty());
    assert_eq!(views[0].progress.total, 0);
    assert!(views[0].issue.is_none()); // missing, not invalid
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
fn remove_project_deletes_row_and_cascades_cache() {
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
    registry::put_cache(&conn, "demo", "delivery", "{}", "2026-09-21T00:00:00Z").unwrap();
    registry::put_cache(&conn, "demo", "cost", "{}", "2026-09-21T00:00:00Z").unwrap();

    // Removing an unknown slug is a no-op that reports false.
    assert!(!registry::remove_project(&conn, "nope").unwrap());

    assert!(registry::remove_project(&conn, "demo").unwrap());
    assert!(registry::list_projects(&conn).unwrap().is_empty());
    // Cached rows cascaded away with the project.
    let cache_rows: i64 = conn
        .query_row("SELECT count(*) FROM ingest_cache", [], |r| r.get(0))
        .unwrap();
    assert_eq!(cache_rows, 0);
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
