# Object Model Decisions

Last updated: 2026-04-08
Status: approved for implementation

## Purpose

Cassette cannot build strong listening surfaces while treating Playlist, Crate, Session, and Queue Scene as fuzzy synonyms.

These objects are distinct. This file defines them, their boundaries, and their conversion rules.

## Non-Negotiable Rules

1. Playlist is not Crate.
2. Session is not Queue Scene.
3. Queue stays volatile by default, but saveable on purpose.
4. Conversions are explicit. Nothing silently changes object class.
5. New persistence must be additive and surface-driven. No broad schema convergence.

## Definitions

| Object | Exact meaning | Ordered | Authored | Volatile or durable | Primary surface |
|---|---|---|---|---|---|
| Playlist | A durable, intentionally ordered listening object meant to be revisited, edited, and named | yes | yes | durable | Playlists |
| Crate | A saved or temporary slice of the collection used for gathering, filtering, comparing, and staging listening material | optional | lightly | durable or temporary | Collection, Playlists, Queue |
| Session | A listening arc that captures a planned or lived run, including sequence, branches, and memory of what happened | yes | yes | durable | Session |
| Queue Scene | A saveable snapshot of queue state at a moment, including current position and sculpt choices | yes | lightly | durable snapshot of volatile state | Queue |

## Exact Semantics

### Playlist

Playlist means:

- named
- explicitly ordered
- intentionally authored
- can contain notes, sections, arc labels, and variants
- can be played directly without additional shaping

Playlist does not mean:

- temporary search result
- queue dump
- auto-generated session memory unless explicitly exported

### Crate

Crate means:

- a slice of the collection
- can be based on filters, manual picks, or both
- may be temporary or saved
- may be unordered or loosely ordered
- useful for collecting candidates before authorship hardens

Crate does not mean:

- final authored sequence
- permanent playback contract

### Session

Session means:

- a listening arc with time and memory
- may begin as a generated plan
- may become a lived record
- may branch, be replayed, or be exported

Session does not mean:

- just another playlist
- just the current queue

### Queue Scene

Queue Scene means:

- a snapshot of the queue at a moment
- includes order and current pointer
- can also include sculpt state such as pinned items, held items, and cut points
- is meant for fast save and restore

Queue Scene does not mean:

- long-form authorship
- collection slicing

## Conversion Rules

These rules are strict.

| From | To | Allowed | Rule |
|---|---|---|---|
| Playlist | Queue Scene | yes | load playlist into queue, then save current queue state as a scene |
| Playlist | Session | yes | start a session from a playlist; resulting session is separate from playlist |
| Crate | Playlist | yes | only after explicit freeze into a named order |
| Crate | Queue Scene | yes | queue selected crate members, then save as scene |
| Crate | Session | yes | start a session from selected crate material |
| Session | Playlist | yes | explicit export only; choose planned order or lived order |
| Session | Queue Scene | yes | save current or branch state as a queue scene |
| Queue Scene | Playlist | yes | explicit freeze into playlist |
| Queue Scene | Session | yes | start or continue session from restored scene |
| Queue Scene | Crate | no | scenes preserve queue state, not collection slice intent |

## Backend Scope Allowed For This Cycle

Only additive storage and commands that unlock the named surfaces are allowed.

Primary touchpoints:

- `ui/src/lib/api/tauri.ts`
- `ui/src/lib/stores/playlists.ts`
- `ui/src/lib/stores/queue.ts`
- `ui/src/lib/stores/crates.ts` (new)
- `ui/src/lib/stores/sessions.ts` (new)
- `ui/src/lib/stores/queue-scenes.ts` (new)
- `src-tauri/src/commands/playlists.rs`
- `src-tauri/src/commands/queue.rs`
- `src-tauri/src/commands/player.rs`
- `src-tauri/src/commands/library.rs`
- `crates/cassette-core/src/db/mod.rs`
- `src-tauri/src/lib.rs`

Allowed additive data:

- playlist notes
- playlist sections
- playlist variants
- crates
- crate membership or filter payload
- sessions and session branches
- queue scenes

Not allowed:

- broad runtime-sidecar merge
- replacing existing playback tables wholesale
- generic "unified object" abstraction

## File-Level Implementation Direction

### `ui/src/lib/api/tauri.ts`

Add first-class interfaces for:

- `PlaylistSection`
- `PlaylistVariant`
- `Crate`
- `Session`
- `SessionBranch`
- `QueueScene`

### `ui/src/lib/stores/playlists.ts`

Extend store behavior to load and mutate:

- notes
- sections
- variants

### `ui/src/lib/stores/crates.ts`

Create crate store for:

- saved crates
- temporary crates
- promote-to-playlist action

### `ui/src/lib/stores/sessions.ts`

Create session store for:

- planned session
- live session capture
- replay
- export to playlist

### `ui/src/lib/stores/queue-scenes.ts`

Create queue scene store for:

- save current queue
- restore scene
- quick pivot

### `src-tauri/src/commands/playlists.rs`

Add additive playlist surface commands only:

- notes
- sections
- variants
- export session to playlist

### `src-tauri/src/commands/queue.rs`

Add queue sculpt commands only:

- play after current
- pin
- hold
- cut after this
- save queue scene
- restore queue scene

### `src-tauri/src/commands/player.rs`

Add session capture and replay helpers only if they unlock Session or Now Playing.

### `crates/cassette-core/src/db/mod.rs`

Add minimal durable tables and helpers for the new objects.

The rule is simple:

- add narrow tables
- add narrow queries
- do not rewrite existing playlist or queue storage just to look cleaner

## Acceptance

This object model is implemented correctly when:

- Playlist, Crate, Session, and Queue Scene have separate interfaces and commands
- each object has a clear primary surface
- conversions are explicit and named in the UI
- no surface uses one object as a disguised copy of another

