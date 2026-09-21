# Architecture

## What this is

A **read-mostly aggregation layer** that presents the status of many
AI-assisted projects in one place. It is not a coding agent and not a
fork of one.

## Why an aggregator, not a harness

The coding-harness layer (Claude Code, Codex, OpenCode, and the
cross-vendor implement→review loops built on them) is crowded and
commoditised. Token-cost tracking per project is already solved
(CodeBurn); agent tracing is already solved (Langfuse, Helicone, and
similar).

What is *not* solved is the layer above them: a portfolio view for a solo
operator that unifies **roadmap progress + maturity + delivery history +
spend + repo link** per project. That synthesis — especially the
completion/maturity dimension, which the cost and tracing tools ignore —
is the whole point of this project.

Forking a harness would drag the novel part back down into the commodity
layer and inherit that layer's churn. So Orchestrator sits above the
harnesses and reads their outputs.

## Own vs. ingest

| Concern | Source | Built here? |
|---|---|---|
| Project + roadmap model | `roadmap.yaml` per repo + local registry | **Yes** |
| Progress % | computed from roadmap phases | **Yes** |
| Maturity stage | manual field in `roadmap.yaml` | **Yes** |
| Merged PRs, resolved blockers, repo link | GitHub API (git log fallback) | ingest |
| Token usage + cost | CodeBurn JSON | ingest |

## Data model

- **`roadmap.yaml`** lives in each project's repo, version-controlled
  next to the code it describes. It outlives this tool. It is also the
  same artifact a future orchestrator would read — a phase's `goal`/
  `tasks` become an implementer prompt, its `exit_criteria` the scope
  boundary. That keeps the door open without building through it now.
- **Registry (SQLite):** the project list, repo paths, and a cache of
  ingested data. Small, local, disposable. The truth lives in the repos,
  not here.

## Non-goals (for now)

- Running agents or any implement→review loop (parked).
- Cloud, accounts, multi-user, sync.
- Re-implementing cost tracking or tracing that already exists.
