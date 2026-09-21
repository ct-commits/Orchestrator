//! Orchestrator — a local-first portfolio dashboard.
//!
//! This crate owns the one thing the tool is responsible for: the
//! project/roadmap model. It parses and validates a `roadmap.yaml`,
//! computes progress, and defines the local SQLite registry. Everything
//! else (git/GitHub, token cost) is ingested in later phases.
//!
//! ```no_run
//! let roadmap = orchestrator::parser::load("roadmap.yaml")?;
//! let progress = roadmap.progress();
//! println!("{}/{} phases done ({:.0}%)", progress.done, progress.total, progress.percent);
//! # Ok::<(), orchestrator::parser::Error>(())
//! ```

pub mod cost;
pub mod ingest;
pub mod model;
pub mod parser;
pub mod registry;
pub mod report;
pub mod view;

pub use model::{Maturity, Parked, Phase, Progress, Project, Roadmap, Status};

/// Path to the local registry. `$ORCHESTRATOR_DB` wins if set; otherwise a
/// stable per-user location (`<data-dir>/orchestrator/registry.db`), so the
/// CLI and the app agree no matter which directory they run from — and the
/// file never lands inside a watched build folder. Local-first, disposable.
pub fn registry_path() -> std::path::PathBuf {
    if let Some(explicit) = std::env::var_os("ORCHESTRATOR_DB") {
        return std::path::PathBuf::from(explicit);
    }
    user_data_dir()
        .map(|d| d.join("orchestrator").join("registry.db"))
        .unwrap_or_else(|| std::path::PathBuf::from("orchestrator.db"))
}

/// The per-user data directory, without pulling in a crate for it:
/// `%APPDATA%` on Windows, `$XDG_DATA_HOME` or `~/.local/share` elsewhere.
fn user_data_dir() -> Option<std::path::PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("APPDATA").map(std::path::PathBuf::from)
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("XDG_DATA_HOME")
            .map(std::path::PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".local/share"))
            })
    }
}

/// Open the default registry (see [`registry_path`]), creating and
/// migrating it if needed. Shared by the CLI and the Tauri app.
pub fn open_default_registry() -> Result<rusqlite::Connection, registry::Error> {
    registry::open(registry_path())
}
