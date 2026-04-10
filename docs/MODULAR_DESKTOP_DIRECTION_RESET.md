# Modular Desktop Direction Reset

Last updated: 2026-04-09
Status: active canonical direction
Owner: Christian (Capn)

## Purpose

Reset Cassette's UI and shell direction away from route-first web-app drift and toward the actual target:

- a modular desktop listening environment
- a music machine with layered surfaces
- a shell that can eventually break apart into true desktop windows where it earns that complexity

This document exists because several prior planning docs correctly improved language, listening posture, and visual quality, but still left too much room for a generic browser-shell interpretation.

## Canonical Precedence

For shell and UI architecture interpretation, use this precedence order:

1. `docs/MODULAR_DESKTOP_DIRECTION_RESET.md`
2. `docs/MODULAR_WORKSPACE_CONTRACT.md`
3. `docs/DECISIONS.md`
4. `docs/TODO.md`
5. `docs/OPEN_GAPS_EXECUTION_BOARD.md`
6. `docs/SIGNATURE_SURFACES_PLAN.md`
7. `docs/EXPERIENCE_BOUNDARY_MAP.md`
8. `docs/VISUAL_SYSTEM_DIRECTION.md`

If an older planning note implies that Cassette should keep expanding as a route-first listening web app inside one fixed shell, this document overrides that interpretation.

## Repo-Wide Audit Summary

The current repo truth is mixed.

What is already strong:

- the Rust backend is real infrastructure
- the trust spine, auditability, recovery, and acquisition logic are worth preserving
- Tauri remains a valid desktop host
- Svelte remains a valid renderer for a serious desktop shell

What is directionally wrong:

1. The active shell is still fundamentally route-led.
2. The main mental model is still "navigate to pages" more than "open and arrange listening surfaces."
3. Existing planning docs often improve the content inside routes without redefining the shell contract itself.
4. Prior UX work reduced operator noise and improved the mood, but it did not yet establish a modular desktop surface model.
5. The repo currently overstates "modular desktop" progress if that phrase is taken to mean dockable, floating, break-apart, persistent workspace behavior.

## Reality Check

Cassette today is not yet:

- a true Winamp-like modular desktop environment
- a DAW-like workspace engine
- a multi-window desktop system with detachable modules

Cassette today is:

- a Tauri desktop app
- with a route-heavy Svelte renderer
- inside a fixed shell frame
- with a right rail and bottom transport
- backed by a serious Rust system

That means the backend is worth keeping, but the shell contract needs to change.

## What Cassette Is Not

Do not interpret Cassette as any of the following:

- a SaaS-style dashboard with music features
- a sidebar-plus-pages website in a desktop wrapper
- a collection of browser cards with title bars
- a generic "media web app" with prettier gradients
- a shell where route growth is treated as architectural progress

## What Cassette Is Becoming

Cassette should behave like a modular desktop listening environment.

That means:

- persistent listening context
- surfaces instead of page churn
- layered utility areas instead of boxed dashboard panels
- edge-to-edge composition instead of card prison
- real resize behavior and layout memory
- calm but dense controls with machine-like identity
- optional true desktop windows for selected modules after the shell contract is proven

## Architecture Truth

### Keep

- Rust backend ownership
- Tauri desktop host
- Svelte UI technology
- existing playback, queue, acquisition, audit, and library command surfaces where they remain truthful

### Change

- the primary UI abstraction
- the shell contract
- layout persistence strategy
- the relationship between routes and listening flow

### Do Not Do First

- do not fork or steal an unrelated native player codebase
- do not jump straight to "everything is a separate OS window"
- do not keep expanding route count and call that modularity
- do not solve this as a visual reskin only

## Operating Model

### Core shell

Cassette needs one strong workspace shell first.

The shell should support:

- library/browser presence that does not disappear during normal use
- a central listening surface
- utility wells and overlays
- bottom transport that remains trustworthy
- pinned, collapsed, and expanded states
- persisted geometry

### Surface model

The product question should become:

- which surfaces are open
- where are they docked
- which ones are pinned
- which ones are floating
- which preset is active

The product question should stop being:

- which page am I on

### Break-apart strategy

True detached windows are still a valid goal, but only for selected modules after the workspace contract is stable.

Initial pop-out candidates:

- mini player
- queue
- lyrics
- EQ or audio tools
- visualizer
- workstation overlay or selected workstation tools

## Interaction Truth Contract

Before major shell expansion, Cassette must prove a trustworthy action spine:

UI action -> Tauri command or event -> Rust state change -> truthful UI reflection

If a visible control, surface, or status implies action, it must act.

This contract takes precedence over:

- player-bar cosmetics
- new route creation
- new shell mood work
- new visual polish passes

## Design Guardrails

Hard no:

- repeated browser-card composition
- generic panel title bars everywhere
- soft mobile-app bubbles
- oversized padded dashboard regions
- "admin app but for music" structure

Hard yes:

- continuous shell surfaces
- embedded local controls
- asymmetric composition where useful
- side wells, inset trays, overlay lids, pinned rails, and floating utility surfaces
- compact, intentional, tactile controls

## Relationship To Existing Planning Docs

These docs still matter, but their scope narrows under this reset:

- `docs/SIGNATURE_SURFACES_PLAN.md`
  Valid for listening rituals, object model priorities, and content ownership.
  Not sufficient as the shell architecture contract by itself.

- `docs/EXPERIENCE_BOUNDARY_MAP.md`
  Valid for listening versus workstation boundary and language governance.
  Not sufficient as the workspace/docking/windowing contract by itself.

- `docs/VISUAL_SYSTEM_DIRECTION.md`
  Valid for visual anti-generic guidance.
  Not sufficient as an interaction architecture or shell behavior contract.

## Execution Order

### Lane 1: Canonical docs correction

Goal:

- remove or annotate documents that still imply route-first expansion is the main path

Deliverables:

- this document
- aligned `TODO.md`
- aligned `OPEN_GAPS_EXECUTION_BOARD.md`
- aligned `DECISIONS.md`
- factual note in `PROJECT_STATE.md`

### Lane 2: Interaction spine audit

Goal:

- verify whether primary shell actions really act

Questions to answer:

- which controls are truthful
- which controls are stale, fragmented, or fake
- where store state and runtime state drift
- which surfaces imply capability they do not truly have

### Lane 3: Workspace contract spec

Goal:

- define Cassette's actual shell model before large UI changes

Must define:

- surface types
- docking zones
- floating behavior
- collapse and pin rules
- overlay rules
- preset model
- persistence schema
- future pop-out eligibility rules

### Lane 4: Shell foundation

Goal:

- convert the primary shell from route-first to workspace-first

Minimum outcome:

- persistent library/browser presence
- central content area
- utility well or wells
- honest transport surface
- real resize behavior
- layout persistence

### Lane 5: Controlled breakout

Goal:

- add true Tauri multi-window behavior only where it clearly helps

First proof candidates:

- queue window
- mini-player window

## Multi-Agent Plan

### Agent A: Canonical Auditor

Owns:

- `docs/*`

Mission:

- find and mark every canonical doc that still permits route-first drift

### Agent B: Action Spine Auditor

Owns:

- UI action surfaces
- Tauri command bindings
- playback and queue state flow

Mission:

- prove what acts, what lies, and what drifts

### Agent C: Workspace Architect

Owns:

- shell model spec
- persistence model
- docking and floating grammar

Mission:

- define the workspace contract before implementation

### Agent D: Shell Integrator

Owns:

- `ui/src/routes/+layout.svelte`
- shell stores
- resize and persistence behavior

Mission:

- land the first real workspace shell

### Agent E: Windowing Integrator

Owns:

- Tauri window management
- selected module breakout

Mission:

- add true windows after the shell contract exists

## Questions That Are Allowed

This direction is allowed to stop and ask hard questions when the answer changes the shell contract.

Valid questions include:

- which modules must be detachable first
- whether queue belongs in the main frame, a utility well, a pop-out, or all three
- whether the workstation is an overlay lid, a side rail, a detached window, or a mix
- what "skins" must control beyond color
- which surfaces are always resident versus opened on demand

These questions are not drift. They are part of the architecture work.

## Immediate Next Steps

1. Finish the canonical document correction pass.
2. Run an interaction-spine audit instead of a mini-player-only pass.
3. Use `docs/MODULAR_WORKSPACE_CONTRACT.md` as the workspace contract spec before broader shell edits.
4. Only then begin UI implementation work.

## Completion Standard

This direction reset is only real when:

- the docs stop telling the old story
- the next agent cannot plausibly interpret Cassette as a route-first web shell project
- the shell contract is explicit enough to implement without guessing
- implementation work is sequenced around interaction truth first, shell behavior second, visual expression third
