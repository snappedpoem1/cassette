# Music System Truth

Last condensed: March 20, 2026

## Product Identity

The real product name is `Cass\ette`.

`Cass` is the AI layer underneath the app.

Older `Lyra` material in the export should be treated as prior naming and earlier framing for the same broader music-intelligence ambition, not as the preferred current product name.

`Cass\ette` is the current shell, focused on:

- local playback
- library stewardship
- queue control
- provider setup
- acquisition trust

The export points toward a larger music-intelligence system behind the shell.
That larger ambition should remain as roadmap context for `Cass\ette` and `Cass`, but the immediate scope is to harden the downloader and playback baseline first and reintroduce broader intelligence after stabilization.

## Canonical Runtime

The canonical runtime described across the docs is:

- Tauri 2 desktop shell
- SvelteKit frontend
- Rust core
- SQLite local store

Python should not be part of canonical startup, playback, queue, library, or settings flow.

## Current Capability Summary

The export describes meaningful backend ownership in Rust for:

- library roots and scanning
- tracks, albums, artists, playlists, queue, and playback state
- provider config and provider validation
- acquisition queue state
- downloader orchestration
- prompt-to-playlist draft generation
- recommendation and explanation payloads
- some graph and lineage intelligence
- some audio-feature extraction

The acceptance matrix in the export says:

- 12 backend areas pass
- 2 backend areas are partial
- 0 backend areas fail

That does not mean the whole product promise is complete.
It means the backend baseline was materially real.

## Current Delivery Phase

The shell is intentionally simplified for stabilization.
Priority order in the export:

1. native acquisition parity
2. provider auth and transport autonomy
3. discovery graph and bridge depth
4. composer and playlist intelligence depth
5. explainability and provenance breadth
6. packaged desktop confidence

## Acquisition Truth

Canonical acquisition ownership after the scope reset:

1. MusicBrainz as the identity spine before acquisition
2. Qobuz and Deezer as premium acquisition adapters
3. Jackett as torrent search owner
4. Real-Debrid as torrent resolver/unrestrict owner
5. slskd, Usenet, and yt-dlp as fallback acquisition adapters

The docs say:

- provider orchestration is tier-owned
- Rust owns the main waterfall
- acquired media is verified with Symphonia before library admission
- false lossless claims are rejected

## Scope Rule

Current in-scope truth should come from the live `C:\Cassette Music` repo.

Useful ideas from the export should still be preserved, but labeled as:

- future roadmap
- deferred capability
- reintroduction candidate after stabilization

They should not be treated as currently shipped truth unless the repo supports them now.
