# Agent Briefing

## Two-Minute Cassette Overview

**Status**: Active — hardening, provider proof, and release confidence
**Next Priority**: Deezer full-track proof, audit completeness, async recovery hardening
**Codebase Shape**: Rust workspace + Tauri 2 + SvelteKit + SQLite
**Last Updated**: 2026-03-25

---

## What Cassette Is

Cassette is a private, local-first music application with a strong operational spine.

It is simultaneously:

- a desktop app (Tauri 2 + SvelteKit)
- a library organizer with staging and quarantine
- a downloader and acquisition tool (multi-provider: Qobuz, Deezer, slskd, Usenet, yt-dlp, local archive)
- a validation and audit system with full file lineage
- a playback and queue surface

The most useful mental model is not "music player." It is "trustworthy personal library system."

---

## Current Runtime Truth

- Workspace layout is `src-tauri/` + `crates/cassette-core/` + `ui/`.
- Domain modules: `librarian`, `custodian`, `orchestrator`, `director`, `gatekeeper`, `library`, `validation`, `metadata`.
- `cargo check`, `cargo test`, `ui` production build, and desktop smoke checks all pass.
- Provider acquisition proof is incomplete for Deezer (full-track), Tidal (not started), and others.
- Packaging for a clean machine has not been proven.
- This is a git repository. History is available.

---

## Architecture

```
UI (ui/)
  ->
Tauri shell (src-tauri/)
  ->
Shared Rust domain (crates/cassette-core/)
  ->
SQLite + filesystem + external providers
```

Pipeline-oriented library flow:

```
Librarian -> Custodian -> Orchestrator -> Director -> Gatekeeper
```

Supporting runtime responsibilities:

- `library` — operations, locking, recovery, observability
- `validation` — sandbox validation and logging proof
- `metadata` — enrichment and tag-fix flows

---

## Read These First

1. [PROJECT_INDEX.md](PROJECT_INDEX.md) — project map, module status, quality gates
2. [TODO.md](TODO.md) — prioritized task list with P0/P1/P2
3. [PROJECT_STATE.md](PROJECT_STATE.md) — current runtime truth and known gaps
4. [DECISIONS.md](DECISIONS.md) — why the codebase is shaped the way it is
5. [PATTERNS.md](PATTERNS.md) — code, naming, and testing conventions

---

## Baseline Commands

```bash
cargo check
cargo test

cd ui && npm run build

# Windows:
.\scripts\smoke_desktop.ps1
```

Validation and inspection CLI:

```bash
cargo run -p cassette-core --bin cassette -- validate --help
cargo run -p cassette-core --bin cassette -- lineage --help
cargo run -p cassette-core --bin cassette -- operation --help
```

---

## Where To Focus First

- Read `TODO.md`. Pick the top `P0` item.
- Keep file and operation lineage provable — it is a core promise.
- Treat provider reliability as an operational concern, not just a feature concern.
- Prefer facts over inherited assumptions. Check the code before updating the docs.
- Protect real files through reversible flows.

---

## Success Criteria For A Change

You are done with a change when:

- the task is scoped and implemented
- relevant verification has been run (`cargo test`, smoke check if applicable)
- docs reflect the new reality (update `TODO.md`, `PROJECT_STATE.md`, `TELEMETRY.md` as needed)
- the next agent can continue without rediscovery

---

## Context: The Reconstruction

In March 2026, the original Cassette workspace was accidentally deleted and partially reconstructed.
See [RECOVERY_STATUS.md](RECOVERY_STATUS.md) for the full record. The current codebase is the
result of that reconstruction and subsequent development. It is functional; treat it as the
current truth, not as a degraded version of something else.
