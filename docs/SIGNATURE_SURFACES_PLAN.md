# Signature Surfaces Plan

Last updated: 2026-04-08
Status: active execution plan
Owner: Christian (Capn)

## Mission

Turn Cassette from a credible system into a desirable place to listen, collect, and return to.

This cycle is not about backend expansion for its own sake. It is about:

- listener-first daily flow
- collector-first ownership and edition visibility
- authored surface identity
- calm automation handoff
- plain-language trust without exposing internal runtime jargon

## Hard Constraints

These are fixed for this cycle:

1. No new providers.
2. No broad schema convergence.
3. No oracle, chatbot, or assistant layer.
4. No raw planner, director, or control-plane language on primary listening surfaces.
5. Workstation is secondary. It cannot become the real app.
6. Backend changes must unlock a named surface. If they do not, they do not land in this wave.

## Current Surface Failures

These are not abstract taste notes. They are the current repo problems.

1. `ui/src/routes/collection/+page.svelte` is a stats dashboard, not an ownership ritual.
2. `ui/src/routes/session/+page.svelte` and `ui/src/lib/components/SessionComposer.svelte` are tool posture, not memory or arc posture.
3. `ui/src/routes/playlists/+page.svelte` is CRUD plus track list. It has no authorship spine.
4. `ui/src/lib/components/QueuePanel.svelte` is only reorder/remove. It is not sculptable.
5. `ui/src/lib/components/NowPlaying.svelte` and `ui/src/lib/components/NowPlayingExpanded.svelte` are informative but not immersive.
6. `ui/src/lib/components/Sidebar.svelte` puts operator surfaces too close to the primary listening loop.
7. `ui/src/routes/downloads/+page.svelte` still leaks "command center", "debug", raw request language, and provider-heavy posture into the main product voice.
8. `ui/src/app.css` sets a generic dark shell, weak contrast values, a network-loaded font import, and visible mojibake.

## Anti-Feature List

Do not spend this cycle on any of the following:

- new provider integrations
- broad DB unification
- new AI recommendation layers
- generic dashboard expansion
- more diagnostic density on Home
- chart-first collection views
- more shell chrome for its own sake
- making Downloads prettier without moving diagnostics behind Workstation boundaries
- inventing new nouns when existing product nouns are enough

## Surface Ownership

Each named surface gets one ritual and one owner story. If a feature does not strengthen that ritual, it does not belong there.

| Surface | Ritual | Primary files | What changes | What does not change | Acceptance gate |
|---|---|---|---|---|---|
| Home | Return ritual | `ui/src/routes/+page.svelte`, `ui/src/routes/+layout.svelte`, `ui/src/lib/components/Sidebar.svelte`, `ui/src/lib/components/SystemStatusStrip.svelte` | Make Home the calm return point: resume, arrivals, unfinished business, recent memory, low-noise handoff | Do not turn Home into another dashboard or workstation summary wall | Launch to first meaningful playback action is shorter and clearer than current Home |
| Collection | Ownership ritual | `ui/src/routes/collection/+page.svelte`, `ui/src/lib/api/tauri.ts`, `src-tauri/src/commands/library.rs`, `crates/cassette-core/src/db/mod.rs` | Replace stats-first view with shelves, best-copy view, archive health, provenance, and edition presence | Do not remove auditability or existing collection stats APIs; demote them below collector shelves | Collector can answer "what do I own, what is best, what is missing, what is fragile?" without leaving Collection |
| Album | Edition ritual | `ui/src/routes/albums/[albumId]/+page.svelte` (new), `ui/src/routes/artists/+page.svelte`, `ui/src/routes/library/+page.svelte`, `ui/src/lib/api/tauri.ts`, `src-tauri/src/commands/library.rs` | Add dedicated album page with edition, provenance, best copy, related versions, track quality context | Do not collapse album identity back to ad hoc strings once a stable album route exists | Album page makes edition choice and best-copy status legible in one screen |
| Artist | Rediscovery ritual | `ui/src/routes/artists/+page.svelte`, `ui/src/lib/artist-clusters.ts`, `ui/src/lib/api/tauri.ts`, `src-tauri/src/commands/library.rs` | Move from grid-of-covers to rediscovery rails: missing-from-artist, related versions, strongest albums, gaps worth filling | Do not lose current artist clustering or gap logic | Artist view surfaces reasons to return to an artist, not just browse their rows |
| Playlists | Authorship ritual | `ui/src/routes/playlists/+page.svelte`, `ui/src/lib/stores/playlists.ts`, `ui/src/lib/api/tauri.ts`, `src-tauri/src/commands/playlists.rs`, `crates/cassette-core/src/db/mod.rs` | Add notes, sections, arc labels, and variants | Do not break current playlist playback path | Playlists feel made, not merely saved |
| Queue | Sculpting ritual | `ui/src/routes/queue/+page.svelte` (new), `ui/src/lib/components/QueuePanel.svelte`, `ui/src/lib/stores/queue.ts`, `ui/src/lib/api/tauri.ts`, `src-tauri/src/commands/queue.rs` | Add play-after-current, pin, hold, cut-after-this, queue scenes, restore, pivots | Do not break existing queue ordering and playback continuity | Queue can be shaped mid-listen without feeling like list surgery |
| Session | Arc ritual | `ui/src/routes/session/+page.svelte`, `ui/src/lib/components/SessionComposer.svelte`, `ui/src/lib/api/tauri.ts`, `src-tauri/src/commands/player.rs`, `crates/cassette-core/src/db/mod.rs` | Shift from "composer" tool to memory-plus-arc surface with replay, branching, export/import | Do not remove current explainable session generation until replacement exists | Session preserves both planned arcs and lived arcs |
| Now Playing | Immersion ritual | `ui/src/lib/components/NowPlaying.svelte`, `ui/src/lib/components/NowPlayingExpanded.svelte`, `ui/src/lib/stores/player.ts`, `ui/src/lib/api/tauri.ts` | Make art dominant, reduce chrome, show provenance and quality context, support focus mode | Do not regress playback controls, lyrics, or queue continuity | Now Playing becomes the emotional center instead of a utility strip |
| Workstation | Control ritual | `ui/src/routes/workstation/+page.svelte` (new), `ui/src/routes/downloads/+page.svelte`, `ui/src/routes/import/+page.svelte`, `ui/src/routes/tools/+page.svelte`, `ui/src/routes/settings/+page.svelte`, `ui/src/lib/stores/commands.ts` | Consolidate operator-heavy surfaces under a clear control area one click away from listening | Do not delete existing tooling or diagnostics; move and rename them with calmer language | Diagnostics stay available without dominating the listening shell |

## Build Order

The execution order below is mandatory. Later waves depend on earlier boundary and language calls.

### Wave 0 - Quality Floor

Purpose:

- remove obvious surface breakage before larger rebuilds

Primary files:

- `ui/src/app.css`
- `ui/src/routes/+page.svelte`
- `ui/src/routes/collection/+page.svelte`
- `ui/src/routes/artists/+page.svelte`
- `ui/src/routes/playlists/+page.svelte`
- `ui/src/routes/session/+page.svelte`
- `ui/src/routes/downloads/+page.svelte`
- `ui/src/lib/components/Sidebar.svelte`
- `ui/src/lib/components/NowPlaying.svelte`
- `ui/src/lib/components/NowPlayingExpanded.svelte`
- `ui/src/lib/components/QueuePanel.svelte`
- `ui/src/lib/components/RightSidebar.svelte`
- `ui/src/lib/api/tauri.ts`

Changes:

- fix mojibake and encoding corruption
- raise the contrast floor for primary text and key metadata
- remove network font dependency and replace it with bundled or system-safe typography
- reduce primary-surface a11y suppression debt
- improve reading rhythm for long sessions

Must not change:

- route architecture
- playback behavior
- download behavior

Acceptance:

- no visible mojibake remains on primary surfaces
- no primary surface relies on remote font loading
- keyboard focus is visible across Home, Collection, Artists, Playlists, Queue, Session, and Now Playing
- `npm run build` still passes

Risks:

- broad CSS token changes can quietly break smaller utility surfaces

### Wave 1 - Identity, Boundaries, Language

Purpose:

- make the shell listening-first and lock language rules before larger rebuilds

Primary files:

- `docs/SIGNATURE_SURFACES_PLAN.md`
- `docs/EXPERIENCE_BOUNDARY_MAP.md`
- `docs/VISUAL_SYSTEM_DIRECTION.md`
- `docs/OBJECT_MODEL_DECISIONS.md`
- `ui/src/routes/+layout.svelte`
- `ui/src/lib/components/Sidebar.svelte`
- `ui/src/lib/stores/commands.ts`
- `ui/src/lib/components/NowPlaying.svelte`
- `ui/src/lib/components/NowPlayingExpanded.svelte`

Changes:

- regroup navigation into listening-first surfaces
- add Workstation as one click away, not a co-equal front door
- rename surface copy away from operator jargon
- make Now Playing calmer and more art-led without full shrine scope yet

Must not change:

- sidecar/runtime split
- current acquisition and trust behavior

Acceptance:

- sidebar top section contains only listening surfaces
- Workstation is reachable in one click but not presented as the main product
- Home, Collection, Artists, Playlists, Queue, Session, and Now Playing do not display banned internal terms

Risks:

- naming drift between routes, command palette, and shell labels

### Wave 2 - Ownership Wave

Purpose:

- make the collection feel owned rather than measured

Primary files:

- `ui/src/routes/collection/+page.svelte`
- `ui/src/routes/artists/+page.svelte`
- `ui/src/routes/library/+page.svelte`
- `ui/src/routes/albums/[albumId]/+page.svelte` (new)
- `ui/src/lib/artist-clusters.ts`
- `ui/src/lib/api/tauri.ts`
- `src-tauri/src/commands/library.rs`
- `crates/cassette-core/src/db/mod.rs`

Changes:

- rebuild Collection around shelves, best-copy stories, archive health, and provenance
- add dedicated Album view
- rebuild Artist around rediscovery, missing-from-artist, and related-version rails

Must not change:

- deterministic album and artist IDs
- existing artist clustering safety
- current collection stats endpoint; keep it available for secondary panels

Acceptance:

- charts exist only as subordinate support
- Album view exposes edition, provenance, and best-copy status
- Artist view surfaces gaps and related versions without forcing Downloads open

Risks:

- album route work can tempt backend overreach; keep it additive and projection-focused

### Wave 3 - Daily-Use Wave

Purpose:

- make saved listening objects and live queueing feel authored instead of mechanical

Primary files:

- `ui/src/routes/playlists/+page.svelte`
- `ui/src/routes/queue/+page.svelte` (new)
- `ui/src/lib/components/QueuePanel.svelte`
- `ui/src/lib/components/NowPlayingExpanded.svelte`
- `ui/src/lib/stores/playlists.ts`
- `ui/src/lib/stores/queue.ts`
- `ui/src/lib/api/tauri.ts`
- `src-tauri/src/commands/playlists.rs`
- `src-tauri/src/commands/queue.rs`
- `crates/cassette-core/src/db/mod.rs`

Changes:

- add playlist notes, sections, arc labels, and variants
- introduce Crates as saved or temporary collection slices
- add queue sculpt actions and queue scene save/restore

Must not change:

- existing playlist play path
- existing queue order semantics unless a saved scene is explicitly loaded

Acceptance:

- playlist can carry authored structure beyond title/description
- queue supports play next, hold, pin, cut-after-this, and scene restore
- crate can be promoted to playlist only through explicit freeze action

Risks:

- object model confusion if playlist and crate drift toward the same thing

### Wave 4 - Emotional Wave

Purpose:

- turn Session and Now Playing into the emotional center

Primary files:

- `ui/src/routes/session/+page.svelte`
- `ui/src/lib/components/SessionComposer.svelte`
- `ui/src/lib/components/NowPlaying.svelte`
- `ui/src/lib/components/NowPlayingExpanded.svelte`
- `ui/src/lib/api/tauri.ts`
- `src-tauri/src/commands/player.rs`
- `crates/cassette-core/src/db/mod.rs`

Changes:

- session memory, replay, branch recall, export/import to playlist
- full Now Playing shrine pass with focus mode, provenance, and reduced chrome

Must not change:

- current playback controls
- current lyrics and context fetches

Acceptance:

- session can preserve what actually happened, not just what was generated
- now playing can hold focus without exposing workstation noise

Risks:

- visual ambition can overpower legibility; the shrine still has to be usable for long sessions

### Wave 5 - Calm Automation Wave

Purpose:

- keep automation visible without letting it colonize listening surfaces

Primary files:

- `ui/src/routes/workstation/+page.svelte` (new)
- `ui/src/routes/downloads/+page.svelte`
- `ui/src/routes/import/+page.svelte`
- `ui/src/routes/tools/+page.svelte`
- `ui/src/routes/settings/+page.svelte`
- `ui/src/lib/components/SystemStatusStrip.svelte`
- `ui/src/lib/stores/commands.ts`

Changes:

- move downloads summary language in main app toward digest form
- keep deeper diagnostics and review detail in Workstation
- add inbox/history digest and calm automation thresholds

Must not change:

- audit detail availability
- replay and review paths

Acceptance:

- primary surfaces only show digest, soft attention, or explicit intervention when warranted
- Workstation holds raw diagnostics, history, and fix surfaces

Risks:

- over-hiding blocked work; visibility must stay high even as noise drops

### Wave 6 - Full Visual System Pass

Purpose:

- make the whole app feel like one authored place

Primary files:

- `ui/src/app.css`
- `ui/src/routes/+layout.svelte`
- all primary route files
- all primary listening components

Changes:

- unify spacing rhythm, hierarchy, density, idle beauty, and playback-active behavior

Must not change:

- keyboard flow
- low-motion parity
- long-session readability

Acceptance:

- navigation, cards, shelves, rails, chips, and overlays read as one system
- playback-active shell behavior is calmer and more legible than current shell

Risks:

- polish-only churn; every visual change must support ritual ownership, clarity, or comfort

## Minimum Viable Implementation Order

This is the smallest real order that can be handed to an implementation agent:

1. Wave 0 quality floor.
2. Wave 1 shell boundary and language pass.
3. Add dedicated Album route and Collection ownership rebuild.
4. Rebuild Artist around rediscovery rails.
5. Extend Playlists with authored metadata.
6. Add Queue route plus sculpt actions and queue scenes.
7. Reframe Session around memory and replay.
8. Finish Now Playing shrine pass.
9. Add Workstation hub and digest thresholds.
10. Run full visual system unification.

## Do Not Regress

- reversibility of file-affecting actions
- auditability and request lineage
- no-interruptive-noise behavior
- artist-first worldview
- sidecar/runtime boundary
- existing proven acquisition flows
- keyboard-only flow on primary listening surfaces
- low-motion parity
- clear blocked-work visibility

