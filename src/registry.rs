//! The local project registry — a small, disposable SQLite database.
//!
//! The truth lives in each repo's `roadmap.yaml`; this registry only holds
//! the list of projects, where their repos are on disk, and a cache of
//! data ingested from elsewhere (git/GitHub/CodeBurn) in later phases.
//! Delete the file and nothing of value is lost — it rebuilds from the
//! repos.
//!
//! Phase 1 defines the schema and opens the database. Populating and
//! reading it back is the job of later phases (the static report, the
//! Tauri shell, ingestion); this module deliberately stops at the shape.

use std::path::Path;

use rusqlite::Connection;

/// Schema version for the registry file itself (distinct from the roadmap
/// schema_version). Bumped when these tables change.
pub const REGISTRY_VERSION: i64 = 1;

/// Table definitions. `ingest_cache` is where later phases stash GitHub /
/// CodeBurn payloads keyed by project and kind; unused in Phase 1.
const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS project (
    id         INTEGER PRIMARY KEY,
    slug       TEXT NOT NULL UNIQUE,
    name       TEXT NOT NULL,
    repo       TEXT,                       -- owner/name, may be null for local-only
    repo_path  TEXT NOT NULL,              -- absolute path to the repo on disk
    maturity   TEXT NOT NULL,
    created    TEXT
);

CREATE TABLE IF NOT EXISTS ingest_cache (
    id         INTEGER PRIMARY KEY,
    project_id INTEGER NOT NULL REFERENCES project(id) ON DELETE CASCADE,
    kind       TEXT NOT NULL,              -- e.g. 'github_prs', 'codeburn'
    payload    TEXT NOT NULL,              -- raw JSON, ingested not re-modelled
    fetched_at TEXT NOT NULL,              -- ISO-8601 timestamp
    UNIQUE (project_id, kind)
);
"#;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("registry database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
}

/// Open (creating if needed) the registry at `path` and ensure the schema
/// is present. Enables foreign-key enforcement, which SQLite leaves off by
/// default.
pub fn open(path: impl AsRef<Path>) -> Result<Connection, Error> {
    let conn = Connection::open(path)?;
    init(&conn)?;
    Ok(conn)
}

/// Open an in-memory registry — used by tests and for a schema smoke check.
pub fn open_in_memory() -> Result<Connection, Error> {
    let conn = Connection::open_in_memory()?;
    init(&conn)?;
    Ok(conn)
}

fn init(conn: &Connection) -> Result<(), Error> {
    conn.pragma_update(None, "foreign_keys", true)?;
    conn.execute_batch(SCHEMA)?;
    conn.pragma_update(None, "user_version", REGISTRY_VERSION)?;
    Ok(())
}
