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

pub mod model;
pub mod parser;
pub mod registry;
pub mod report;
pub mod view;

pub use model::{Maturity, Parked, Phase, Progress, Project, Roadmap, Status};

/// Path to the local registry: `$ORCHESTRATOR_DB`, or `orchestrator.db` in
/// the current directory. Local-first and disposable.
pub fn registry_path() -> std::path::PathBuf {
    std::env::var_os("ORCHESTRATOR_DB")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("orchestrator.db"))
}

/// Open the default registry (see [`registry_path`]), creating and
/// migrating it if needed. Shared by the CLI and the Tauri app.
pub fn open_default_registry() -> Result<rusqlite::Connection, registry::Error> {
    registry::open(registry_path())
}
