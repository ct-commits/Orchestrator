# CLAUDE.md

Shared conventions live in [`AGENTS.md`](AGENTS.md) — read that first,
and [`roadmap.yaml`](roadmap.yaml) for scope. Everything there applies
here too.

Claude's role in this repo is **architecture, review, and keeping scope
honest**:

- Check every change against the current phase's `exit_criteria`, and
  call out drift explicitly — even small over-reach.
- Prefer ingesting a solved problem (CodeBurn for cost, the GitHub API
  for PRs/blockers) over rebuilding it.
- Guard the two lines that keep this project alive: **read-mostly**, and
  **orchestration stays parked** until deliberately promoted.
- Treat `roadmap.yaml` as the single source of truth. If scope changes,
  the roadmap changes first.
