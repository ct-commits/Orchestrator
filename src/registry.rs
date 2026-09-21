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

    #[error("could not create registry directory {path}: {source}")]
    Dir {
        path: String,
        #[source]
        source: std::io::Error,
    },
}

/// A project as stored in the registry. Identity + where to read its
/// `roadmap.yaml`; the roadmap itself stays the source of truth.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredProject {
    pub id: i64,
    pub slug: String,
    pub name: String,
    pub repo: Option<String>,
    pub repo_path: String,
    pub maturity: String,
    pub created: Option<String>,
}

/// Values written when registering (or re-registering) a project. Borrowed
/// so the caller keeps ownership of the parsed roadmap.
#[derive(Debug, Clone)]
pub struct UpsertProject<'a> {
    pub slug: &'a str,
    pub name: &'a str,
    pub repo: Option<&'a str>,
    pub repo_path: &'a str,
    pub maturity: &'a str,
    pub created: Option<&'a str>,
}

/// Register a project, or update it in place if its `slug` already exists.
/// Returns the row id.
pub fn upsert_project(conn: &Connection, p: &UpsertProject) -> Result<i64, Error> {
    conn.execute(
        "INSERT INTO project (slug, name, repo, repo_path, maturity, created)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(slug) DO UPDATE SET
             name = excluded.name,
             repo = excluded.repo,
             repo_path = excluded.repo_path,
             maturity = excluded.maturity,
             created = excluded.created",
        rusqlite::params![p.slug, p.name, p.repo, p.repo_path, p.maturity, p.created],
    )?;
    let id: i64 = conn.query_row(
        "SELECT id FROM project WHERE slug = ?1",
        [p.slug],
        |row| row.get(0),
    )?;
    Ok(id)
}

/// A cached, ingested payload for a project (raw JSON, ingested not
/// re-modelled) plus when it was fetched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheEntry {
    pub payload: String,
    pub fetched_at: String,
}

/// Store (or replace) an ingested payload for a project, keyed by `slug`
/// and `kind` (e.g. "delivery"). No-op-safe to call repeatedly — the
/// latest write wins. Errors if the slug is not registered.
pub fn put_cache(
    conn: &Connection,
    slug: &str,
    kind: &str,
    payload: &str,
    fetched_at: &str,
) -> Result<(), Error> {
    conn.execute(
        "INSERT INTO ingest_cache (project_id, kind, payload, fetched_at)
         SELECT id, ?2, ?3, ?4 FROM project WHERE slug = ?1
         ON CONFLICT(project_id, kind) DO UPDATE SET
             payload = excluded.payload,
             fetched_at = excluded.fetched_at",
        rusqlite::params![slug, kind, payload, fetched_at],
    )?;
    Ok(())
}

/// Read a cached payload for a project, if present.
pub fn get_cache(conn: &Connection, slug: &str, kind: &str) -> Result<Option<CacheEntry>, Error> {
    let mut stmt = conn.prepare(
        "SELECT c.payload, c.fetched_at
         FROM ingest_cache c JOIN project p ON p.id = c.project_id
         WHERE p.slug = ?1 AND c.kind = ?2",
    )?;
    let mut rows = stmt.query(rusqlite::params![slug, kind])?;
    match rows.next()? {
        Some(row) => Ok(Some(CacheEntry {
            payload: row.get(0)?,
            fetched_at: row.get(1)?,
        })),
        None => Ok(None),
    }
}

/// Remove a project by slug. Its cached delivery/cost rows go with it
/// (`ingest_cache` is `ON DELETE CASCADE`, and `open` enables foreign
/// keys). Returns true if a row was removed, false if the slug was unknown.
pub fn remove_project(conn: &Connection, slug: &str) -> Result<bool, Error> {
    let n = conn.execute("DELETE FROM project WHERE slug = ?1", [slug])?;
    Ok(n > 0)
}

/// Every registered project, ordered by name for a stable report.
pub fn list_projects(conn: &Connection) -> Result<Vec<RegisteredProject>, Error> {
    let mut stmt = conn.prepare(
        "SELECT id, slug, name, repo, repo_path, maturity, created
         FROM project ORDER BY name COLLATE NOCASE",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(RegisteredProject {
            id: row.get(0)?,
            slug: row.get(1)?,
            name: row.get(2)?,
            repo: row.get(3)?,
            repo_path: row.get(4)?,
            maturity: row.get(5)?,
            created: row.get(6)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Error::from)
}

/// Open (creating if needed) the registry at `path` and ensure the schema
/// is present. Enables foreign-key enforcement, which SQLite leaves off by
/// default.
pub fn open(path: impl AsRef<Path>) -> Result<Connection, Error> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|source| Error::Dir {
                path: parent.display().to_string(),
                source,
            })?;
        }
    }
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
