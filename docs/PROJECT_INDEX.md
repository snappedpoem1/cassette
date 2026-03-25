# Cassette Project Index

**Status**: Active hardening and documentation pass  
**Next**: Close audit gaps, validate provider reliability, keep desktop/runtime docs current  
**Last Updated**: 2026-03-24  
**Owner**: Christian (Capn)  
**Agent Responsibility**: Treat this file as the project map and update it when architecture, workflows, or confidence levels materially change.

---

## Project Overview

Cassette is a private, local-first music application built for managing, organizing, auditing, and acquiring a personal audio library. It combines:

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

## Runtime Truth

The current workspace is a Rust workspace with two primary crates:

```text
C:\Cassette Music
|-- Cargo.toml                  # workspace root
|-- crates\
|   `-- cassette-core\          # shared Rust domain logic
|-- src-tauri\                  # desktop shell and Tauri commands
|-- ui\                         # SvelteKit frontend
|-- docs\                       # project documentation
|-- scripts\                    # smoke tests and maintenance helpers
`-- cassette.db                 # active local database in this workspace
```

Canonical implementation layers:

- `crates/cassette-core`: scanning, organization, downloads, validation, metadata, orchestration, locking
- `src-tauri`: app boot, command surface, state wiring, desktop plugins
- `ui`: end-user interface
- `scripts`: smoke runs, sandbox reset, operational helpers

Important current fact:

- This folder does not currently contain local `.git` metadata, so local branch and history operations are unavailable from this workspace snapshot.

---

## Architecture At A Glance

Cassette has two overlapping views of architecture that both matter:

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
[Librarian]   Scan, normalize, classify local files
  ->
[Custodian]   Organize, validate, quarantine, stage safely
  ->
[Orchestrator] Reconcile desired vs local and plan work
  ->
[Director]    Acquire missing media from sources/providers
  ->
[Gatekeeper]  Validate arrivals, fingerprint, admit or quarantine
  ->
[Metadata]    Enrich tags, release data, supporting context
  ->
Library state + audit trail + UI visibility
```

This workflow is implemented inside a broader application that also exposes:

- Playback
- Queue management
- Playlists
- Downloads dashboard
- Spotify history/import helpers
- Settings and provider health surfaces
- Library organization tooling

---

## Modules And Status

| Area | Location | Status | What Is True Now | Main Gaps |
|---|---|---|---|---|
| Desktop shell | `src-tauri` | Active | Tauri app boots, commands wired, shortcuts registered | Packaging confidence still needs continued proof |
| UI | `ui` | Active | Library, downloads, settings, and advanced routes exist | Long-session UX polish still evolving |
| Librarian | `crates/cassette-core/src/librarian` | Implemented | Scanning, normalization, import helpers, matching paths exist | Confidence thresholds and edge-case coverage should keep improving |
| Custodian | `crates/cassette-core/src/custodian` | Implemented | Sorting, staging, quarantine, validation, custody log modules exist | Audit/event completeness should remain a focus |
| Orchestrator | `crates/cassette-core/src/orchestrator` | Implemented | Reconciliation, sequencing, delta generation are present | Needs ongoing determinism and traceability checks |
| Director | `crates/cassette-core/src/director` | Implemented | Engine, providers, sources, resilience, temp recovery modules exist | Provider reliability and circuit-break style behavior need stronger proof |
| Gatekeeper | `crates/cassette-core/src/gatekeeper` | Implemented | Validation, placement, audit, database integrations exist | Admission audit completeness and policy tuning should be monitored |
| Library manager | `crates/cassette-core/src/library` | Implemented | Locking, operations, recovery, schema, observability are present | Single-machine only; no distributed coordination |
| Validation | `crates/cassette-core/src/validation` | Implemented | Full validation flow, logging verification, sandbox support exist | Needs repeatable performance and resilience baselines |
| Metadata | `crates/cassette-core/src/metadata.rs` and librarian enrichers | Partial | Metadata and enrichment code exists | Full background integration is still a future hardening step |

---

## Command Surface Snapshot

The Tauri command layer currently exposes commands across these areas:

- Library roots, scans, track search, albums, artists
- Queue management
- Playback controls and now-playing state
- Download job starts, metadata search, discography lookups, transfer inspection
- Playlist CRUD and playback
- Spotify import parsing and album queueing
- Settings/config persistence and provider status
- Organizer actions, duplicate finding, tag fixes, staging ingest

This means Cassette is already beyond a narrow pipeline prototype. The canonical docs need to support both:

- The pipeline-oriented audio workflow
- The broader desktop product/runtime

---

## Known Issues And Technical Debt

### Critical

- [ ] Audit/event coverage still needs to remain provable across organization and admission paths.
  - Why it matters: lineage is one of the project's core promises.
  - Evidence source: validation/logging stack exists, but this should remain a shipping gate rather than an assumption.

### High Priority

- [ ] Provider live-proof coverage is incomplete on this machine.
  - Impact: runtime confidence for external acquisition paths is not yet complete.
  - Notes: Deezer full-track path is still called out as incomplete in current repo docs.

- [ ] Async hardening is incomplete across some acquisition/orchestration flows.
  - Impact: operational consistency and throughput are harder to reason about under load or flaky networks.

- [ ] Packaging and clean-machine confidence need stronger proof.
  - Impact: "builds here" is not the same as "ready for release".

### Medium Priority

- [ ] Performance telemetry is not yet treated as a strict regression budget.
- [ ] Documentation for public APIs and operating patterns can still be tightened further.
- [ ] Long-session reliability and recovery drills should be formalized.

---

## Quality Gates

Before declaring a release candidate ready, verify all of the following:

- [ ] `cargo check` passes at workspace root
- [ ] `cargo test` passes for the Rust workspace
- [ ] `ui` build passes with `npm run build`
- [ ] Desktop smoke script passes: `scripts/smoke_desktop.ps1`
- [ ] Validation flows complete against sandbox inputs without corrupting source files
- [ ] File and operation lineage is queryable for representative workflows
- [ ] Provider failures are visible, recoverable, and documented
- [ ] Docs reflect the current runtime rather than historical plan text

---

## Operational Principles

Cassette is handling real music files and real local state. Every implementation choice should be:

- Reversible before destructive mutation
- Auditable through logs and operation records
- Local-first by default
- Defensive against file corruption, network failure, partial downloads, and stale metadata
- Documented with tradeoffs, not just outcomes

---

## Performance Baseline

Current evidence in repo docs is qualitative rather than benchmark-driven:

- Rust workspace compiles cleanly
- UI production build succeeds
- Desktop smoke checks pass via `scripts/smoke_desktop.ps1`

Formal baseline ownership lives in [TELEMETRY.md](/c:/Cassette%20Music/docs/TELEMETRY.md), and that file should be updated whenever we add benchmarks or observe regressions.

---

## Testing Strategy

Primary verification commands in this workspace:

```powershell
cargo check
cargo test

Set-Location ui
npm run build

Set-Location ..
.\scripts\smoke_desktop.ps1
```

Validation CLI surface:

```powershell
cargo run -p cassette-core --bin cassette -- validate --help
cargo run -p cassette-core --bin cassette -- lineage --help
cargo run -p cassette-core --bin cassette -- operation --help
```

Use sandboxed validation paths before production-mode actions whenever possible.

---

## Decisions That Shape The Codebase

The current codebase clearly reflects these decisions:

- Local SQLite over external service dependencies
- Shared domain crate to keep business logic outside the Tauri shell
- Defensive file handling with validation and quarantine concepts
- Explicit module boundaries for acquisition, organization, validation, and observability
- Single-machine operational assumptions

Full rationale lives in [DECISIONS.md](/c:/Cassette%20Music/docs/DECISIONS.md).

---

## Agent Handoff Checklist

New agents should:

1. Read [AGENT_BRIEFING.md](/c:/Cassette%20Music/docs/AGENT_BRIEFING.md)
2. Read [AGENT_CODEX.md](/c:/Cassette%20Music/docs/AGENT_CODEX.md)
3. Read [TODO.md](/c:/Cassette%20Music/docs/TODO.md)
4. Review [DECISIONS.md](/c:/Cassette%20Music/docs/DECISIONS.md)
5. Review [PATTERNS.md](/c:/Cassette%20Music/docs/PATTERNS.md)
6. Run the baseline build/test commands
7. Confirm whether they are operating in a workspace snapshot without `.git`

If onboarding takes longer than about 30 minutes, the documentation should be improved again.

---

## Near-Term Roadmap

### Hardening Track

- Close any remaining auditability gaps
- Strengthen provider reliability and recovery behavior
- Formalize telemetry baselines and regression thresholds
- Improve packaging/release confidence

### Documentation Track

- Keep agent docs aligned with runtime truth
- Add or refine API documentation where public surfaces are ambiguous
- Record architecture changes as decisions, not tribal knowledge

### Product Track

- Continue improving library management and acquisition UX
- Keep advanced tools discoverable without bloating the primary flow

---

## Canonical Companion Docs

- [AGENT_CODEX.md](/c:/Cassette%20Music/docs/AGENT_CODEX.md)
- [AGENT_BRIEFING.md](/c:/Cassette%20Music/docs/AGENT_BRIEFING.md)
- [TODO.md](/c:/Cassette%20Music/docs/TODO.md)
- [DECISIONS.md](/c:/Cassette%20Music/docs/DECISIONS.md)
- [PATTERNS.md](/c:/Cassette%20Music/docs/PATTERNS.md)
- [TELEMETRY.md](/c:/Cassette%20Music/docs/TELEMETRY.md)
- [PROJECT_STATE.md](/c:/Cassette%20Music/docs/PROJECT_STATE.md)

---

**This document is canonical project map material. Keep it factual, current, and tied to observed runtime truth.**
