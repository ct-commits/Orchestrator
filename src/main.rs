//! `orchestrator` — CLI.
//!
//! Subcommands:
//!   show   [roadmap.yaml]     Parse one roadmap and print phases + progress
//!   add    <repo-path>        Register a project (a repo containing roadmap.yaml)
//!   report [--out <file>]     Emit a static HTML portfolio report (default report.html)
//!
//! The registry lives at `orchestrator.db` in the current directory, or at
//! $ORCHESTRATOR_DB if set. It is local-first and disposable.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use orchestrator::model::{Roadmap, Status};
use orchestrator::{parser, registry, report};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (cmd, rest) = match args.split_first() {
        Some((c, r)) => (c.as_str(), r),
        None => ("show", &[] as &[String]),
    };

    let result = match cmd {
        "show" => cmd_show(rest.first().map(String::as_str)),
        "add" => cmd_add(rest.first().map(String::as_str)),
        "report" => cmd_report(rest),
        "ingest" => cmd_ingest(rest.first().map(String::as_str)),
        "-h" | "--help" | "help" => {
            print_usage();
            Ok(())
        }
        other => Err(format!("unknown command '{other}'\n\n{USAGE}")),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            eprintln!("error: {msg}");
            ExitCode::FAILURE
        }
    }
}

/// Phase 1 glance: parse one roadmap, print its phases and progress %.
fn cmd_show(path: Option<&str>) -> Result<(), String> {
    let path = path.unwrap_or("roadmap.yaml");
    let roadmap = parser::load(path).map_err(|e| e.to_string())?;

    let p = &roadmap.project;
    println!("{} ({})  —  maturity: {}", p.name, p.repo, p.maturity.as_str());
    println!("{}", "─".repeat(56));
    for phase in &roadmap.phases {
        println!("  {} {:>2}. {}", marker(phase.status), phase.id, phase.name);
    }
    if !roadmap.parked.is_empty() {
        println!("  (parked, not counted:)");
        for item in &roadmap.parked {
            println!("    · {}", item.name);
        }
    }
    let prog = roadmap.progress();
    println!("{}", "─".repeat(56));
    println!(
        "progress: {}/{} phases done  ({:.0}%)",
        prog.done, prog.total, prog.percent
    );
    Ok(())
}

/// Register a project by the path to a repo that contains a roadmap.yaml.
fn cmd_add(path: Option<&str>) -> Result<(), String> {
    let repo_path = path.ok_or("usage: orchestrator add <repo-path>")?;
    let (canonical, roadmap) = load_repo(repo_path)?;

    let conn = open_registry()?;
    let p = &roadmap.project;
    registry::upsert_project(
        &conn,
        &registry::UpsertProject {
            slug: &p.slug,
            name: &p.name,
            repo: Some(p.repo.as_str()),
            repo_path: &canonical,
            maturity: p.maturity.as_str(),
            created: p.created.as_deref(),
        },
    )
    .map_err(|e| e.to_string())?;

    println!("registered {} ({}) at {}", p.name, p.slug, canonical);
    Ok(())
}

/// Read every registered project's roadmap and emit the static HTML report.
fn cmd_report(args: &[String]) -> Result<(), String> {
    let out = parse_out_flag(args)?.unwrap_or_else(|| PathBuf::from("report.html"));

    let conn = open_registry()?;
    let projects = registry::list_projects(&conn).map_err(|e| e.to_string())?;

    // Re-read each roadmap fresh — the repo is the source of truth, not the
    // registry snapshot. Skip a project whose roadmap has gone missing or
    // invalid, with a warning, rather than failing the whole report.
    let mut roadmaps: Vec<(String, Roadmap)> = Vec::new();
    for proj in &projects {
        match parser::load(roadmap_path(&proj.repo_path)) {
            Ok(rm) => roadmaps.push((proj.repo_path.clone(), rm)),
            Err(e) => eprintln!("warning: skipping {} — {e}", proj.name),
        }
    }

    let entries: Vec<report::Entry> = roadmaps
        .iter()
        .map(|(path, rm)| report::Entry {
            roadmap: rm,
            repo_path: path,
        })
        .collect();

    let html = report::render(&entries);
    std::fs::write(&out, html).map_err(|e| format!("could not write {}: {e}", out.display()))?;

    println!(
        "wrote {} ({} project{})",
        out.display(),
        entries.len(),
        if entries.len() == 1 { "" } else { "s" }
    );
    Ok(())
}

/// Ingest delivery history (merged PRs, resolved blockers) for every
/// registered project, or one by `slug`, and cache it in the registry.
/// This is the only path that touches the network (via `gh`); the app
/// reads the cache it writes.
fn cmd_ingest(slug: Option<&str>) -> Result<(), String> {
    let conn = open_registry()?;
    let projects = registry::list_projects(&conn).map_err(|e| e.to_string())?;

    let targets: Vec<_> = match slug {
        Some(s) => projects.iter().filter(|p| p.slug == s).collect(),
        None => projects.iter().collect(),
    };
    if targets.is_empty() {
        return Err(match slug {
            Some(s) => format!("no registered project with slug '{s}'"),
            None => "no projects registered yet — add one with `orchestrator add <repo-path>`".into(),
        });
    }

    for p in targets {
        let repo_url = p.repo.as_ref().map(|r| format!("https://github.com/{r}"));
        let summary =
            orchestrator::ingest::ingest_project(p.repo.as_deref(), &p.repo_path, repo_url);
        let payload = serde_json::to_string(&summary).map_err(|e| e.to_string())?;
        registry::put_cache(&conn, &p.slug, "delivery", &payload, &summary.fetched_at)
            .map_err(|e| e.to_string())?;

        let detail = match summary.source.as_str() {
            "github" => format!(
                "{} merged PRs, {} resolved blockers",
                summary.merged_prs.len(),
                summary.resolved_blockers.len()
            ),
            "git" => format!("{} recent commits (git fallback)", summary.commits.len()),
            _ => summary.note.clone().unwrap_or_else(|| "no data".into()),
        };
        println!("ingested {} [{}] — {}", p.name, summary.source, detail);
    }
    Ok(())
}

// ── helpers ─────────────────────────────────────────────────────────────

const USAGE: &str = "\
usage:
  orchestrator show   [roadmap.yaml]     parse one roadmap, print phases + progress
  orchestrator add    <repo-path>        register a repo that contains roadmap.yaml
  orchestrator report [--out <file>]     emit a static HTML portfolio report
  orchestrator ingest [slug]             fetch delivery history (gh/git) into the cache";

fn print_usage() {
    println!("{USAGE}");
}

fn roadmap_path(repo_path: &str) -> PathBuf {
    Path::new(repo_path).join("roadmap.yaml")
}

/// Canonicalize a repo path and parse its roadmap.yaml.
fn load_repo(repo_path: &str) -> Result<(String, Roadmap), String> {
    let canonical = std::fs::canonicalize(repo_path)
        .map_err(|e| format!("no such repo path '{repo_path}': {e}"))?;
    let roadmap = parser::load(canonical.join("roadmap.yaml")).map_err(|e| e.to_string())?;
    Ok((canonical.to_string_lossy().into_owned(), roadmap))
}

fn open_registry() -> Result<rusqlite::Connection, String> {
    orchestrator::open_default_registry().map_err(|e| e.to_string())
}

fn parse_out_flag(args: &[String]) -> Result<Option<PathBuf>, String> {
    match args {
        [] => Ok(None),
        [flag, value] if flag == "--out" || flag == "-o" => Ok(Some(PathBuf::from(value))),
        [flag] if flag == "--out" || flag == "-o" => Err("--out needs a file path".into()),
        _ => Err(format!("unexpected arguments: {}", args.join(" "))),
    }
}

fn marker(status: Status) -> char {
    match status {
        Status::Done => '✓',
        Status::InProgress => '▸',
        Status::Blocked => '✗',
        Status::Todo => '·',
    }
}
