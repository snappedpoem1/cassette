# Agent Briefing

## Two-Minute Cassette Overview

**Status**: Active - hardening, audit proof, coordinator proof, and release confidence
**Next Priority**: end-to-end coordinator proof, organizer safe-subset proof, packaging confidence
**Codebase Shape**: Rust workspace + Tauri 2 + SvelteKit + SQLite
**Last Updated**: 2026-03-30

Scope note:

- Cassette is a single-owner personal project.
- Treat all readiness language as personal reliability confidence, not productization or commercial launch planning.

---

## What Cassette Is

Cassette is a private, local-first music application with a strong operational spine.

It is simultaneously:

- a desktop app (Tauri 2 + SvelteKit)
- a library organizer with staging and quarantine
- an acquisition system with Director-owned providers (Qobuz, Deezer, slskd, Usenet, yt-dlp, local archive, Real-Debrid)
- a validation and audit system with file lineage goals
- a playback and queue surface

The most useful mental model is not "music player." It is "trustworthy personal library system."

---

## Current Runtime Truth

- Canonical project docs live under `docs/`, not the repo root.
- Workspace layout is `src-tauri/` + `crates/cassette-core/` + `ui/`.
- Domain modules include `librarian`, `custodian`, `orchestrator`, `director`, `gatekeeper`, `library`, `validation`, and `metadata`.
- `cargo check --workspace` passes.
- `cargo test --workspace` passes.
- `npm run build` in `ui/` passes, with an existing accessibility warning in `src/routes/downloads/+page.svelte`.
- `.\scripts\smoke_desktop.ps1` passes.
- Deezer full-track acquisition is live-proven on this machine.
- Packaging for a clean machine has not been proven.
- This is a git repository. History is available.

---

## Architecture

```text
UI (ui/)
  ->
Tauri shell (src-tauri/)
  ->
Shared Rust domain (crates/cassette-core/)
  ->
SQLite + filesystem + external providers
```

Pipeline-oriented library flow:

```text
Librarian -> Custodian -> Orchestrator -> Director -> Gatekeeper
```

Supporting runtime responsibilities:

- `library` - operations, locking, recovery, observability
- `validation` - sandbox validation and logging proof
- `metadata` - enrichment and tag-fix flows

---

## Read These First

1. [PROJECT_INDEX.md](PROJECT_INDEX.md) - project map, module status, quality gates
2. [PROJECT_STATE.md](PROJECT_STATE.md) - current runtime truth and known gaps
3. [TODO.md](TODO.md) - prioritized task list with P0/P1/P2
4. [DECISIONS.md](DECISIONS.md) - why the codebase is shaped the way it is
5. [PATTERNS.md](PATTERNS.md) - code, naming, and testing conventions

---

## Baseline Commands

```powershell
cargo check --workspace
cargo test --workspace

Set-Location ui; npm install; npm run build; Set-Location ..

.\scripts\smoke_desktop.ps1
```

These are the baseline verification commands. Re-run them when the task depends on current pass/fail state instead of inheriting stale claims.

Validation and inspection CLI:

```powershell
cargo run -p cassette-core --bin cassette -- validate --help
cargo run -p cassette-core --bin cassette -- lineage --help
cargo run -p cassette-core --bin cassette -- operation --help
```

---

## Where To Focus First

- Read `TODO.md`. Pick the top `P0` item unless a higher-priority request overrides it.
- Keep file and operation lineage provable - it is a core promise.
- Treat provider reliability as an operational concern, not just a feature concern.
- Prefer facts over inherited assumptions. Check the code before updating docs.
- Protect real files through reversible flows.

---

## Success Criteria For A Change

You are done with a change when:

- the task is scoped and implemented
- relevant verification has been run
- docs reflect the new reality (`TODO.md`, `PROJECT_STATE.md`, `DECISIONS.md`, `TELEMETRY.md` as needed)
- the next agent can continue without rediscovery

---

## Context: The Reconstruction

In March 2026, the original Cassette workspace was accidentally deleted and partially reconstructed.
See [RECOVERY_STATUS.md](RECOVERY_STATUS.md) for the full record. The current codebase is the
result of that reconstruction and subsequent development. It is functional; treat it as the
current truth, not as a degraded version of something else.
