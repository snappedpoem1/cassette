# Experience Boundary Map

Last updated: 2026-04-08
Status: active contract

Supersession note:

- This document still governs listening versus Workstation boundaries and language rules.
- It does not by itself define the shell/workspace/windowing contract.
- For the architecture reset away from route-first drift, read `docs/MODULAR_DESKTOP_DIRECTION_RESET.md` first.

## Purpose

Cassette needs a hard experience boundary between listening surfaces and operator surfaces.

The current repo already has music-first intent, but the shell still mixes daily listening, diagnostics, and operator language too freely. This file fixes that boundary.

## Surface Tiers

### Tier 1: Listening Surfaces

These are the real app. They must stay calm, music-first, and human.

- Home
- Collection
- Album
- Artist
- Playlists
- Queue
- Session
- Now Playing

Rules:

- plain language only
- no raw internal status jargon
- no provider troubleshooting detail
- no task IDs, failure classes, request signatures, or debug labels

### Tier 2: Workstation

This is the control surface. It is allowed to be denser, but it still cannot be hostile.

- Workstation deck or focused Workstation route
- Downloads
- Import
- History
- Tools
- Settings
- diagnostics
- review and replay

Rules:

- diagnostics live here
- raw request and provider detail lives here
- replay, review, and repair live here
- this surface must be one click away, never the default emotional center
- the main shell should expose Workstation as the doorway, not the whole operator sitemap at once

## Route Boundary Map

| Surface | Current route/file shape | Target shape | Boundary rule |
|---|---|---|---|
| Home | `ui/src/routes/+page.svelte` | keep route, reduce operator summary density | return point, not dashboard |
| Collection | `ui/src/routes/collection/+page.svelte` | keep route, change content model | ownership before stats |
| Album | no dedicated route | add `ui/src/routes/albums/[albumId]/+page.svelte` | edition ritual needs its own page |
| Artist | `ui/src/routes/artists/+page.svelte` | keep route, rebuild content | rediscovery before grid browsing |
| Playlists | `ui/src/routes/playlists/+page.svelte` | keep route, rebuild authorship | authored object, not CRUD pane |
| Queue | right rail only via `ui/src/lib/components/QueuePanel.svelte` | add `ui/src/routes/queue/+page.svelte` and keep rail as compact view | sculptable queue needs dedicated surface |
| Session | `ui/src/routes/session/+page.svelte` | keep route, rename and reframe content | memory and arc, not composer-only |
| Now Playing | footer plus overlay | keep component footprint, deepen overlay/focus | immersion surface, not shell utility |
| Workstation | split across `/downloads`, `/import`, `/tools`, `/settings`, `/history` | add `/workstation` hub and keep subroutes reachable | secondary control area |

## Navigation Contract

Primary shell navigation belongs to listening surfaces plus the single Workstation doorway:

- Home
- Collection
- Artists
- Playlists
- Queue
- Session
- Workstation

Secondary navigation belongs inside Workstation:

- Downloads
- Import
- History
- Tools
- Settings

Rules:

- `ui/src/lib/components/Sidebar.svelte` must expose listening surfaces plus Workstation, not the whole operator sitemap.
- `ui/src/lib/stores/commands.ts` must mirror the same hierarchy.
- `ui/src/routes/+layout.svelte` top bar must stop naming the app as a utility console.
- Shell-owned surfaces such as the library rail and Workstation deck are part of the listening shell contract, not route clutter.

## Language Governance

### Allowed Vocabulary

Use these words on primary surfaces:

- Home
- Collection
- Album
- Edition
- Artist
- Playlist
- Crate
- Queue
- Up next
- Session
- Now playing
- Best copy
- Related versions
- Missing
- Arrived
- In progress
- Needs review
- Ready
- Inbox
- History
- Workstation
- Acquire
- Acquisition

### Banned Internal Terms On Primary Surfaces

These terms are banned from Tier 1 surfaces:

- planner
- director
- control-plane
- sidecar
- delta queue
- request signature
- candidate set
- gatekeeper
- dead letter
- task history
- failure class
- provider memory
- debug
- command center
### Translation Rules

When internal terms are needed, translate them before they touch the listening shell.

| Internal term | Primary surface label | Workstation label |
|---|---|---|
| acquisition request | item | request |
| reviewing | needs review | review pending |
| finalized | arrived | finalized |
| already_present | already here | already present |
| failed | blocked | failed |
| cancelled | stopped | cancelled |
| dead letter | retry group | failed replay group |
| candidate review | options checked | candidate review |
| provider health | service status | provider health |
| backlog | inbox | backlog |
| planner approval | review decision | approval |

`Acquire` / `Acquisition` rule:

- allowed when naming the owner-approved preset or the Workstation-facing collection-recovery posture
- not allowed as a leak of deeper runtime jargon on listening copy

## Primary Surface Content Limits

Primary surfaces may show:

- calm summaries
- confidence cues
- provenance and quality chips
- missing or blocked work in human language

Primary surfaces may not show:

- raw JSON
- request IDs
- provider error blobs
- per-provider failure counts
- low-level execution status streams

## Workstation Content Contract

Workstation is where the system can be explicit.

Allowed in Workstation:

- provider diagnostics
- raw request timeline
- repair tooling
- review and replay
- import controls
- deeper automation detail

Still banned in Workstation:

- gratuitous internal jargon when a simpler label works
- debugger-style walls of data with no fix path

## Acceptance

This boundary is only real when all of the following are true:

- listening surfaces do not use banned internal terms
- shell navigation exposes listening surfaces plus Workstation, not the full operator sitemap
- Workstation is present and one click away
- Downloads, Import, Tools, History, and Settings are framed as control surfaces, not as the main app
- command palette labels follow the same vocabulary rules as the visible shell
