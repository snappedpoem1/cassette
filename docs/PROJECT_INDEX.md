# Cassette Project Index

**Status**: Active - identity-first convergence, hardening, audit proof, and packaging confidence
**Next**: proof hardening, audit completeness proof, and packaging confidence
**Last Updated**: 2026-04-05
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

Current operating reset:

- MusicBrainz is the canonical identity spine.
- Spotify is the intent/import seed plus fallback metadata input, not canonical truth.
- `cassette_librarian.db` is the canonical control-plane and identity/planning store.
- `cassette.db` remains the playback/runtime cache until a later deliberate convergence pass.

---

## Repository Layout

```text
Cassette/
|- Cargo.toml                  # workspace root
|- crates/
|  `- cassette-core/           # shared Rust domain logic
|- src-tauri/                  # desktop shell and Tauri commands
|- ui/                         # SvelteKit frontend
|- docs/                       # canonical project documentation
|- scripts/                    # smoke tests and maintenance helpers
`- test_data/                  # test fixtures
```

Canonical implementation layers:

- `crates/cassette-core` - scanning, organization, downloads, validation, metadata, orchestration, locking
- `src-tauri` - app boot, command surface, state wiring, desktop plugins
- `ui` - end-user interface
- `scripts` - smoke runs, sandbox reset, operational helpers

Canonical docs live under `docs/`. Research/supporting docs are useful context, but when they diverge from
`docs/*`, the canonical docs win.

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

Tool ownership spine:

- Identity and release truth: MusicBrainz
- Intent and source aliases: Spotify
- Torrent search: Jackett
- Torrent resolve/unrestrict: Real-Debrid
- Premium acquisition: Qobuz, Deezer
- Long-tail acquisition: slskd, Usenet
- Last-resort acquisition: yt-dlp

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

The application also exposes playback, queue management, playlists, a downloads dashboard,
Spotify history/import helpers, settings, provider status, and library organization tooling.

---

## Module Status

| Area | Location | Status | What Is True Now | Main Gaps |
|---|---|---|---|---|
| Desktop shell | `src-tauri` | Active | Tauri app boots, commands wired, shortcuts registered | Packaging proof incomplete |
| UI | `ui` | Active | Library, downloads, settings, artists, playlists, tools routes exist | Long-session UX polish; richer provider health and troubleshooting views |
| Librarian | `crates/cassette-core/src/librarian` | Implemented | Scanning, normalization, import helpers, matching paths exist | Edge-case coverage should keep improving |
| Custodian | `crates/cassette-core/src/custodian` | Implemented | Sorting, staging, quarantine, validation, custody log modules exist | Audit/event completeness proof is a P0 gate |
| Orchestrator | `crates/cassette-core/src/orchestrator` | Implemented | Reconciliation, sequencing, delta generation are present | Determinism and traceability checks ongoing |
| Director | `crates/cassette-core/src/director` | Implemented | Engine, providers, resilience, temp recovery, task-local cancellation, health checks, startup recovery, and planner-time edition policy filtering exist | Bounded coordinator live proof still pending |
| Acquisition control plane | `crates/cassette-core/src/acquisition.rs` + `crates/cassette-core/src/librarian/db` | Active | Sidecar-owned request contract, request timeline persistence, canonical identity planning tables, source-alias evidence, request -> task translation, planner rationale, and review mutations exist | Full proof/UX hardening and exclusion-memory lanes still remain |
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
- Spotify history parsing, desired-track import, and album queueing
- Settings/config persistence and provider status
- Organizer actions, duplicate finding, tag fixes, staging ingest

---

## Known Issues And Technical Debt

### P0 - Shipping Blockers

- [ ] Audit/event coverage must remain provable across organization and admission paths.
  Validation/logging checks should fail loudly if coverage regresses.
- [ ] Provider and recovery proof coverage is still incomplete at the UX/package level.
  Deezer full-track and startup recovery are now proven locally; audit completeness and
  packaging confidence are the bigger remaining shipping gaps.

### P1 - Important Hardening

- [ ] Async hardening is incomplete across some acquisition/orchestration flows.
  Cancellation safety, retry behavior, and temp/staging cleanup guarantees need test coverage.
- [ ] Packaging and clean-machine confidence need proof. "Builds here" is not the same as "ready to ship."
- [ ] Performance telemetry is not yet treated as a strict regression budget.
- [ ] Active runtime provenance now persists request signatures, candidate sets, provider search outcomes,
  and durable negative-result memory, but the runtime still does not reuse that memory for staged
  candidate review, query-cache TTLs, or user approval flows.

### P2 - Improvement

- [ ] `downloader/` remains in-tree as a legacy compatibility re-export for provider settings.
  Runtime acquisition ownership is `director/providers/`; no new runtime behavior should land in `downloader/`.
- [x] `MetadataRepairOnly` is implemented for runtime DB-backed local metadata repair flows.
- [ ] Long-session desktop reliability is not formally tested or documented.
- [x] Album/artist projection IDs now use deterministic hash IDs rather than seeded `DefaultHasher`
  values, so IDs remain stable across process restarts.

---

## Quality Gates

Before declaring a release candidate ready, all of the following must pass:

- [ ] `cargo check` passes at workspace root with no actionable warnings
- [ ] `cargo test` passes for the Rust workspace
- [ ] `ui` build passes with `npm run build`
- [ ] Desktop smoke script passes: `scripts/smoke_desktop.ps1 -Strict`
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

- Rust workspace compiles
- UI production build is part of the verification surface
- Desktop smoke checks are part of the verification surface

Formal baselines live in [TELEMETRY.md](TELEMETRY.md) and should be updated when benchmarks are added.

Current verification note:

- `cargo test -p cassette-core` passes
- `cargo test --workspace` passes, with the pure `src-tauri` logic tests moved to `src-tauri/tests/pure_logic.rs` so the Windows Tauri lib harness is no longer part of the gate

---

## Testing Strategy

Primary verification commands:

```powershell
cargo check --workspace
cargo test --workspace
Set-Location ui; npm install; npm run build; Set-Location ..
.\scripts\smoke_desktop.ps1
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
- [HIT_LIST.md](HIT_LIST.md)
- [TODO.md](TODO.md)
- [DECISIONS.md](DECISIONS.md)
- [PATTERNS.md](PATTERNS.md)
- [TELEMETRY.md](TELEMETRY.md)
- [PROJECT_STATE.md](PROJECT_STATE.md)
- [CLEAN_MACHINE_CHECKLIST.md](CLEAN_MACHINE_CHECKLIST.md)
- [RELEASE_CHECKLIST.md](RELEASE_CHECKLIST.md)
- [RECOVERY_STATUS.md](RECOVERY_STATUS.md)

---

**This is canonical project map material. Keep it factual, current, and tied to observed runtime truth. The canonical docs for this repo live under `docs/`.**
