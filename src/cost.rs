//! Ingest CodeBurn's JSON export (schema `codeburn.export.v2`) into a
//! per-project token + cost ledger.
//!
//! CodeBurn 0.9.24 ships no CLI or stdout JSON — it exports a file from
//! Settings → Export. We read that file. Per-project totals are summed
//! from the granular `records` (each record is one API call, carrying its
//! own token counts and cost); CodeBurn keys every record by the absolute
//! launch-directory path, which we match to a registry project's repo path.
//!
//! Ingestion is explicit (the `cost` CLI command); the app only reads the
//! cached result. The parser is pure and unit-tested.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// The export schema this parser understands.
pub const EXPECTED_SCHEMA: &str = "codeburn.export.v2";

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct TokenTotals {
    pub input: i64,
    pub output: i64,
    pub reasoning: i64,
    pub cache_write: i64,
    pub cache_read: i64,
}

impl TokenTotals {
    pub fn total(&self) -> i64 {
        self.input + self.output + self.reasoning + self.cache_write + self.cache_read
    }
}

/// One project's aggregated cost, keyed by the CodeBurn launch path.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectCost {
    pub path: String,
    pub cost_usd: f64,
    pub api_calls: i64,
    pub tokens: TokenTotals,
}

/// The cached per-registry-project cost record — what the app reads back.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CostRecord {
    pub cost_usd: f64,
    pub api_calls: i64,
    pub tokens: TokenTotals,
    /// The export's `generated` timestamp (when CodeBurn wrote it).
    pub generated: String,
    /// The CodeBurn project path this was matched from.
    pub source_path: String,
}

/// A parsed export: when it was generated, and the per-project buckets.
#[derive(Debug, Clone)]
pub struct Ingested {
    pub generated: String,
    pub projects: Vec<ProjectCost>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("could not read CodeBurn export {path}: {source}")]
    Read {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("CodeBurn export is not valid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unsupported CodeBurn export schema {found:?}; expected {EXPECTED_SCHEMA:?}. Re-export from Settings > Export.")]
    Schema { found: String },
}

#[derive(Deserialize)]
struct Export {
    #[serde(default)]
    schema: String,
    #[serde(default)]
    generated: String,
    #[serde(default)]
    records: Vec<Record>,
}

// Each record is one API call. CodeBurn uses camelCase keys; unknown keys
// (sessionId, timestamp, provider, model, …) are ignored.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Record {
    #[serde(default)]
    project: String,
    #[serde(default)]
    cost: f64,
    #[serde(default)]
    input_tokens: i64,
    #[serde(default)]
    output_tokens: i64,
    #[serde(default)]
    reasoning_tokens: i64,
    #[serde(default)]
    cache_write_tokens: i64,
    #[serde(default)]
    cache_read_tokens: i64,
}

/// Parse a CodeBurn export from JSON text, aggregating records per project.
pub fn parse_export(json: &str) -> Result<Ingested, Error> {
    let export: Export = serde_json::from_str(json)?;
    if !export.schema.is_empty() && export.schema != EXPECTED_SCHEMA {
        return Err(Error::Schema {
            found: export.schema,
        });
    }

    let mut by_path: BTreeMap<String, ProjectCost> = BTreeMap::new();
    for r in &export.records {
        let bucket = by_path.entry(r.project.clone()).or_insert_with(|| ProjectCost {
            path: r.project.clone(),
            cost_usd: 0.0,
            api_calls: 0,
            tokens: TokenTotals::default(),
        });
        bucket.cost_usd += r.cost;
        bucket.api_calls += 1;
        bucket.tokens.input += r.input_tokens;
        bucket.tokens.output += r.output_tokens;
        bucket.tokens.reasoning += r.reasoning_tokens;
        bucket.tokens.cache_write += r.cache_write_tokens;
        bucket.tokens.cache_read += r.cache_read_tokens;
    }

    Ok(Ingested {
        generated: export.generated,
        projects: by_path.into_values().collect(),
    })
}

/// Parse a CodeBurn export from a file path.
pub fn load_export(path: impl AsRef<std::path::Path>) -> Result<Ingested, Error> {
    let path = path.as_ref();
    let json = std::fs::read_to_string(path).map_err(|source| Error::Read {
        path: path.display().to_string(),
        source,
    })?;
    parse_export(&json)
}

/// Normalise a filesystem path for cross-source comparison: strip Windows
/// `\\?\` verbatim prefix, unify separators, lowercase (Windows paths are
/// case-insensitive), and drop any trailing separator.
pub fn normalize_path(p: &str) -> String {
    let p = p.strip_prefix(r"\\?\").unwrap_or(p);
    let mut s = p.replace('\\', "/").to_lowercase();
    while s.len() > 1 && s.ends_with('/') {
        s.pop();
    }
    s
}

/// Find the export bucket whose path matches a registry project's repo
/// path. `None` means the project has no cost data — the caller renders
/// that as zero/none.
pub fn match_project<'a>(projects: &'a [ProjectCost], repo_path: &str) -> Option<&'a ProjectCost> {
    let target = normalize_path(repo_path);
    projects
        .iter()
        .find(|p| normalize_path(&p.path) == target)
}
