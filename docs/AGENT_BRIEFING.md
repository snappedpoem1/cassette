# Agent Briefing
## Two-Minute Cassette Overview

**Status**: Active hardening and documentation normalization  
**Next Priority**: audit completeness, provider proof, and release confidence  
**Codebase Shape**: Rust workspace + Tauri + SvelteKit + SQLite  
**Last Updated**: 2026-03-24

---

## What Cassette Is

Cassette is a private, local-first music application with a strong operational spine.

It is simultaneously:

- a desktop app
- a library organizer
- a downloader and acquisition tool
- a validation and audit system
- a playback and queue surface

The most useful mental model is not "music player." It is "trustworthy personal library system."

---

## Current Runtime Truth

- Workspace layout is `src-tauri` + `crates/cassette-core` + `ui`.
- Domain modules include `librarian`, `custodian`, `orchestrator`, `director`, `gatekeeper`, `library`, `validation`, and `metadata`.
- Existing repo docs state that `cargo check`, `ui` production build, and desktop smoke checks have passed in this workspace.
- Provider proof and packaging confidence still need more work.
- This workspace snapshot does not currently include local `.git` metadata.

---

## Architecture

```text
UI (ui)
  ->
Tauri shell (src-tauri)
  ->
Shared Rust domain (crates/cassette-core)
  ->
SQLite + filesystem + external providers
```

Pipeline-oriented library flow:

```text
Librarian -> Custodian -> Orchestrator -> Director -> Gatekeeper
```

Supporting runtime responsibilities:

- `library` owns operations, locking, recovery, and observability
- `validation` owns sandbox validation and logging proof
- `metadata` supports enrichment and tag-fix flows

---

## Read These First

1. [PROJECT_INDEX.md](/c:/Cassette%20Music/docs/PROJECT_INDEX.md)
2. [TODO.md](/c:/Cassette%20Music/docs/TODO.md)
3. [AGENT_CODEX.md](/c:/Cassette%20Music/docs/AGENT_CODEX.md)
4. [DECISIONS.md](/c:/Cassette%20Music/docs/DECISIONS.md)
5. [PATTERNS.md](/c:/Cassette%20Music/docs/PATTERNS.md)

---

## Baseline Commands

```powershell
cargo check
cargo test

Set-Location ui
npm run build

Set-Location ..
.\scripts\smoke_desktop.ps1
```

Validation and inspection:

```powershell
cargo run -p cassette-core --bin cassette -- validate --help
cargo run -p cassette-core --bin cassette -- lineage --help
cargo run -p cassette-core --bin cassette -- operation --help
```

---

## Where To Focus First

- Keep file and operation lineage provable.
- Treat provider reliability as an operational concern, not just a feature concern.
- Prefer facts over inherited assumptions.
- Protect real files through reversible flows.

If you need a first task, pick the top `P0` item in [TODO.md](/c:/Cassette%20Music/docs/TODO.md).

---

## Success Criteria

You are done with a change when:

- the task is scoped and implemented
- relevant verification has been run
- docs reflect the new reality
- the next agent can continue without rediscovery
