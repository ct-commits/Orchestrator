//! Serializable views over the model — the shape the UI (and any JSON
//! consumer) receives.
//!
//! This is the read model the Tauri command hands to the frontend. Keeping
//! it in the library (not in the Tauri crate) means it is plain,
//! dependency-light, and unit-testable without a running app. The Tauri
//! command is then a thin wrapper around [`portfolio`].

use rusqlite::Connection;
use serde::Serialize;

use crate::model::Roadmap;
use crate::{parser, registry};

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ProgressView {
    pub done: usize,
    pub total: usize,
    pub percent: f64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PhaseView {
    pub id: i64,
    pub name: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ProjectView {
    pub name: String,
    pub slug: String,
    pub repo: String,
    pub repo_path: String,
    pub maturity: String,
    pub progress: ProgressView,
    pub phases: Vec<PhaseView>,
    pub parked: Vec<String>,
}

impl ProjectView {
    /// Build a view from a parsed roadmap and where it lives on disk.
    pub fn from_roadmap(rm: &Roadmap, repo_path: &str) -> Self {
        let prog = rm.progress();
        ProjectView {
            name: rm.project.name.clone(),
            slug: rm.project.slug.clone(),
            repo: rm.project.repo.clone(),
            repo_path: repo_path.to_string(),
            maturity: rm.project.maturity.as_str().to_string(),
            progress: ProgressView {
                done: prog.done,
                total: prog.total,
                percent: prog.percent,
            },
            phases: rm
                .phases
                .iter()
                .map(|p| PhaseView {
                    id: p.id,
                    name: p.name.clone(),
                    status: p.status.as_str().to_string(),
                })
                .collect(),
            parked: rm.parked.iter().map(|p| p.name.clone()).collect(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Registry(#[from] registry::Error),

    #[error("cached delivery payload is not valid JSON: {0}")]
    Json(serde_json::Error),
}

/// The cached delivery summary for one project, if it has been ingested.
/// Reads only the local cache — no network — so the app stays read-mostly.
pub fn delivery(
    conn: &Connection,
    slug: &str,
) -> Result<Option<crate::ingest::DeliverySummary>, Error> {
    match registry::get_cache(conn, slug, "delivery")? {
        Some(entry) => {
            let summary = serde_json::from_str(&entry.payload).map_err(Error::Json)?;
            Ok(Some(summary))
        }
        None => Ok(None),
    }
}

/// Every registered project, as a view. A project whose `roadmap.yaml` has
/// gone missing or invalid is skipped (with a warning to stderr) rather
/// than failing the whole portfolio — the same read-mostly, degrade-
/// gracefully behaviour as the static report.
pub fn portfolio(conn: &Connection) -> Result<Vec<ProjectView>, Error> {
    let projects = registry::list_projects(conn)?;
    let mut views = Vec::with_capacity(projects.len());
    for p in &projects {
        let path = std::path::Path::new(&p.repo_path).join("roadmap.yaml");
        match parser::load(&path) {
            Ok(rm) => views.push(ProjectView::from_roadmap(&rm, &p.repo_path)),
            Err(e) => eprintln!("warning: skipping {} — {e}", p.name),
        }
    }
    Ok(views)
}
