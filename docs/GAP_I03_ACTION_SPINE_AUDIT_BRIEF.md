# GAP-I03 Action-Spine Audit Brief

Last updated: 2026-04-09
Status: ready for execution
Owner: Christian (Capn)
Gap ID: `GAP-I03`

## Purpose

Audit Cassette's interaction spine before more shell expansion lands.

This brief exists to answer one question honestly:

When the shell points at an action, does anything real happen?

The goal is not visual polish.
The goal is not route cleanup.
The goal is not to "improve the mini player."

The goal is to prove or disprove trust in the action path:

UI intent -> Tauri command or event -> Rust state change -> truthful UI reflection

## Why This Exists

The owner concern is not just "the UI feels like a website."
It is deeper:

- surfaces imply capability
- controls may point without acting
- architecture may be producing fake confidence

If that is true, no shell redesign will feel honest until the action spine is audited.

## Canonical Inputs

Read in this order before starting the audit:

1. `docs/MODULAR_DESKTOP_DIRECTION_RESET.md`
2. `docs/MODULAR_WORKSPACE_CONTRACT.md`
3. `docs/MODULAR_WORKSPACE_EXECUTION_PLAN.md`
4. `docs/PROJECT_STATE.md`
5. `docs/TODO.md`

## Audit Rule

Do not infer working behavior from:

- component existence
- route existence
- compile success
- store shape
- prior claims in conversation

Only count behavior as truthful if it is traced end to end and observed.

## Deliverable

Produce one action matrix with these columns:

| Surface | Control | Expected behavior | Actual behavior | Truth status | Path(s) |
|---|---|---|---|---|---|

Truth status vocabulary is fixed:

- `truthful`
- `drifting`
- `fake`
- `untested`

Also produce:

- a short findings list ordered by severity
- a repair order for trust-breaking issues
- any follow-up `GAP-*` tasks required if the current board is too coarse

## Scope

### 1. Shell chrome actions

Audit:

- command palette open
- compact player toggle
- minimize
- restore

Primary files:

- `ui/src/routes/+layout.svelte`
- `ui/src/lib/components/CommandPalette.svelte`
- `ui/src/lib/stores/commands.ts`
- `ui/src/lib/stores/shell.ts`
- `src-tauri/src/lib.rs`

### 2. Playback transport actions

Audit:

- play
- pause
- previous
- next
- seek
- progress update reflection
- now-playing identity reflection
- album-art reflection
- end-of-track behavior

Primary files:

- `ui/src/lib/components/NowPlaying.svelte`
- `ui/src/lib/stores/player.ts`
- `ui/src/lib/api/tauri.ts`
- `src-tauri/src/state.rs`
- `src-tauri/src/commands/player.rs`
- `crates/cassette-core/src/player/*`

### 3. Queue actions

Audit:

- load queue
- add to queue
- remove from queue
- reorder queue
- clear queue
- queue-driven playback handoff
- playlist-to-queue sync

Primary files:

- `ui/src/lib/components/QueuePanel.svelte`
- `ui/src/lib/stores/queue.ts`
- `ui/src/lib/stores/playlists.ts`
- `src-tauri/src/commands/queue.rs`
- `src-tauri/src/commands/playlists.rs`

### 4. Library-to-action flows

Audit:

- play from library
- queue from library
- click-through from library rail or listening surface into active content
- browse while playback continues

Primary files:

- `ui/src/lib/stores/library.ts`
- `ui/src/routes/library/+page.svelte`
- `ui/src/routes/collection/+page.svelte`
- `ui/src/routes/artists/+page.svelte`
- `ui/src/lib/components/ContextActionRail.svelte`
- relevant Tauri library commands

### 5. Utility-well actions

Audit:

- queue mode
- context mode
- room mode
- open/close behavior
- whether visible states actually match store/runtime truth

Primary files:

- `ui/src/lib/components/RightSidebar.svelte`
- related player/download/library stores

### 6. Workstation entry posture

Audit:

- current route or command entry into workstation-owned surfaces
- whether moving into those surfaces breaks listening context
- whether the shell currently lies about being modular when it is still route-swapping

Primary files:

- `ui/src/routes/workstation/+page.svelte`
- `ui/src/routes/downloads/+page.svelte`
- `ui/src/routes/import/+page.svelte`
- `ui/src/routes/tools/+page.svelte`
- `ui/src/routes/settings/+page.svelte`
- `ui/src/lib/components/Sidebar.svelte`

## Required Audit Method

### Step 1: Static trace

For each action:

- identify the UI control
- identify the store or handler
- identify the Tauri command/event path
- identify the Rust receiver and state mutation path
- identify the expected reflected UI state

### Step 2: Runtime proof

Exercise the action in the real desktop app where possible.

Minimum expectation:

- launch the app
- trigger the action
- observe whether behavior and state reflection match

Do not treat preview-mode-only checks as full proof.
If desktop runtime proof is blocked, label the row `untested`, not `truthful`.

### Step 3: Classification

Use these meanings exactly:

- `truthful`: action path works and reflected state matches
- `drifting`: action path partly works, but reflection or continuity is off
- `fake`: visible affordance implies action but action does not truly happen
- `untested`: code path identified but not runtime-proven

## Severity Rules

Findings should be ordered like this:

1. `fake` actions on primary listening surfaces
2. `drifting` playback and queue continuity
3. shell controls that imply modular behavior but are only route swaps
4. lower-priority cosmetic or wording mismatches

## Must-Answer Questions

The audit must answer these plainly:

1. Which visible controls on primary listening surfaces are fake?
2. Where does playback truth drift between runtime, store, and UI?
3. Where does queue truth drift between runtime, store, and UI?
4. Which current shell behaviors are honest fixed-shell behavior, and which ones are falsely implying modularity?
5. What is the minimum repair order before shell-foundation work begins?

## Must Not Happen

- do not redesign the shell during the audit
- do not bury action failures under a broad UI rewrite
- do not treat route breadth as evidence of product depth
- do not claim modular desktop behavior where only route navigation exists

## Output Format

The final audit artifact should include:

### A. Action matrix

The full matrix.

### B. Findings

A concise severity-ordered list with file references.

### C. Repair order

A flat numbered list of the first fixes required before shell-foundation implementation.

### D. Reality statement

One short paragraph stating what the current shell actually is after the audit.

## Exit Condition

`GAP-I03` is only done when:

- the action matrix exists
- primary actions are classified
- trust-breaking failures are named explicitly
- the next shell implementation agent can start from observed truth instead of assumption
