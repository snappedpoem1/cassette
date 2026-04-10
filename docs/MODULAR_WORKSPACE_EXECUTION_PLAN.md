# Modular Workspace Execution Plan

Last updated: 2026-04-09
Status: active implementation plan
Owner: Christian (Capn)

## Purpose

Turn the direction reset and workspace contract into a concrete execution order.

This plan is built from:

- the owner conversation
- `docs/MODULAR_DESKTOP_DIRECTION_RESET.md`
- `docs/MODULAR_WORKSPACE_CONTRACT.md`

This plan is not a generic UX roadmap.
It is the shortest honest path from the current route-led shell toward the intended modular desktop listening environment.

## Locked Inputs

These are already decided:

- first detached module: `visualizer`
- first Workstation shape: `sliding side deck`
- floating modules: `deferred until resize + persistence are proven`
- first presets: `Listen / Queue` and `Acquisition`
- library rail direction: `Explorer-like browser/filter rail with preview/detail behavior`
- queue default in `Listen / Queue`: reopen when queue content exists or on explicit toggle
- Workstation deck edge: provisional `left edge`
- first preview/detail priority: classic track metadata with album art

## Current Reality

The active shell is still:

- top bar
- sidebar
- routed main slot
- right rail
- bottom transport

The repo already has:

- real Rust backend depth
- playback and queue infrastructure
- a Tauri host
- listening-surface and visual work that can be reused

The repo does not yet have:

- workspace-first shell behavior
- true pane resizing across the main shell
- durable layout memory for a modular workspace
- a Workstation deck
- detached module windows

## Primary Goal Order

1. Prove the action spine.
2. Build the main workspace shell.
3. Add the sliding Workstation deck.
4. Add presets.
5. Add the first detached visualizer window.

## Phase 1: Action-Spine Audit

### Goal

Prove what in the shell actually acts, what drifts, and what lies.

### Why first

If visible controls point but do not act, shell work on top of that will feel fake no matter how good it looks.

### Audit scope

#### Top-level shell actions

- command palette open
- compact player toggle
- minimize and restore behavior
- route-to-surface transition points that still exist

#### Playback actions

- play
- pause
- previous
- next
- seek
- queue-started playback
- playlist-started playback
- end-of-track advance or stop

#### Queue actions

- load queue
- reorder
- remove
- clear
- scene save and restore where applicable

#### Library-to-action flows

- click album/artist/track
- play from library
- queue from library
- browse while playback continues

#### Utility-well actions

- queue mode
- context mode
- room mode
- collapse and reopen behavior

#### Workstation entry actions

- open
- close
- maintain listening context underneath

### Required deliverable

Produce an action matrix with these columns:

| Surface | Control | Expected behavior | Actual behavior | Truth status | File path(s) |
|---|---|---|---|---|---|

Truth status vocabulary:

- `truthful`
- `drifting`
- `fake`
- `untested`

### Exit condition

No shell-foundation work should begin until:

- the matrix exists
- the most trust-breaking fake or drifting controls are identified
- follow-up implementation tasks are filed

## Phase 2: Shell Foundation

### Goal

Replace the route-first feel with a workspace-first shell without introducing true detached windows yet.

### Build target

#### Left: library rail

Must include:

- browser/filter behavior
- persistent presence
- resize handle
- compact and expanded states

Should feel closer to:

- old Explorer browse/filter behavior

Should not feel like:

- a static navigation sidebar

#### Center: active listening surface

Must include:

- current primary listening object
- edge-to-edge composition
- preserved context while browsing

#### Right: utility well

Must include:

- queue mode
- context mode
- room mode
- resize
- collapse and reopen
- remembered active mode

#### Bottom: transport band

Must include:

- direct action
- truthful playback state
- compact quality and identity cues

### Persistence required in this phase

- left width
- right width
- right collapsed/open state
- right active mode
- compact player preference
- active preset id

### Must not happen in this phase

- no detached windows yet
- no in-shell floating surfaces yet
- no broad new route expansion sold as shell progress

## Phase 3: Sliding Workstation Deck

### Goal

Create the arr-like operational layer without turning it into another page.

### Shape

Use:

- sliding side deck
- current owner lean: `left edge`

Tone:

- professional
- clean
- search-forward
- progress-forward
- review-capable

Should evoke:

- qBittorrent
- the arr apps

Should not evoke:

- bubbly dashboard cards
- playful overlay UI
- "downloads page but prettier"

### Minimum contents

- search
- in-progress work
- queue/progress
- blocked/review-needed state
- clear operational summaries

## Phase 4: Presets

### Goal

Make the shell rearrange itself intentionally instead of staying one static composition.

### First presets

#### `Listen / Queue`

Default purpose:

- listening mode with queue-adjacent control

Minimum payload:

- library rail visible
- utility well available for queue and reopened when queue content exists or when explicitly toggled
- workstation deck closed

#### `Acquisition`

Default purpose:

- search/progress/review mode

Minimum payload:

- workstation deck open
- queue/context de-emphasized
- active operational search and progress state visible

## Phase 5: First Detached Window

### Goal

Prove the breakout pattern with the least ambiguous module.

### Locked first module

- `visualizer`

### Why visualizer first

- it is naturally independent
- it does not need to own playback truth
- it can prove position/restore/focus behavior without turning the core shell into window chaos

### Required behaviors

- open and close cleanly
- restore size and position
- recover from off-screen placement
- stay synced with playback state from the Rust side

## Suggested Agent Packet

### Agent 1: Docs + audit coordinator

Owns:

- action matrix setup
- doc references
- follow-up task filing

### Agent 2: Playback and queue truth auditor

Owns:

- playback actions
- queue actions
- store/runtime sync

### Agent 3: Shell foundation implementer

Owns:

- resizable shell
- layout persistence
- utility well behavior

### Agent 4: Workstation deck implementer

Owns:

- sliding deck
- acquisition-mode shell posture

### Agent 5: Visualizer breakout implementer

Owns:

- first detached window
- restore/focus/off-screen recovery behavior

## Immediate Next Steps

1. Run the action-spine audit and produce the truth matrix.
2. File the concrete shell-foundation fixes that audit reveals.
3. Implement resize and persistence before any floating or detached module work.
4. Build the sliding Workstation deck.
5. Add the `Listen / Queue` and `Acquisition` presets.
6. Only then build the detached visualizer window.

## Open Owner Questions

There are no blocking owner questions left for the action-spine audit or first shell-foundation pass.

One non-blocking validation remains:

1. Confirm whether the Workstation deck should stay on the provisional `left edge` after feel testing in implementation.
