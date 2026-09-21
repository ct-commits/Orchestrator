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

pub use model::{Maturity, Parked, Phase, Progress, Project, Roadmap, Status};
