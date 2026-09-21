# Orchestrator

A **local-first portfolio dashboard** for a solo developer juggling many
AI-assisted projects. It answers one question at a glance: *where does
each of my projects actually stand?*

It owns exactly one thing — the **project/roadmap model** — and ingests
everything else (git history, PRs, token cost) rather than rebuilding it.

## Why

The coding-harness layer (Claude Code, Codex, and the implement→review
loops on top of them) is crowded, and per-project token cost (CodeBurn)
and agent tracing (Langfuse, Helicone) are already solved. What isn't is
the layer *above* them: a portfolio view that unifies **roadmap progress
+ maturity + delivery history + spend + repo link** per project. The
completion/maturity dimension — which cost and tracing tools ignore — is
the whole point.

So Orchestrator sits above the harnesses and reads their outputs. It is a
read-mostly aggregator, not a coding agent and not a fork of one.

## How it works

Each project carries a [`roadmap.yaml`](roadmap.yaml) in its own repo,
version-controlled next to the code it describes. The dashboard reads
these and **computes** the glance — it does not store it:

- **Progress %** = phases with `status: done` ÷ total committed phases.
- **Maturity** = a manual judgement field (`idea → prototype → working →
  production`).

`roadmap.yaml` is the single source of truth. It is validated against
[`roadmap.schema.json`](roadmap.schema.json) (JSON Schema 2020-12). A
small local **SQLite registry** holds the project list, repo paths, and a
cache of ingested data — disposable; the truth lives in the repos.

### Own vs. ingest

| Concern | Source | Built here? |
|---|---|---|
| Project + roadmap model | `roadmap.yaml` + local registry | **Yes** |
| Progress % | computed from roadmap phases | **Yes** |
| Maturity stage | manual field in `roadmap.yaml` | **Yes** |
| Merged PRs, resolved blockers, repo link | GitHub API (git log fallback) | ingest |
| Token usage + cost | CodeBurn JSON | ingest |

## Stack

**Rust** for the whole core. The parser/registry becomes a Tauri command
directly once the app shell lands (Tauri's backend is Rust), so there's
no runtime to bundle and no rewrite at the app boundary. Memory-safe, no
shipped runtime, small dependency surface, no network — matching the
read-mostly, local-first constraints.

- Parser/validator — `serde_yaml_ng` + `jsonschema`
- Registry — SQLite via bundled `rusqlite`
- First proof — a static HTML report; then a Tauri shell

## Usage

Parse and validate a roadmap and print its phase list + progress %
(defaults to this repo's own `roadmap.yaml`):

```bash
cargo run -- show [path/to/roadmap.yaml]
```

Register projects and emit a static HTML portfolio report. A repo with no
`roadmap.yaml` is registered as a placeholder; `scaffold` prints a prompt
you paste into a coding agent (Claude Code / Codex) to generate one:

```bash
cargo run -- add <path/to/a/repo>
cargo run -- scaffold <path/to/a/repo>   # prints an agent prompt; writes nothing
cargo run -- report --out report.html
```

Ingest delivery history (merged PRs / resolved blockers via `gh`) and a
CodeBurn cost export (Settings → Export → JSON) into the local cache:

```bash
cargo run -- ingest
cargo run -- cost --import <path/to/CodeBurn-Export.json>
```

Run the desktop app (Tauri) — the same read model in a local window.
Needs Node (for the Svelte frontend) and the Tauri prerequisites
(WebView2 on Windows):

```bash
npm install
npm start        # = tauri dev
```

The registry is a per-user SQLite file (`<data-dir>/orchestrator/
registry.db`, e.g. `%APPDATA%` on Windows), so the CLI and the app share
it wherever they run; override with `ORCHESTRATOR_DB`. Run the tests with
`cargo test`.

## Roadmap

Committed scope lives in [`roadmap.yaml`](roadmap.yaml) and is the source
of truth. In short:

1. **Schema & parser** — lock the schema + parser; compute progress. *(done)*
2. **Static portfolio report** — one command emits a static HTML portfolio view. *(done)*
3. **Tauri shell** — the same read model as a real local window. *(done)*
4. **Git & GitHub ingestion** — merged PRs, resolved blockers, repo link. *(done)*
5. **Token & cost ledger** — per-project usage/cost from CodeBurn. *(done)*
6. **Working-tool cutline** — daily-usable, scope frozen; the maturity gate. *(in progress)*

## Principles

- **One phase at a time**, gated by each phase's `exit_criteria`. Scope
  changes go in `roadmap.yaml` first, then the code.
- **Read-mostly.** The tool reads and displays; it triggers no agents.
  Orchestration is deliberately *parked* until the dashboard earns it.
- **Local-first.** No cloud, no auth, no multi-user. Files in repos plus a
  local SQLite registry.

See [`AGENTS.md`](AGENTS.md) for working conventions and
[`docs/architecture.md`](docs/architecture.md) for the full rationale.
