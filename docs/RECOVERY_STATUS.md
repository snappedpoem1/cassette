# Recovery Status

Last updated: March 20, 2026

This repo was partially reconstructed after an accidental deletion of `C:\Cassette Music`.

## What was restored from prior captured context

- top-level workspace manifest
- `.gitignore`
- Tauri manifest and host entrypoints
- core crate manifest and module layout
- UI package manifest and route/component/store layout
- workspace file
- local recovery docs from `C:\chatgpt\docs`
- machine-level Cassette app DB at `%APPDATA%\dev.cassette.app\cassette.db`
- machine-level provider config from `%APPDATA%\streamrip\config.toml`
- machine-level Soulseek credentials from `%LOCALAPPDATA%\slskd\slskd.yml`
- rebuilt local `.env` using surviving machine config

## What is still missing

- most original Rust implementation files
- most Svelte component/page source
- some original feature-complete Rust/Tauri downloader logic
- portions of the original scripts and binary assets
- any fuller source backup beyond the recovered docs

## Best surviving external context

- `C:\chatgpt\docs\CASSETTE_SESSION_LOG.md`
- `C:\chatgpt\docs\PROJECT_STATE.md`
- `C:\chatgpt\docs\WORKLIST.md`
- `C:\Users\Admin\.claude\projects\C--Cassette-Music\memory\project_cassette_lyra.md`

## Recovery findings

- No fuller repo restore was found in the documented `C:\chatgpt\docs` and Claude memory locations.
- The machine already has the real Cassette music root configured as `A:\music`.
- The existing app DB already contains a live library root entry for `A:\music`.
- `streamrip` and `slskd` config files survived outside the repo and can seed bootstrap defaults.
- `slskd` config survived and the official Windows x64 daemon binary has now been restored into `binaries\slskd\`.
- Download orchestration is no longer shell-only: Tauri now runs a serial async worker that resolves provider metadata through Qobuz, Deezer, and Spotify, then hands off to the local `slskd` daemon through its session-token REST flow.
- The current `slskd` handoff path uses `POST /api/v0/session`, `POST /api/v0/searches`, `GET /api/v0/searches/{id}?includeResponses=true`, and `POST /api/v0/transfers/downloads/{username}`.
- Live probe on this machine confirmed that broad fallback queries such as `Brand New Sic Transit Gloria Glory Fades` and `Brand New Deja Entendu` return populated `slskd` search responses, while over-specific triple queries can still miss.
- The handoff contract is now confirmed against the daemon's OpenAPI/source: search detail should use `GET /api/v0/searches/{id}/responses`, and download enqueue expects an array of `QueueDownloadRequest` objects with both `filename` and `size`.
- Live transfer probes now get past auth and JSON-shape validation; remaining failures are remote-peer acceptance issues such as `Transfer rejected: File not shared`, which points to candidate/path fidelity work rather than a broken local handoff contract.
- The slskd handoff path now retries across a ranked, deduplicated candidate pool, skips obvious non-audio files, and records detailed per-candidate rejection diagnostics so failures are inspectable instead of silent.
- Acquisition queue orchestration now supports multiple scopes in Tauri: single song/album, artist/discography with release-type rules (album-only by default, optional singles/EPs/compilations), and database-seeded album queue building from the local library index.

## Immediate next recovery options

1. Continue rebuilding missing source files from surviving docs and machine-level config.
2. Reconnect deeper downloader parity, search, and proof flows on top of the restored daemon/runtime baseline.
3. Harden the `slskd` worker with transfer reconciliation, richer candidate heuristics, and a direct end-to-end proof from the desktop shell.
