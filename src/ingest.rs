//! Ingest delivery history — merged PRs, open PRs, recent commits, and last
//! activity from GitHub (via the `gh` CLI, reusing the user's existing
//! auth), with a `git log` fallback for local-only repos. Recent commits
//! mean repos that push straight to `main` (no PRs) still show real work.
//!
//! This is *ingestion*, not orchestration: it runs only when explicitly
//! invoked (the `ingest` CLI command) and writes a cached summary to the
//! registry. The app reads that cache and never touches the network — the
//! read-mostly line holds.
//!
//! The JSON parsers are pure and unit-tested; the subprocess runners are
//! thin wrappers around them.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PrRef {
    pub number: i64,
    pub title: String,
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merged_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommitRef {
    pub hash: String,
    pub subject: String,
    pub date: String,
}

/// The per-project delivery summary that is cached and shown in the app.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeliverySummary {
    /// "github", "git", or "none".
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo_url: Option<String>,
    #[serde(default)]
    pub merged_prs: Vec<PrRef>,
    /// PRs currently open (in flight).
    #[serde(default)]
    pub open_prs: Vec<PrRef>,
    /// When the repo was last pushed to / last committed (ISO-8601).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_activity: Option<String>,
    /// Populated by the git-log fallback when GitHub is unavailable.
    #[serde(default)]
    pub commits: Vec<CommitRef>,
    pub fetched_at: String,
    /// A human note when something degraded (e.g. gh missing, git failed).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("could not run {tool}: {source}")]
    Spawn {
        tool: String,
        #[source]
        source: std::io::Error,
    },
    #[error("{tool} failed: {msg}")]
    Tool { tool: String, msg: String },
    #[error("could not parse {tool} output as JSON: {source}")]
    Json {
        tool: String,
        #[source]
        source: serde_json::Error,
    },
}

// ── pure parsers ────────────────────────────────────────────────────────

// gh emits camelCase JSON keys; map them to our snake_case fields.
#[derive(Deserialize)]
struct GhPr {
    number: i64,
    title: String,
    url: String,
    #[serde(rename = "mergedAt")]
    merged_at: Option<String>,
}

/// Parse `gh pr list --json number,title,url,mergedAt`.
pub fn parse_prs(json: &str) -> Result<Vec<PrRef>, Error> {
    let raw: Vec<GhPr> = serde_json::from_str(json).map_err(|source| Error::Json {
        tool: "gh".into(),
        source,
    })?;
    Ok(raw
        .into_iter()
        .map(|p| PrRef {
            number: p.number,
            title: p.title,
            url: p.url,
            merged_at: p.merged_at,
        })
        .collect())
}

// GitHub's commits API shape (the subset we use).
#[derive(Deserialize)]
struct GhCommitEntry {
    sha: String,
    commit: GhCommitInner,
}
#[derive(Deserialize)]
struct GhCommitInner {
    message: String,
    committer: GhCommitWho,
}
#[derive(Deserialize)]
struct GhCommitWho {
    date: String,
}

/// Parse `gh api repos/{repo}/commits` into recent commits.
pub fn parse_gh_commits(json: &str) -> Result<Vec<CommitRef>, Error> {
    let raw: Vec<GhCommitEntry> = serde_json::from_str(json).map_err(|source| Error::Json {
        tool: "gh".into(),
        source,
    })?;
    Ok(raw
        .into_iter()
        .map(|e| CommitRef {
            hash: e.sha.chars().take(9).collect(),
            subject: e.commit.message.lines().next().unwrap_or("").to_string(),
            date: e.commit.committer.date.chars().take(10).collect(), // YYYY-MM-DD
        })
        .collect())
}

/// Parse `git log --pretty=format:%H%x1f%s%x1f%cs` (fields unit-separated).
pub fn parse_git_log(text: &str) -> Vec<CommitRef> {
    text.lines()
        .filter_map(|line| {
            let mut parts = line.split('\u{1f}');
            let hash = parts.next()?.trim();
            let subject = parts.next()?;
            let date = parts.next().unwrap_or("");
            if hash.is_empty() {
                return None;
            }
            Some(CommitRef {
                hash: hash.chars().take(9).collect(),
                subject: subject.to_string(),
                date: date.to_string(),
            })
        })
        .collect()
}

// ── subprocess runners ──────────────────────────────────────────────────

fn run(tool: &str, args: &[&str]) -> Result<String, Error> {
    let out = std::process::Command::new(tool)
        .args(args)
        .output()
        .map_err(|source| Error::Spawn {
            tool: tool.into(),
            source,
        })?;
    if !out.status.success() {
        return Err(Error::Tool {
            tool: tool.into(),
            msg: String::from_utf8_lossy(&out.stderr).trim().to_string(),
        });
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn gh_available() -> bool {
    run("gh", &["--version"]).is_ok()
}

fn gh_prs(repo: &str, state: &str) -> Result<Vec<PrRef>, Error> {
    let json = run(
        "gh",
        &[
            "pr", "list", "--repo", repo, "--state", state, "--limit", "20", "--json",
            "number,title,url,mergedAt",
        ],
    )?;
    parse_prs(&json)
}

/// Recent commits on the repo's default branch, via the GitHub API.
fn gh_recent_commits(repo: &str) -> Result<Vec<CommitRef>, Error> {
    let json = run("gh", &["api", &format!("repos/{repo}/commits?per_page=20")])?;
    parse_gh_commits(&json)
}

/// The repo's last push time (ISO-8601), via `gh repo view`.
fn gh_last_activity(repo: &str) -> Option<String> {
    let out = run(
        "gh",
        &["repo", "view", repo, "--json", "pushedAt", "--jq", ".pushedAt"],
    )
    .ok()?;
    let s = out.trim();
    if s.is_empty() || s == "null" {
        None
    } else {
        Some(s.to_string())
    }
}

fn git_recent_commits(repo_path: &str) -> Result<Vec<CommitRef>, Error> {
    let text = run(
        "git",
        &[
            "-C",
            repo_path,
            "log",
            "-n",
            "20",
            "--pretty=format:%H%x1f%s%x1f%cs",
        ],
    )?;
    Ok(parse_git_log(&text))
}

/// Ingest one project. Prefers GitHub via `gh` when a `repo` is known and
/// `gh` is available; otherwise falls back to `git log` on the local repo.
/// Never returns an error for a normal "no data" case — it records what it
/// found and notes any degradation.
pub fn ingest_project(repo: Option<&str>, repo_path: &str, repo_url: Option<String>) -> DeliverySummary {
    let fetched_at = now_utc_iso();

    if let Some(repo) = repo {
        if gh_available() {
            let merged = gh_prs(repo, "merged");
            let open = gh_prs(repo, "open");
            if let (Ok(merged_prs), Ok(open_prs)) = (&merged, &open) {
                return DeliverySummary {
                    source: "github".into(),
                    repo_url: repo_url.or_else(|| Some(format!("https://github.com/{repo}"))),
                    merged_prs: merged_prs.clone(),
                    open_prs: open_prs.clone(),
                    last_activity: gh_last_activity(repo),
                    // Also show recent commits — repos that push straight to
                    // main (no PRs) still surface their real work.
                    commits: gh_recent_commits(repo).unwrap_or_default(),
                    fetched_at,
                    note: None,
                };
            }
            // gh present but a call failed — fall back to git, keep the reason.
            let reason = merged.err().or_else(|| open.err()).map(|e| e.to_string());
            return git_fallback(repo_path, repo_url, fetched_at, reason);
        }
    }
    git_fallback(repo_path, repo_url, fetched_at, None)
}

fn git_fallback(
    repo_path: &str,
    repo_url: Option<String>,
    fetched_at: String,
    reason: Option<String>,
) -> DeliverySummary {
    match git_recent_commits(repo_path) {
        Ok(commits) => DeliverySummary {
            source: "git".into(),
            repo_url,
            merged_prs: Vec::new(),
            open_prs: Vec::new(),
            // Most recent commit's date stands in for last activity.
            last_activity: commits.first().map(|c| c.date.clone()),
            commits,
            fetched_at,
            note: reason.map(|r| format!("GitHub unavailable ({r}); showing local git log")),
        },
        Err(e) => DeliverySummary {
            source: "none".into(),
            repo_url,
            merged_prs: Vec::new(),
            open_prs: Vec::new(),
            last_activity: None,
            commits: Vec::new(),
            fetched_at,
            note: Some(format!("no delivery data: {e}")),
        },
    }
}

// ── timestamp (no external crate) ───────────────────────────────────────

/// Current UTC time as an ISO-8601 string, computed from the system clock
/// without pulling in a date crate.
fn now_utc_iso() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let (h, mi, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

/// Howard Hinnant's days-from-civil, inverted: days since 1970-01-01 to
/// (year, month, day). Public-domain algorithm.
fn civil_from_days(z: i64) -> (i64, u64, u64) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u64;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u64;
    (if m <= 2 { y + 1 } else { y }, m, d)
}
