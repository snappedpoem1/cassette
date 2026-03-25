# Cassette Project State

Last audited: 2026-03-25

## Runtime Truth

- Desktop shell: Tauri 2 (`src-tauri`)
- Renderer: SvelteKit (`ui`)
- Core domain: Rust (`crates/cassette-core`)
- Local store: SQLite (WAL mode, in Tauri app data directory)
- Workspace is a git repository with a full commit history.

## Verified As Built

- Rust workspace compiles: `cargo check` passes, no warnings.
- `cargo test` passes for the Rust workspace.
- UI production build passes: `npm run build` in `ui/`.
- Library scanning, queue, playback, playlists, downloads, Spotify import, settings,
  and organizer command surfaces are implemented and wired through Tauri commands.
- Primary navigation: `Library`, `Downloads`, `Settings`.
  Advanced routes (`/playlists`, `/import`, `/tools`, `/artists`) exist and are functional.

## Current Gaps

- **Deezer full-track path is incomplete.** The provider is wired up but the ARL-decrypted
  full-track acquisition path is not proven end-to-end. Preview MP3 fallback is the current
  runtime behavior for Deezer.
- **Tidal is not implemented.** OAuth device flow has not been started.
- **Provider live-proof is incomplete.** Qobuz and Soulseek (slskd) paths exist but have not
  been formally proven on a clean machine. Failures are visible in the downloads dashboard but
  recovery behavior needs more coverage.
- **Async hardening is incomplete** in some acquisition and orchestration flows. Cancellation
  safety and temp/staging cleanup guarantees are not formally tested.
- **Packaging/release proof is incomplete.** Clean-machine install has not been documented or tested.
- **MetadataRepairOnly acquisition strategy is stubbed.** Flagged in `director/engine.rs`.
- **`downloader/` module vs `director/providers/` overlap.** Two partially parallel
  implementation paths exist for slskd, usenet, and other providers. The `director/providers/`
  path is the active one; `downloader/` contains earlier implementations that have not been
  fully removed or reconciled.

## Quality Signals

- `cargo check`: clean (no warnings)
- `cargo test`: passing
- `ui` production build: clean
- Desktop smoke checks pass via `scripts/smoke_desktop.ps1`

## Known Code Issues (tracked in TODO.md)

Issues confirmed and fixed in this session (2026-03-25):

- `replace_spotify_album_history` was not wrapped in a transaction — fixed.
- `prune_missing_tracks` issued individual deletes without a transaction — fixed.
- `Player::send()` silently dropped commands when the channel was full — now logs a warning.
- `decode_loop` and `decode_loop_seek` were ~200 lines of duplicated code — merged into one
  function with an `Option<f64>` seek parameter.
- `load_streamrip_config` used a hand-rolled line scanner instead of real TOML parsing — replaced
  with the `toml` crate. Added `toml = "0.8"` to `src-tauri/Cargo.toml`.
- Library page artist rows used `<a href="/artists">` — converted to proper navigation buttons.

## Documentation Status

- This file is canonical runtime truth for this repo.
- `RECOVERY_STATUS.md` records the history of the March 2026 reconstruction event.
  It is historical record, not current operating state.
- All doc internal links use relative paths from `docs/`.
