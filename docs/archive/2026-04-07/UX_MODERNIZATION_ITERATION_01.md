# UX Modernization Iteration 01

Last updated: 2026-04-05

## Scope

Sprint 1 execution plan for modular desktop UX modernization:

- Winamp-inspired + concept-3 visual direction
- No Lyra assistant surfaces in active desktop path
- Modular shell boundaries
- Windows command registry and keyboard-first execution
- Minimized player presence
- Taskbar integration

## Context Anchors

- Backlog source of truth: `docs/TODO.md`
- Short execution board: `docs/HIT_LIST.md`
- Runtime/state wiring: `src-tauri/src/lib.rs`, `src-tauri/src/state.rs`
- Frontend shell and player: `ui/src/routes/+layout.svelte`, `ui/src/lib/components/NowPlaying.svelte`

## Multi-Agent Iteration Outputs

### 1. Context Architecture

Primary files for first implementation slice:

- `ui/src/routes/+layout.svelte`
- `ui/src/app.css`
- `ui/src/lib/components/NowPlaying.svelte`
- `ui/src/lib/stores/player.ts`
- `ui/src/lib/api/tauri.ts`
- `src-tauri/src/lib.rs`
- `src-tauri/src/commands/mod.rs`
- `src-tauri/src/commands/player.rs`
- `src-tauri/src/state.rs`

Boundary rules adopted:

1. Route files must consume stores/services only; no desktop plugin lifecycle logic in route pages.
2. All desktop invokes remain centralized behind `ui/src/lib/api/tauri.ts`.
3. Shortcut/taskbar/window behavior remains backend-owned in `src-tauri/src/lib.rs` and command modules.
4. Shell lifecycle setup/teardown stays in `ui/src/routes/+layout.svelte`.
5. Minimized-player state belongs to shell/player stores, never per-route local state.

### 2. UX Spec (Sprint 1)

Required shell regions:

- Left nav rail
- Center workspace
- Right utility rail
- Bottom transport bar
- Command palette overlay

Must-have UX outcomes:

1. Command palette with grouped actions (Navigation, Playback, Queue, Library Ops, Window).
2. Compact/minimized player mode with persistent state and one-step restore.
3. Discoverable navigation to all core routes.
4. Direct operational wording only (no Lyra framing).

### 3. Engineering Breakdown (Sprint 1)

PR Slice 1: Contracts and shell foundation

- Add typed command DTOs and shell mode contracts.
- Keep behavior additive and backward-compatible.

PR Slice 2: Command registry

- Central registry + keyboard dispatcher.
- Sidebar/keyboard/palette execute identical command IDs.

PR Slice 3: Minimized player

- Introduce mini mode, dock/restore behavior, compact transport UI.

PR Slice 4: Taskbar integration

- Sync playback state to taskbar controls/progress.

PR Slice 5: Hardening

- Add command-routing, window-mode, and taskbar-projection tests.

### 4. CI/Gates (Iteration 1)

PR-blocking first:

1. Command registry contract tests
2. Minimized-player persistence logic tests
3. UI check/build gate
4. Accessibility static checks

Nightly first, then promote:

1. Windows taskbar integration smoke
2. Minimize/restore end-to-end smoke
3. Browser a11y audit for key routes

## Concrete Sprint 1 Board

### Ticket A - Shell Foundation

- Files: `ui/src/routes/+layout.svelte`, `ui/src/app.css`
- Done when: modular shell regions exist and persist across Library/Downloads/Settings.

### Ticket B - Command Registry

- Files: `src-tauri/src/lib.rs`, `src-tauri/src/commands/mod.rs`, `src-tauri/src/commands/player.rs`, `ui/src/lib/api/tauri.ts`
- Done when: command IDs are centralized and callable by keyboard + palette + UI entrypoints.

### Ticket C - Minimized Player

- Files: `ui/src/lib/components/NowPlaying.svelte`, `ui/src/lib/stores/player.ts`, `src-tauri/tauri.conf.json`, `src-tauri/capabilities/default.json`
- Done when: compact player mode persists and restores deterministically.

### Ticket D - Taskbar Integration

- Files: `src-tauri/src/lib.rs`, `src-tauri/src/state.rs`, `src-tauri/src/commands/player.rs`
- Done when: taskbar controls and progress reflect playback state correctly.

### Ticket E - Validation

- Files: `src-tauri/tests/pure_logic.rs`, CI workflow files to be added under `.github/workflows/`
- Done when: iteration-1 gates are automated and enforced.

## Verification Commands

```powershell
cargo check --workspace
cargo test --workspace
Set-Location ui; npm install; npm run check; npm run build; Set-Location ..
.\scripts\smoke_desktop.ps1 -Strict
```

Targeted test buckets to add in Sprint 1:

- command_registry
- window_mode
- taskbar_projection

## Risks and Mitigations

1. Risk: route-level coupling leaks desktop behavior into pages.
   Mitigation: enforce boundary rule and review imports for direct invoke/plugin usage.
2. Risk: minimized mode causes state desync with playback.
   Mitigation: one normalized playback projection source and deterministic restore tests.
3. Risk: taskbar features are flaky in PR CI.
   Mitigation: start as nightly informational, promote after stable run window.

## Iteration 1 Exit Criteria

1. Backlog item remains in `in_progress` with completed acceptance sub-boxes tracked.
2. Modular shell + command registry + mini-player + taskbar first slice is landed.
3. Keyboard-first and focus-visible checks pass for redesigned shell.
4. No Lyra strings in shell/player/command-palette surfaces for sprint-1 scope.
