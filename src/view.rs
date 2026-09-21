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
    /// Cached CodeBurn cost, if ingested. `None` = no cost data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost: Option<crate::cost::CostRecord>,
    /// Repo's last-activity timestamp from the cached delivery summary, for
    /// an at-a-glance freshness signal. `None` until delivery is ingested.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_activity: Option<String>,
    /// False for a registered repo that has no (or an invalid) roadmap.yaml
    /// yet — the app renders it as a placeholder "run scaffold" card.
    pub has_roadmap: bool,
    /// A human note for a placeholder (e.g. why the roadmap didn't load).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub issue: Option<String>,
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
            cost: None,
            last_activity: None,
            has_roadmap: true,
            issue: None,
        }
    }

    /// A placeholder view for a registered repo whose roadmap.yaml is
    /// missing or invalid. `issue` is `None` for "no roadmap yet".
    pub fn placeholder(p: &registry::RegisteredProject, issue: Option<String>) -> Self {
        ProjectView {
            name: p.name.clone(),
            slug: p.slug.clone(),
            repo: p.repo.clone().unwrap_or_default(),
            repo_path: p.repo_path.clone(),
            maturity: p.maturity.clone(),
            progress: ProgressView {
                done: 0,
                total: 0,
                percent: 0.0,
            },
            phases: Vec::new(),
            parked: Vec::new(),
            cost: None,
            last_activity: None,
            has_roadmap: false,
            issue,
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
        // Missing roadmap -> placeholder ("no roadmap yet"); present but
        // invalid -> placeholder carrying the reason; valid -> full view.
        let mut view = if !path.exists() {
            ProjectView::placeholder(p, None)
        } else {
            match parser::load(&path) {
                Ok(rm) => ProjectView::from_roadmap(&rm, &p.repo_path),
                Err(e) => ProjectView::placeholder(p, Some(e.to_string())),
            }
        };
        view.cost = cost(conn, &p.slug)?;
        view.last_activity = delivery(conn, &p.slug)?.and_then(|d| d.last_activity);
        views.push(view);
    }
    Ok(views)
}

/// The cached CodeBurn cost record for one project, if ingested. Reads only
/// the local cache — no network, no CodeBurn dependency at read time.
pub fn cost(
    conn: &Connection,
    slug: &str,
) -> Result<Option<crate::cost::CostRecord>, Error> {
    match registry::get_cache(conn, slug, "cost")? {
        Some(entry) => Ok(Some(serde_json::from_str(&entry.payload).map_err(Error::Json)?)),
        None => Ok(None),
    }
}
