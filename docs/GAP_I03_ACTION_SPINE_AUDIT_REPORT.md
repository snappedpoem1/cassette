# GAP-I03 Action-Spine Audit Report

Last updated: 2026-04-09
Status: completed with explicit untested rows
Owner: Christian (Capn)
Gap ID: `GAP-I03`

## Scope And Method

This audit was executed against the canonical direction reset:

1. `docs/MODULAR_DESKTOP_DIRECTION_RESET.md`
2. `docs/MODULAR_WORKSPACE_CONTRACT.md`
3. `docs/MODULAR_WORKSPACE_EXECUTION_PLAN.md`
4. `docs/GAP_I03_ACTION_SPINE_AUDIT_BRIEF.md`

Method used:

- static trace across Svelte surface -> store/handler -> Tauri command/event -> Rust state path
- repo verification pass via `cargo check --workspace`, `npm run build`, and `.\scripts\smoke_desktop.ps1`
- desktop boot probe via `cargo tauri dev --no-watch`

Important boundary:

- This session proved build/readiness and traced the action spine end to end in code.
- This session did not produce full native click-through proof for every desktop interaction.
- Per the brief, rows that depend on live desktop observation and were not fully exercised are marked `untested`, not `truthful`.

## A. Action Matrix

| Surface | Control | Expected behavior | Actual behavior | Truth status | Path(s) |
|---|---|---|---|---|---|
| Shell chrome | Command palette open | Overlay opens and accepts commands from the listening shell | Local palette state and overlay path exist, but no native interaction proof was captured in this session | `untested` | `ui/src/routes/+layout.svelte:230-254`; `ui/src/lib/components/CommandPalette.svelte:88-147`; `ui/src/lib/stores/commands.ts:182-203` |
| Shell chrome | Compact player toggle | Toggle bottom-player presentation and persist the preference | The toggle only flips a localStorage-backed flag and shell class; it does not prove any broader workspace behavior and was not runtime-observed here | `untested` | `ui/src/routes/+layout.svelte:224-225`; `ui/src/lib/stores/shell.ts:5-27` |
| Shell chrome | Minimize window | Minimize the real desktop window | Tauri minimize path exists, but errors are swallowed and no live minimize/restore observation was captured | `untested` | `ui/src/routes/+layout.svelte:227-228`; `ui/src/lib/stores/shell.ts:29-70` |
| Command system | "Open Workstation" / "Open Library Browser" / route commands | Open shell-owned surfaces | The command system is mostly route navigation, not surface orchestration, docking, or shell state changes | `fake` | `ui/src/lib/stores/commands.ts:35-160`; `ui/src/routes/+layout.svelte:239-245` |
| Playback transport | Play / pause toggle | Invoke playback runtime and reflect truthful state back into the shell | The invoke path exists, but failures are swallowed with no operator signal; if the runtime path breaks, the control quietly pretends nothing happened | `drifting` | `ui/src/lib/components/NowPlaying.svelte:220-230`; `ui/src/lib/stores/player.ts:194-215`; `src-tauri/src/commands/player.rs:39-101` |
| Playback transport | Previous / next | Move queue position and reflect the new track | Backend path exists and the component reloads queue after use, but failures are silent and no live desktop observation was captured | `drifting` | `ui/src/lib/components/NowPlaying.svelte:149-163,215-235`; `ui/src/lib/stores/player.ts:223-236`; `src-tauri/src/commands/player.rs:103-164` |
| Playback transport | Seek | Move playback head and reflect confirmed position | The frontend updates local `playbackState.position_secs` even if the Tauri seek invoke fails, so the bar can move without confirmed runtime seek | `drifting` | `ui/src/lib/components/NowPlaying.svelte:47-68,87-109,249-267`; `ui/src/lib/stores/player.ts:238-247`; `src-tauri/src/commands/player.rs:174-176`; `crates/cassette-core/src/player/mod.rs:94` |
| Playback transport | Volume | Change runtime volume and reflect confirmed level | The frontend updates local `playbackState.volume` even if the backend set-volume command fails, so UI confidence can outrun runtime truth | `drifting` | `ui/src/lib/components/NowPlaying.svelte:70-85,111-130,289-304`; `ui/src/lib/stores/player.ts:249-256`; `src-tauri/src/commands/player.rs:166-170`; `crates/cassette-core/src/player/mod.rs:98` |
| Playback reflection | Progress / now-playing identity / art | Poll and event paths keep the shell synced to real playback state | There is now a real poll path and a real `playback_state_changed` event path, plus runtime player-event supervision, but no full native observation was captured here | `untested` | `ui/src/lib/stores/player.ts:148-190`; `src-tauri/src/state.rs:885-1003`; `src-tauri/src/commands/player.rs:15-23,180-181` |
| Playback continuity | End-of-track behavior | Advance to next queued track or stop cleanly | Runtime listener and track-end handler exist in the desktop state layer, but this session did not observe an actual end-of-track transition in the app | `untested` | `src-tauri/src/state.rs:904-970`; `crates/cassette-core/src/player/mod.rs:503` |
| Queue | Load queue | Show actual runtime queue contents | Queue load path exists, but failures collapse to `[]`, which makes broken state look indistinguishable from an empty queue | `drifting` | `ui/src/lib/stores/queue.ts:10-15`; `ui/src/lib/components/QueuePanel.svelte:109-173`; `src-tauri/src/commands/queue.rs:5-8` |
| Queue | Add / clear / remove / reorder | Mutate queue and reflect the new order truthfully | Runtime mutation paths exist, but the UI mostly recovers by reloading afterward and several failure branches degrade into silent refresh or empty-state fallback | `drifting` | `ui/src/lib/stores/queue.ts:18-51`; `ui/src/lib/components/QueuePanel.svelte:14-106,115-172`; `src-tauri/src/commands/queue.rs:11-63` |
| Queue / playlist handoff | Play playlist into queue | Replace queue, start playback, and refresh queue-facing surfaces | Backend and UI paths exist and now explicitly reload queue after playlist launch, but this session did not observe the handoff live | `untested` | `ui/src/lib/stores/playlists.ts:41-44`; `src-tauri/src/commands/playlists.rs:64-110` |
| Library -> playback | Play album / play track | Start playback from library surfaces without losing shell playback context | Library actions directly call queue/play APIs; the shell transport lives outside routed content, but runtime behavior was not manually exercised in this session | `untested` | `ui/src/routes/library/+page.svelte:48-60,182-205,317-339`; `ui/src/lib/components/ContextActionRail.svelte:49-130`; `ui/src/lib/stores/queue.ts:18-23`; `src-tauri/src/commands/queue.rs:25-63` |
| Library browsing model | Library as persistent browser/preview surface | Stay present as a shell-owned browser/filter rail with preview/detail behavior | The actual implementation is a dedicated `/library` page with page-local album/track state, not a persistent shell rail or workspace surface | `fake` | `ui/src/routes/library/+page.svelte:15-418`; `ui/src/routes/+layout.svelte:239-245`; `ui/src/lib/stores/commands.ts:107-112` |
| Utility well | Right-rail queue / room / context tabs | Switch contextual surfaces that stay truthful to runtime and shell state | Tab switching itself is real, but `Room` is an informational digest and `Context` depends on soft-fail data loads; errors degrade to empty or reduced states without explicit trust signaling | `drifting` | `ui/src/lib/components/RightSidebar.svelte:12-150`; `ui/src/lib/stores/downloads.ts:29-46,67-84,189-221`; `ui/src/lib/stores/player.ts:54-76` |
| Workstation entry posture | Sidebar/palette/workstation/downloads entry | Open a workstation-owned surface without replacing the main listening workspace | Current behavior is still route-swapping inside the main slot; the shell keeps the player chrome visible, but workstation behavior is page navigation, not a shell deck or modular lid | `fake` | `ui/src/routes/+layout.svelte:235-252`; `ui/src/lib/components/Sidebar.svelte`; `ui/src/lib/stores/commands.ts:99-112`; `ui/src/routes/workstation/+page.svelte:58-214` |
| Workstation surface | Workstation room itself | Act like a clean operational deck adjacent to listening, not another card page | The current workstation is still a route page built from cards and route links, so it overstates modularity and underdelivers the intended deck behavior | `fake` | `ui/src/routes/workstation/+page.svelte:94-214` |

## B. Findings

1. Primary playback controls can drift away from runtime truth because failures are swallowed and two controls mutate local playback state even when backend commands fail. The most direct break is seek and volume in [player.ts](C:/Cassette%20Music/ui/src/lib/stores/player.ts#L238) and [player.ts](C:/Cassette%20Music/ui/src/lib/stores/player.ts#L249), surfaced by [NowPlaying.svelte](C:/Cassette%20Music/ui/src/lib/components/NowPlaying.svelte#L249) and [NowPlaying.svelte](C:/Cassette%20Music/ui/src/lib/components/NowPlaying.svelte#L289).
2. Queue and supporting data surfaces often translate failures into believable emptiness instead of visible fault. That happens in [queue.ts](C:/Cassette%20Music/ui/src/lib/stores/queue.ts#L10), [downloads.ts](C:/Cassette%20Music/ui/src/lib/stores/downloads.ts#L29), [downloads.ts](C:/Cassette%20Music/ui/src/lib/stores/downloads.ts#L67), [downloads.ts](C:/Cassette%20Music/ui/src/lib/stores/downloads.ts#L189), [library.ts](C:/Cassette%20Music/ui/src/lib/stores/library.ts#L39), and [playlists.ts](C:/Cassette%20Music/ui/src/lib/stores/playlists.ts#L9).
3. The shell still implies a more modular product than it actually is. The app is structurally a fixed shell with a routed content slot in [layout.svelte](C:/Cassette%20Music/ui/src/routes/+layout.svelte#L239), while command and sidebar entry points are still dominated by route jumps in [commands.ts](C:/Cassette%20Music/ui/src/lib/stores/commands.ts#L35) and [workstation/+page.svelte](C:/Cassette%20Music/ui/src/routes/workstation/+page.svelte#L94).
4. Library and workstation do not yet exist as shell-owned workspace surfaces. Library browsing is page-local state inside [library/+page.svelte](C:/Cassette%20Music/ui/src/routes/library/+page.svelte#L24), and workstation is still a card page in [workstation/+page.svelte](C:/Cassette%20Music/ui/src/routes/workstation/+page.svelte#L132).
5. The backend transport spine is substantially better than the frontend trust language suggests. Playback events are now supervised in [state.rs](C:/Cassette%20Music/src-tauri/src/state.rs#L885) and emitted through [player.rs](C:/Cassette%20Music/src-tauri/src/commands/player.rs#L15), but the frontend still hides too much failure and over-relies on silent fallback.

## C. Repair Order

1. Remove silent no-op behavior from primary listening controls. Play, pause, next, previous, seek, volume, minimize, and restore need explicit success/failure handling instead of empty `catch` branches.
2. Stop optimistic local mutation for seek and volume until runtime confirmation comes back, or mark those controls as pending and reconcile from emitted state.
3. Replace empty-on-error behavior on queue, library, downloads, and playlist loads with bounded fault states so "nothing here" and "failed to load" are no longer indistinguishable.
4. Reframe command, sidebar, and workstation entry language so route swaps stop pretending to be modular shell surfaces.
5. Start shell-foundation work only after the trust floor is corrected: library rail, center workspace, contextual well, and workstation deck should be shell-owned surfaces rather than route-first pages.

## D. Follow-Up Gaps

- `GAP-I04`: eliminate silent no-op and optimistic state drift on primary listening controls
- `GAP-I05`: convert workstation and library entry from route-implied surfaces into shell-owned workspace regions/decks

## E. Runtime Evidence

- `cargo check --workspace` passed on 2026-04-09
- `Set-Location ui; npm run build; Set-Location ..` passed on 2026-04-09
- `.\scripts\smoke_desktop.ps1` passed on 2026-04-09
- `cargo tauri dev --no-watch` from repo root stayed alive past the command timeout instead of failing immediately, while the same command from `src-tauri/` failed because `beforeDevCommand` resolves `cd ../ui` relative to the wrong working directory

## F. Reality Statement

Cassette is currently a fixed-shell desktop app with a real Rust playback/runtime spine underneath it, but the visible shell still behaves mostly like a route-swapped listening web app with desktop chrome around it. The backend is more real than the current shell language, and the next honest move is not more page polish: it is to restore trust in action outcomes, then rebuild the shell around persistent workspace surfaces instead of route theater.
