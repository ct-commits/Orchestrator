//! Tauri shell for Orchestrator.
//!
//! The shell owns no model logic — it exposes the library's read model to
//! the frontend through a single command and opens a window. Reading stays
//! local-first: the same `roadmap.yaml` files and local SQLite registry
//! the CLI uses. It triggers no agents (orchestration is parked).

use orchestrator::ingest::DeliverySummary;
use orchestrator::view::ProjectView;

/// Return every registered project as a view for the UI. Reads the local
/// registry and each project's `roadmap.yaml`; errors surface to the
/// frontend as a string.
#[tauri::command]
fn list_projects() -> Result<Vec<ProjectView>, String> {
    let conn = orchestrator::open_default_registry().map_err(|e| e.to_string())?;
    orchestrator::view::portfolio(&conn).map_err(|e| e.to_string())
}

/// Return the cached delivery summary (merged PRs, open PRs, last activity, or a
/// git-log fallback) for one project, if it has been ingested. Reads only
/// the local cache — no network — so the app stays read-mostly. `null`
/// means "not ingested yet".
#[tauri::command]
fn get_delivery(slug: String) -> Result<Option<DeliverySummary>, String> {
    let conn = orchestrator::open_default_registry().map_err(|e| e.to_string())?;
    orchestrator::view::delivery(&conn, &slug).map_err(|e| e.to_string())
}

/// Fetch fresh delivery data for every registered project (reaching the
/// network via `gh`), cache it, and return the refreshed portfolio. This
/// is the Sync button — a manual, user-initiated refresh, not a loop.
#[tauri::command]
fn sync() -> Result<Vec<ProjectView>, String> {
    let conn = orchestrator::open_default_registry().map_err(|e| e.to_string())?;
    orchestrator::sync::ingest_all(&conn).map_err(|e| e.to_string())?;
    orchestrator::view::portfolio(&conn).map_err(|e| e.to_string())
}

/// Unregister a project (and its cached data) by slug. The project's own
/// roadmap.yaml on disk is untouched. Returns true if a row was removed.
/// This is the app's one write path — user-initiated portfolio management,
/// not portfolio work; it triggers nothing.
#[tauri::command]
fn remove_project(slug: String) -> Result<bool, String> {
    let conn = orchestrator::open_default_registry().map_err(|e| e.to_string())?;
    orchestrator::registry::remove_project(&conn, &slug).map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            list_projects,
            get_delivery,
            sync,
            remove_project
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
