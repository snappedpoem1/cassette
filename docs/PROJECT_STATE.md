# Cassette Project State

Last audited: March 21, 2026

## Runtime Truth

- Desktop shell: Tauri 2 (`src-tauri`)
- Renderer: SvelteKit (`ui`)
- Core domain: Rust (`crates/cassette-core`)
- Local store: SQLite (`cassette.db` in Tauri app data)
- Python helpers are non-canonical and not required for app boot.

## Verified As Built

- Rust workspace compiles: `cargo check` passes.
- UI production build passes: `ui` `npm run build` passes.
- Library scanning, queue, playback, playlists, downloads, import, settings, and organizer command surfaces are implemented and wired through Tauri commands.
- Primary shell is now cassette-baseline in navigation:
  `Library`, `Downloads`, `Settings`.
- Advanced routes (`/playlists`, `/import`, `/tools`) still exist but are no longer in primary sidebar flow.

## Current Gaps

- Deezer lane is still partial in current runtime:
  fallback uses preview MP3 URLs and not full ARL-decrypted tracks.
- End-to-end provider live proof remains incomplete on this machine
  (Deezer ARL and Tidal device OAuth acquisition proof).
- Acquisition orchestration still mixes async runtime + legacy-style stages and needs end-to-end async hardening.
- Packaging/release proof is incomplete:
  clean-machine and long-session confidence is not yet documented as complete.

## Quality Signals

- Backend and frontend currently build without hard errors.
- `cargo check` is currently clean in this workspace (no warnings).
- `ui` production build is currently clean in this workspace.
- Desktop smoke checks currently pass via `scripts/smoke_desktop.ps1`.

## Documentation Status

- This file is canonical runtime truth for this repo.
- Earlier Lyra-centric references (`lyra-core`, legacy path contracts) are stale and have been superseded by current Cassette paths.
