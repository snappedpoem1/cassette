# Modular Workspace Contract

Last updated: 2026-04-09
Status: approved for implementation
Owner: Christian (Capn)

## Purpose

Translate the direction reset into a buildable shell contract.

This document is the implementation brief for Cassette's next UI architecture step.
It uses the user conversation that triggered the reset as the design authority.

This contract is intentionally not a generic "desktop shell" spec.
It is a spec for the thing actually described:

- modern Winamp spirit
- modular desktop listening environment
- layered and break-apart surfaces
- real library presence
- machine-like composition instead of browser-card composition
- optional arr-style operational overlay

## Canonical Relationship

Read in this order:

1. `docs/MODULAR_DESKTOP_DIRECTION_RESET.md`
2. `docs/MODULAR_WORKSPACE_CONTRACT.md`
3. `docs/OBJECT_MODEL_DECISIONS.md`
4. `docs/EXPERIENCE_BOUNDARY_MAP.md`
5. `docs/VISUAL_SYSTEM_DIRECTION.md`

If a later implementation note conflicts with this contract, this document wins unless the owner explicitly changes direction.

## Blueprint Taken From The Conversation

The conversation defines the target more clearly than the old planning docs.

Cassette should feel like:

- a music environment, not a route collection
- a machine with surfaces, not a dashboard with cards
- something you inhabit, not something you navigate
- a shell with layers, wells, drawers, overlays, and optional detached pieces

Cassette should not feel like:

- a regular website with cool function
- a bubble-heavy media web app
- a sidebar plus pages plus footer player
- a generic panel app with title bars on everything

## Owner Setup Locked On 2026-04-09

These choices came directly from the owner and should not be re-guessed by later agents unless the owner changes them explicitly.

| Question | Locked answer |
|---|---|
| First true detached window | `visualizer` |
| First Workstation form | `sliding side deck` |
| Floating modules in first shell pass | `not yet`; keep them individual in concept but prove placement, resize, and persistence first |
| First two presets | `Listen / Queue` and `Acquisition` |
| Library rail leaning | `Explorer-like browser/filter rail` with preview/detail behavior rather than a pure nav list |
| `Listen / Queue` queue default | reopen when queue content exists or by explicit user toggle; not forced open at all times |
| Workstation deck edge | provisional `left edge`, subject to later feel validation |
| First preview/detail priority | classic track metadata first: artist, album, track, with album art |

## Core Product Sentence

Cassette should behave like a modular desktop listening environment with persistent surfaces for playback, browsing, curation, and acquisition, using a strong main shell first and selective true desktop windows second.

## Non-Negotiable Rules

1. Surface state beats route state.
2. If a visible control implies action, it must act truthfully.
3. The library remains present during normal listening work.
4. Workstation detail must not dominate the listening shell.
5. Real detached windows are selective, not the default answer to every module.
6. No generic browser-card composition as the fallback layout language.
7. Owner questions that materially change the shell stay explicit until answered.

## Shell Model

### Primary shell

Cassette's first stable target is one strong main window.

That window contains:

- a persistent library rail
- a central active surface
- one or more utility wells
- a bottom transport band
- optional overlay lids

The main shell is the first-class home of layout memory.

### Route role after this contract

Routes may continue to exist, but only as:

- deep links
- focused views
- compatibility surfaces during transition

Routes are no longer the primary product architecture.

The shell should be usable without route churn as the dominant interaction pattern.

## Surface Taxonomy

### Resident surfaces

These are always present in some form.

| Surface | Role | Default residency |
|---|---|---|
| Library rail | browse, filter, jump, queue, inspect | always visible |
| Main listening surface | current album, artist, crate, playlist, session, shrine, collection focus | always visible |
| Transport band | playback, progress, state, compact quality cues | always visible |

### Utility surfaces

These can be pinned, collapsed, expanded, or floated inside the shell.

| Surface | Role | Default behavior |
|---|---|---|
| Queue well | up next, sculpting, save/restore scenes | pinned right by default |
| Context well | lyrics, provenance, recommendations, inspector | mode-switched utility well |
| Room/automation well | calm digest of background work | collapsed or secondary by default |

### Overlay surfaces

These are layered above the shell and should feel like lids, drawers, or operational sheets.

| Surface | Role | Default behavior |
|---|---|---|
| Workstation lid | downloads, import, repair, review, diagnostics | opens above listening shell |
| Command palette | fast jump/action surface | transient overlay |
| Focus/shrine layer | immersive now playing or album/session focus | transient overlay or dedicated mode |

### Breakout candidates

These may become true Tauri windows later.

| Candidate | Why it may break out |
|---|---|
| Mini player | useful as a small persistent desktop companion |
| Queue | can benefit from secondary monitor or detached placement |
| Lyrics | low-coupling utility surface |
| EQ/audio tools | classic detachable module |
| Visualizer | naturally independent visual surface |
| Workstation tool windows | only after overlay workflow is proven |

## Region Contract

### Left region: library rail

Must do:

- stay present during normal use
- support resizing
- support compact and expanded density
- let the user move from owned material to active listening context without losing shell state
- behave more like an Explorer-style browser/filter surface than a plain navigation list

Must not do:

- behave like a mere nav list
- disappear on normal content changes

### Center region: active surface

Must do:

- host the current primary listening object
- support edge-to-edge composition
- avoid giant route-page padding
- preserve neighboring shell context

Must not do:

- replace the whole app feeling every time content changes

### Right region: utility well

Must do:

- support tabbed or mode-switched utility context
- support resize, collapse, pin, and reopen
- be allowed to host queue, lyrics, context, and room surfaces
- support detail and preview behavior in the spirit of old Explorer side/detail affordances

Must not do:

- become a junk drawer of random panels

### Bottom region: transport band

Must do:

- stay visible
- stay truthful
- expose direct playback action
- reflect runtime state without drift

Must not do:

- become the only place the app feels alive

## Behavior Contract

### Layout behaviors required in the first shell pass

Must exist:

- left rail resize
- right well resize
- right well collapse and reopen
- persistent geometry
- persistent open/closed state
- persistent active well mode

May wait:

- internal floating surfaces
- overlapping draggable modules
- true breakout windows

### Floating inside the main shell

Deferred for the first shell pass.

Allowed only after resize and persistence are solid.

Floating surfaces inside the main shell must:

- share the same state model as docked surfaces
- preserve focus predictably
- be restorable if hidden or off-bounds

Floating is not required for the first implementation pass.
Floating should stay conceptually available, but it is not a phase-one requirement.

### True detached windows

Detached windows are phase-two behavior.

Detached windows must:

- be backed by Rust-owned shared state
- restore position and size
- detect and recover from off-screen placement
- survive monitor-layout changes gracefully
- not own authoritative playback state locally

Detached windows must not:

- become the first shell milestone
- be used to avoid solving the main workspace contract

## Visual Composition Rules

Hard requirements:

- edge-to-edge regions, not stacked card islands
- minimal visible framing
- region identity by placement and local controls, not repeated title bars
- compact, tactile controls
- asymmetry where it improves feel

Hard bans:

- repeated "panel headers" on every region
- giant rounded mobile-app controls
- spacious SaaS-dashboard emptiness
- interchangeable card grids as the dominant composition

## State Ownership Contract

### Rust owns

- playback truth
- queue truth
- now-playing truth
- window manifest truth once breakout begins
- layout persistence file or persisted shell config if implemented outside the renderer

### Renderer owns

- local interaction state
- transient drag/resize state
- non-authoritative open/closed UI state during interaction

### Shared stores must never become the sole source of truth for

- playback status
- queue pointer
- detached-window existence

## Persistence Contract

The shell needs durable layout memory.

### Required persisted fields

| Field | Meaning |
|---|---|
| `library.width` | left rail width |
| `utility.width` | right well width |
| `utility.collapsed` | right well visibility |
| `utility.mode` | active mode such as queue, context, room |
| `workstation.open` | whether workstation lid is open |
| `shell.compactPlayer` | compact transport preference |
| `workspace.activePresetId` | selected preset |

### Phase-two persisted fields

| Field | Meaning |
|---|---|
| `floatingSurfaces.*` | in-shell floating surface geometry and z-order |
| `detachedWindows.*` | true window geometry, open state, monitor recovery metadata |

### Persistence storage choice

This contract does not force the storage medium yet.

Allowed:

- Tauri-side JSON file
- settings-backed persistence
- hybrid shell config path

Not allowed:

- ephemeral local-only behavior with no restore path

## Workspace Presets

Presets are first-class and should eventually reshape the shell.

Initial preset vocabulary from the conversation:

- Listen
- Crate Dig
- Build Playlist
- Acquisition
- Tag / Repair
- Full Shrine

The first two presets are now locked:

- `Listen / Queue`
- `Acquisition`

Phase-one rule:

- define the preset model now
- actual preset switching behavior can start small
- deeper preset-choreography behavior (for example animated transitions and richer per-preset
	layout cascades) remains follow-on work

Minimum preset payload:

- left width
- right width
- right mode
- workstation open/closed
- optional focus surface

### `Listen / Queue`

Purpose:

- primary listening mode
- now playing and queue are immediately legible
- playback and sculpting stay close together

Expected shell posture:

- library rail visible
- queue well available and reopened when queue content exists or when explicitly toggled
- workstation closed
- center surface favors now-playing, album, artist, playlist, or queue-adjacent listening work

### `Acquisition`

Purpose:

- professional, clean operational mode
- more like qBittorrent and the arr apps than a quirky consumer page
- search, progress, review, and status are visible without infecting the default listening posture

Expected shell posture:

- workstation deck available as the main active operational surface
- search and progress are prominent
- listening shell remains present underneath
- operator density is cleaner and more technical, not bubbly or decorative

## Workstation Contract

The conversation clearly describes Workstation as more arr-like than page-like.

That means:

- treat it as an operational lid or deck over the listening shell
- keep it visually and behaviorally distinct from the listening world
- do not let it become just another route page with cards

Phase-one allowed forms:

- sliding side deck

Current owner leaning:

- `left edge`, pending later feel validation

Phase-one disallowed form:

- "Downloads page but prettier"
- full detached workstation window as the first implementation move

The deck should read more like:

- qBittorrent
- Lidarr/Radarr/Sonarr
- a clean operational search/progress/review deck

The deck should not read like:

- a playful bubble panel
- a quirky card page
- a marketing-style overlay

## Implementation Sequence

### Phase 1: action-spine audit

Before broad shell changes:

- trace primary actions
- classify truthful versus drifting affordances
- remove or downgrade fake surfaces

### Phase 2: main shell foundation

Build:

- resizable library rail
- center active surface
- resizable utility well
- persistent transport band
- layout persistence

### Phase 3: workstation lid

Build:

- arr-style operational overlay
- boundary between listening and operator density

### Phase 4: preset model

Build:

- preset data structure
- at least one or two meaningful preset transitions

### Phase 5: selective breakout proof

Build:

- one detached module first
- then one more if the pattern is clean

Recommended first detached proof:

- visualizer

## Open Questions That Need Owner Setup

There are no blocking owner questions left for the action-spine audit or first shell-foundation pass.

One non-blocking validation remains:

1. Confirm whether the Workstation deck should stay on the provisional `left edge` after feel testing in implementation.

## Acceptance

This contract is good enough to build from when:

- a future agent can describe the shell without defaulting to routes and cards
- docked versus overlay versus detached surfaces are clearly separated
- persistence needs are named
- workstation behavior is no longer interpreted as a normal page flow
- owner decision questions are explicit instead of being silently guessed
