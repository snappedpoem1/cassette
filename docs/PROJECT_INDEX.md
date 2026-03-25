# Cassette Project Index

**Status**: Active — hardening and provider proof
**Next**: Deezer full-track path, async recovery hardening, packaging confidence
**Last Updated**: 2026-03-25
**Owner**: Christian (Capn)

---

## Project Overview

Cassette is a private, local-first music application for managing, organizing, auditing, and acquiring
a personal audio library. It combines:

- A desktop shell in Tauri 2
- A SvelteKit renderer
- A shared Rust domain crate for library, acquisition, validation, and operational logic
- A local SQLite database with WAL support

At its best, Cassette answers five questions clearly:

1. What music do I already have?
2. What music do I want?
3. What is missing or low quality?
4. What happened to every file or download attempt?
5. Can I recover safely when a stage fails?

---

## Repository Layout

```text
Cassette/
├── Cargo.toml                  # workspace root
├── crates/
│   └── cassette-core/          # shared Rust domain logic
├── src-tauri/                  # desktop shell and Tauri commands
├── ui/                         # SvelteKit frontend
├── docs/                       # project documentation
├── scripts/                    # smoke tests and maintenance helpers
└── test_data/                  # test fixtures (e.g. spotify_export.json)
```

Canonical implementation layers:

- `crates/cassette-core` — scanning, organization, downloads, validation, metadata, orchestration, locking
- `src-tauri` — app boot, command surface, state wiring, desktop plugins
- `ui` — end-user interface
- `scripts` — smoke runs, sandbox reset, operational helpers

---

## Architecture At A Glance

### Product Surface

```text
UI (SvelteKit)
  ->
Tauri command layer
  ->
cassette-core
  ->
SQLite + local filesystem + external metadata/download providers
```

### Library Workflow

```text
Desired state / user intent / import data
  ->
[Librarian]    Scan, normalize, classify local files
  ->
[Custodian]    Organize, validate, quarantine, stage safely
  ->
[Orchestrator] Reconcile desired vs local and plan work
  ->
[Director]     Acquire missing media from sources/providers
  ->
[Gatekeeper]   Validate arrivals, fingerprint, admit or quarantine
  ->
[Metadata]     Enrich tags, release data, supporting context
  ->
Library state + audit trail + UI visibility
```

The application also exposes: playback, queue management, playlists, downloads dashboard,
Spotify history/import helpers, settings and provider health surfaces, library organization tooling.

---

## Module Status

| Area | Location | Status | What Is True Now | Main Gaps |
|---|---|---|---|---|
| Desktop shell | `src-tauri` | Active | Tauri app boots, commands wired, shortcuts registered | Packaging proof incomplete |
| UI | `ui` | Active | Library, downloads, settings, artists, playlists, tools routes exist | Long-session UX polish; artist deep-link from library tab navigates to /artists list |
| Librarian | `crates/cassette-core/src/librarian` | Implemented | Scanning, normalization, import helpers, matching paths exist | Edge-case coverage should keep improving |
| Custodian | `crates/cassette-core/src/custodian` | Implemented | Sorting, staging, quarantine, validation, custody log modules exist | Audit/event completeness proof is a P0 gate |
| Orchestrator | `crates/cassette-core/src/orchestrator` | Implemented | Reconciliation, sequencing, delta generation are present | Determinism and traceability checks ongoing |
| Director | `crates/cassette-core/src/director` | Implemented | Engine, providers, resilience, temp recovery exist | Deezer full-track path incomplete; MetadataRepairOnly stubbed; `downloader/` module overlap unresolved |
| Gatekeeper | `crates/cassette-core/src/gatekeeper` | Implemented | Validation, placement, audit, database integrations exist | Admission audit completeness is a P0 gate |
| Library manager | `crates/cassette-core/src/library` | Implemented | Locking, operations, recovery, schema, observability present | Single-machine only; no distributed coordination |
| Validation | `crates/cassette-core/src/validation` | Implemented | Full validation flow, logging verification, sandbox support exist | Needs repeatable performance and resilience baselines |
| Metadata | `crates/cassette-core/src/metadata.rs` | Partial | Metadata and enrichment code exists | Background enrichment integration is a future hardening step |
| Player | `crates/cassette-core/src/player` | Implemented | Symphonia decode + CPAL output + ring buffer; seek, pause, volume, queue advance | Long-session reliability not formally tested |

---

## Command Surface Snapshot

The Tauri command layer exposes commands across these areas:

- Library roots, scans, track/album/artist queries
- Queue management and playback controls
- Download job starts, metadata search, discography lookups, transfer inspection
- Playlist CRUD and playback
- Spotify import parsing and album queueing
- Settings/config persistence and provider status
- Organizer actions, duplicate finding, tag fixes, staging ingest

---

## Known Issues And Technical Debt

### P0 — Shipping Blockers

- [ ] Audit/event coverage must remain provable across organization and admission paths.
  Validation/logging checks should fail loudly if coverage regresses.
- [ ] Provider live-proof coverage is incomplete. Deezer full-track path is the most
  significant gap. Qobuz and slskd paths exist but are not clean-machine proven.

### P1 — Important Hardening

- [ ] Async hardening incomplete across some acquisition/orchestration flows.
  Cancellation safety, retry behavior, and temp/staging cleanup guarantees need test coverage.
- [ ] Packaging and clean-machine confidence need proof. "Builds here" ≠ "ready to ship."
- [ ] Performance telemetry not yet treated as a strict regression budget.

### P2 — Improvement

- [ ] `downloader/` module and `director/providers/` have overlapping implementations.
  The director/providers path is active; the older downloader path should be reconciled or removed.
- [ ] `MetadataRepairOnly` acquisition strategy is explicitly stubbed in `director/engine.rs`.
- [ ] Long-session desktop reliability not formally tested or documented.
- [ ] `Album.id` is a computed `ROW_NUMBER()` from SQL, not a real primary key.
  IDs are not stable across queries if data changes. Any code that caches Album IDs by value is fragile.

---

## Quality Gates

Before declaring a release candidate ready, all of the following must pass:

- [ ] `cargo check` passes at workspace root (no warnings)
- [ ] `cargo test` passes for the Rust workspace
- [ ] `ui` build passes with `npm run build`
- [ ] Desktop smoke script passes: `scripts/smoke_desktop.ps1`
- [ ] Validation flows complete against sandbox inputs without corrupting source files
- [ ] File and operation lineage is queryable for representative workflows
- [ ] Provider failures are visible, recoverable, and documented
- [ ] Docs reflect the current runtime rather than plan text

---

## Operational Principles

Cassette handles real music files and real local state. Every implementation choice should be:

- Reversible before destructive mutation
- Auditable through logs and operation records
- Local-first by default
- Defensive against file corruption, network failure, partial downloads, and stale metadata
- Documented with tradeoffs, not just outcomes

---

## Performance Baseline

Current evidence is qualitative:

- Rust workspace compiles cleanly
- UI production build succeeds
- Desktop smoke checks pass

Formal baselines live in [TELEMETRY.md](TELEMETRY.md) and should be updated when benchmarks are added.

---

## Testing Strategy

Primary verification commands:

```bash
cargo check
cargo test
cd ui && npm run build
cd .. && ./scripts/smoke_desktop.ps1   # Windows
```

Validation CLI surface:

```bash
cargo run -p cassette-core --bin cassette -- validate --help
cargo run -p cassette-core --bin cassette -- lineage --help
cargo run -p cassette-core --bin cassette -- operation --help
```

Use sandboxed validation paths before production-mode actions whenever possible.

---

## Decisions That Shape The Codebase

- Local SQLite over external service dependencies
- Shared domain crate to keep business logic outside the Tauri shell
- Defensive file handling with validation and quarantine concepts
- Explicit module boundaries for acquisition, organization, validation, and observability
- Single-machine operational assumptions

Full rationale in [DECISIONS.md](DECISIONS.md).

---

## Canonical Companion Docs

- [AGENT_CODEX.md](AGENT_CODEX.md)
- [AGENT_BRIEFING.md](AGENT_BRIEFING.md)
- [TODO.md](TODO.md)
- [DECISIONS.md](DECISIONS.md)
- [PATTERNS.md](PATTERNS.md)
- [TELEMETRY.md](TELEMETRY.md)
- [PROJECT_STATE.md](PROJECT_STATE.md)
- [RECOVERY_STATUS.md](RECOVERY_STATUS.md)

---

**This document is canonical project map material. Keep it factual, current, and tied to observed runtime truth.**
