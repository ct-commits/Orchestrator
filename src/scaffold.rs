//! Onboarding helpers: derive a project slug from a path, parse a git
//! remote into `owner/name`, and build an agent-ready prompt that gets a
//! coding agent to write a repo's `roadmap.yaml`.
//!
//! Orchestrator itself writes nothing into other repos — `scaffold` only
//! emits text. The agent (Claude Code, Codex, …) reads the repo and writes
//! the file, using the embedded schema as the contract.

use crate::parser::SCHEMA_SOURCE;

/// Turn a directory or project name into a schema-valid slug
/// (`^[a-z0-9-]+$`): lowercase, non-alphanumerics collapsed to single
/// dashes, trimmed. Falls back to `project` if nothing is left.
pub fn slugify(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut prev_dash = false;
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    let trimmed = out.trim_matches('-');
    if trimmed.is_empty() {
        "project".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Parse a git remote URL into `owner/name`, dropping any `.git` suffix.
/// Handles both `https://github.com/owner/name(.git)` and
/// `git@github.com:owner/name(.git)`. `None` if it doesn't look like one.
pub fn parse_remote(url: &str) -> Option<String> {
    let url = url.trim();
    let tail = if let Some(rest) = url.split_once("://") {
        // scheme://host/owner/name  ->  owner/name
        rest.1.split_once('/').map(|(_, p)| p)?
    } else if let Some((_, rest)) = url.split_once('@') {
        // git@host:owner/name  ->  owner/name
        rest.split_once(':').map(|(_, p)| p)?
    } else {
        return None;
    };
    let tail = tail.strip_suffix(".git").unwrap_or(tail).trim_matches('/');
    let mut parts = tail.split('/').filter(|s| !s.is_empty());
    let owner = parts.next()?;
    let name = parts.next()?;
    if owner.is_empty() || name.is_empty() {
        None
    } else {
        Some(format!("{owner}/{name}"))
    }
}

/// Build the prompt to hand a coding agent so it writes `roadmap.yaml` for
/// the repo at `repo_path`. Embeds the authoritative JSON Schema so the
/// agent produces a document that will actually validate.
pub fn prompt(repo_path: &str, repo: Option<&str>) -> String {
    let repo_line = match repo {
        Some(r) => format!("The repo is `{r}`.\n"),
        None => String::new(),
    };
    format!(
        "You are onboarding this project into Orchestrator, a local-first \
portfolio dashboard. Write a file named `roadmap.yaml` at the root of the \
repository at `{repo_path}`.\n\
{repo_line}\n\
Rules:\n\
- Analyse the actual repository (its code, README, git history, TODOs) and \
describe *this* project honestly — do not invent scope.\n\
- The file MUST validate against the JSON Schema below.\n\
- Break the work into a small number of sequential phases. Each phase's \
`exit_criteria` must be an objective, testable definition of done that also \
serves as the scope boundary for that phase.\n\
- Set exactly one phase's `status` to reflect reality (`done` for finished \
work, `in_progress` for what's underway, `todo` for the rest).\n\
- `maturity` is a judgement call: idea | prototype | working | production.\n\
- Put genuinely-deferred ideas under `parked` (excluded from progress), not \
in `phases`.\n\
- Keep prose terse. Write only the file; do not add commentary.\n\n\
Progress % is computed as phases with status `done` / total committed \
phases, so only list phases you actually intend to deliver.\n\n\
JSON Schema (roadmap.schema.json):\n\
```json\n{SCHEMA_SOURCE}\n```\n"
    )
}
