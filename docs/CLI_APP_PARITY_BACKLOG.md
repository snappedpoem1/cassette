# CLI vs App Capability Parity Backlog

Last updated: 2026-04-09
Owner: Christian (Capn)

Scope note:

- This parity map is for a single-owner personal system.
- "Parity" means day-to-day personal workflow coverage, not product-level feature completeness.

---

## Purpose

Make CLI and desktop-app capability boundaries explicit so UX and runtime work can be prioritized with evidence.

---

## Status Definitions

- `app-parity`: available in desktop app flow.
- `partial`: exposed in app but with reduced control or evidence surface versus CLI.
- `cli-only`: intentionally operator or diagnostics lane.
- `candidate`: good app candidate after trust/UX guardrails are added.

---

## Current Parity Map

| Capability Lane | Current Surface | Status | Notes |
|---|---|---|---|
| Library scan/add roots | App + CLI | app-parity | Settings and scan flow active in app. |
| Queue playback/session handoff | App + CLI | app-parity | Queue and session flows present in app runtime. |
| Playlist/Crate authoring | App + CLI | app-parity | Daily authorship lane is app-first. |
| Download config/provider credentials | App + CLI | app-parity | App settings can edit and persist effective config. |
| Policy profile select/apply | App + CLI | partial | App supports select/apply, but CLI still exposes lower-level operator framing. |
| Provider status proof/probing | CLI primary, partial app view | partial | App shows status; repeatable bounded probes remain CLI-led. |
| Audit lineage/operation validation | CLI | cli-only | `cassette-cli lineage` / `cassette-cli operation` remain operator diagnostics lanes. |
| DB convergence and repair tooling | CLI | cli-only | `db_converge_cli`, rescue/repair paths are operator-only by design. |
| Runtime probe tooling | CLI | cli-only | `provider_probe_cli`, `slskd_runtime_probe_cli`, and strict smoke remain proof lanes. |
| Engine pipeline bounded replay | CLI | cli-only | `engine_pipeline_cli` remains controlled operator lane. |

---

## App-Candidate Backlog

| Candidate ID | Lane | Why move toward app | Guardrail needed first |
|---|---|---|---|
| PARITY-01 | Provider proof snapshots | Faster daily trust checks without leaving app | Keep probe scope bounded and artifact-linked |
| PARITY-02 | Read-only lineage lookup panel | Easier explanation path for "what happened" questions | No mutation commands; operator-only deep links |
| PARITY-03 | Runtime probe digest widget | Improves startup confidence and troubleshooting speed | Preserve strict CLI probes as source-of-truth evidence |

---

## Explicit Non-Goals

- Promote destructive or repair-oriented DB/operator commands to one-click app actions.
- Replace strict CLI smoke/probe workflows as canonical proof artifacts.
- Remove operator lanes that exist for bounded recovery and audit integrity.

---

## Verification Inputs Used

- `src-tauri/src/lib.rs` command registration surface
- `ui/src/routes/*` listening and workstation surfaces
- CLI command surfaces in `src-tauri/src/bin/*.rs`
- Strict smoke and bounded runtime checks from `scripts/smoke_desktop.ps1 -Strict`
