# AGENTS.md — working conventions for this repo

Orchestrator is a **local-first portfolio dashboard**. Read
[`roadmap.yaml`](roadmap.yaml) first — it is the source of truth for
scope.

## Golden rules

1. **One phase at a time.** Work the phase currently marked
   `in_progress` in `roadmap.yaml`. Don't begin a later phase until it
   is promoted.
2. **`exit_criteria` is the boundary.** A change is in scope only if it
   serves the current phase's `exit_criteria`. Anything past that line is
   scope drift — stop and flag it rather than pressing on.
3. **Own the model, ingest the rest.** Don't re-instrument token
   tracking (ingest CodeBurn) and don't rebuild tracing/observability.
   Build only the project/roadmap model and the view over it.
4. **Read-mostly.** The tool reads and displays. It does not trigger
   agents or run any loop. Orchestration is *parked* (see
   `roadmap.yaml`), and stays parked until deliberately promoted.
5. **Local-first.** No cloud, no auth, no multi-user. Data is files in
   repos plus a local SQLite registry.

## Workflow

- One roadmap phase = one branch = one PR.
- Every phase must end in a **usable** state, per its `exit_criteria`.
- Keep each change single-concern.
- If scope genuinely needs to change, change `roadmap.yaml` **first**,
  then the code — never the reverse.

## Stack

Chosen per phase; see the roadmap. Expected shape: a parser/validator for
`roadmap.yaml` (validated against `roadmap.schema.json`), a SQLite
registry, a static HTML renderer as the first proof, then a Tauri shell,
with GitHub API + CodeBurn JSON for ingestion.
