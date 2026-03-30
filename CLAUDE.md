# Cassette — Claude Code Onboarding

Cassette is a local-first desktop music application for managing, organizing, auditing, and acquiring a personal audio library. Built in Rust + Tauri 2 + SvelteKit with a pipeline architecture.

## Current Phase

Active hardening — audit completeness proof, packaging confidence, provenance reuse and review. Core pipeline is operational with 7 providers live.

## Repository Structure

```
Cargo.toml              # workspace root (members: src-tauri, crates/cassette-core)
crates/cassette-core/   # shared Rust domain logic (all pipeline modules)
src-tauri/              # Tauri 2 desktop shell, command surface, state wiring
ui/                     # SvelteKit frontend
docs/                   # canonical project documentation (19+ reference docs)
scripts/                # smoke tests, cleanup pipeline, maintenance helpers
```

## Six Pipeline Modules (all in `crates/cassette-core/src/`)

| Module | Role |
|--------|------|
| `librarian/` | Scan, import, normalize, classify library state |
| `custodian/` | Organize, validate, quarantine, stage files safely |
| `orchestrator/` | Reconcile desired vs local state, plan work |
| `director/` | Acquire media through 7 providers (waterfall, scoring, concurrency) |
| `gatekeeper/` | Validate and admit staged arrivals |
| `library/` | Operational management, locking, recovery, observability |

Supporting modules: `metadata/`, `validation/`, `db/`, `models/`, `player/`, `sources/`, `downloader/` (legacy, slated for cleanup).

## Key Technical Decisions

- **Rust workspace with shared core crate** — business logic stays out of the Tauri shell
- **Local-first SQLite** — rusqlite for runtime, sqlx for custodian subsystem, WAL mode
- **Pipeline-oriented domain** — Librarian → Custodian → Orchestrator → Director → Gatekeeper
- **Safety through staging/quarantine/validation** — no destructive file mutation without rollback
- **Auditability first-class** — operation events, lineage tracking, provable automation
- **Provider diversity** — 7 providers with trust ranking, waterfall orchestration, health monitoring
- **blake3 checksums** at every handoff
- **One provider per album, always** — no split albums

See `docs/DECISIONS.md` for full rationale (14 decisions logged).

## Anti-Rules

- No new CLI commands until existing ones are proven
- No new enrichment sources until metadata story is formalized
- No new scoring dimensions until the experience layer ships
- Documentation must reflect observed runtime truth, not aspirations

## Default Paths

| Purpose | Path |
|---------|------|
| Source library | `A:\music` |
| Canonical library | `A:\music_sorted` |
| Staging | `A:\music_staging` |
| Quarantine | `A:\music_quarantine` |
| Admin/DB | `A:\music_admin` |

## Build & Run

```powershell
# Check workspace compiles
cargo check --workspace

# Run tests
cargo test

# Build frontend
Set-Location ui; npm run build; Set-Location ..

# Run desktop app
cargo tauri dev

# Smoke test
.\scripts\smoke_desktop.ps1
```

- `cargo check` succeeds with warnings.
- `cargo test` is not currently green — known gap.
- Repo-root `*.db` files are local artifacts, not the live runtime database.

## Docs

Canonical project docs are in `docs/`, not the repo root. Before substantial work, read:

1. `docs/PROJECT_INDEX.md` — project map and status
2. `docs/PROJECT_STATE.md` — current runtime capabilities
3. `docs/TODO.md` — living task list
4. `docs/DECISIONS.md` — architectural rationale
5. `docs/AGENT_CODEX.md` — agent operating manual
6. `docs/AGENT_BRIEFING.md` — quick onboarding

## Behavioral Contract

See `AGENTS.md` for repo-local agent instructions. Verify current state before inheriting claims from docs.
