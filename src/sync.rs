//! Refresh the cache for every registered project — the batch behind the
//! CLI `ingest` (no-arg) and the app's Sync button. Fetches each project's
//! delivery history and writes it to the registry cache. This is the one
//! path that reaches the network; the app reads only the cache it writes.

use rusqlite::Connection;

use crate::{ingest, registry};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Registry(#[from] registry::Error),
    #[error("could not serialize delivery summary: {0}")]
    Json(#[from] serde_json::Error),
}

/// Ingest delivery for every registered project, caching each result.
/// Returns how many projects were refreshed. Ingestion itself never fails
/// per project (it records a degraded summary), so only cache/registry
/// errors surface here.
pub fn ingest_all(conn: &Connection) -> Result<usize, Error> {
    let projects = registry::list_projects(conn)?;
    for p in &projects {
        let repo_url = p.repo.as_ref().map(|r| format!("https://github.com/{r}"));
        let summary = ingest::ingest_project(p.repo.as_deref(), &p.repo_path, repo_url);
        let payload = serde_json::to_string(&summary)?;
        registry::put_cache(conn, &p.slug, "delivery", &payload, &summary.fetched_at)?;
    }
    Ok(projects.len())
}
