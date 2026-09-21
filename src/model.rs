//! The project/roadmap model — the one thing Orchestrator owns.
//!
//! These types mirror `roadmap.schema.json`. The schema is the wire
//! contract (validated first); these structs are the typed view the rest
//! of the tool works against.

use serde::{Deserialize, Serialize};

/// Manual judgement call — where a project sits on its way to being real.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Maturity {
    Idea,
    Prototype,
    Working,
    Production,
}

impl Maturity {
    /// The lowercase label used in `roadmap.yaml` and stored in the registry.
    pub fn as_str(self) -> &'static str {
        match self {
            Maturity::Idea => "idea",
            Maturity::Prototype => "prototype",
            Maturity::Working => "working",
            Maturity::Production => "production",
        }
    }
}

/// A phase's lifecycle state. Only `Done` counts toward progress %.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Todo,
    InProgress,
    Blocked,
    Done,
}

impl Status {
    /// The snake_case label used in `roadmap.yaml` and sent to the UI.
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Todo => "todo",
            Status::InProgress => "in_progress",
            Status::Blocked => "blocked",
            Status::Done => "done",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    pub slug: String,
    /// `owner/name`, e.g. `ct-commits/Orchestrator`.
    pub repo: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub maturity: Maturity,
    /// ISO-8601 date, kept as a string to match the schema's `format: date`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Phase {
    pub id: i64,
    pub name: String,
    pub status: Status,
    pub goal: String,
    #[serde(default)]
    pub tasks: Vec<String>,
    /// Defines "done" objectively and declares the scope boundary.
    pub exit_criteria: String,
}

/// An idea deliberately excluded from progress %. Promoted into `phases`
/// only when committed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Parked {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub why_later: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Roadmap {
    pub schema_version: u32,
    pub project: Project,
    pub phases: Vec<Phase>,
    #[serde(default)]
    pub parked: Vec<Parked>,
}

/// The computed glance: how many committed phases are done, and the
/// percentage. `parked` is not counted — it is not committed scope.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Progress {
    pub done: usize,
    pub total: usize,
    pub percent: f64,
}

impl Roadmap {
    /// Committed phases marked `done`.
    pub fn done_count(&self) -> usize {
        self.phases
            .iter()
            .filter(|p| p.status == Status::Done)
            .count()
    }

    /// Progress % = done phases ÷ total committed phases. An empty phase
    /// list is 0% (the schema forbids it, but we never divide by zero).
    pub fn progress(&self) -> Progress {
        let done = self.done_count();
        let total = self.phases.len();
        let percent = if total == 0 {
            0.0
        } else {
            (done as f64 / total as f64) * 100.0
        };
        Progress {
            done,
            total,
            percent,
        }
    }
}
