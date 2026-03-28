# Cassette Project State

Last updated: 2026-03-28

## Architecture

- **Desktop shell**: Tauri 2.10.3 (`src-tauri/`)
- **Frontend**: SvelteKit (`ui/`)
- **Core domain**: Rust workspace (`crates/cassette-core/`)
- **Database**: SQLite via rusqlite (runtime) + sqlx (custodian subsystem)
- **Active DB location**: Tauri app-data directory (`cassette.db`)
- **Repo-local DB files**: Root-level `*.db` files in this workspace are local artifacts for tests, probes, or inspection; do not assume they are the live desktop runtime database

## What Works Today

### Library Management
- Library root scanning with recursive audio file discovery
- Track metadata extraction (artist, album, title, track/disc number, year, duration, sample rate, bit depth, bitrate, format, file size)
- Cover art path detection
- Search across tracks by title/artist/album
- Play count and skip count tracking
- Library organization (move files to Artist/Album/NN-Title structure)

### Playback
- Audio playback via Symphonia decode + cpal output
- Queue management (add, remove, reorder, clear)
- Playlist CRUD (create, list, add tracks, remove tracks, reorder)
- Now-playing context from Last.fm (artist bio, album info)
- Synced/plain lyrics from LRCLIB

### Acquisition Pipeline (Director Engine)
- Two-pass waterfall orchestration with per-provider semaphores
- 7 acquisition strategies (Standard, HighQualityOnly, ObscureFallbackHeavy, SingleTrackPriority, DiscographyBatch, RedownloadReplaceIfBetter, MetadataRepairOnly)
- 6-factor candidate scoring (metadata confidence, duration match, codec quality, provider trust, validation result, file size)
- Task-local cancellation via `CancellationToken` registry, with batch-wide cancel reserved for shutdown
- Symphonia-based audio validation (format probing, magic bytes, duration extraction)
- Post-acquisition metadata tagging via Lofty (artist, album, title, track#, disc#, year, cover art)
- Atomic finalization with dedup policy (KeepExisting or ReplaceIfBetter)
- Per-task temp directories with stale recovery and quarantine
- Retry with linear backoff (configurable max attempts and base delay)
- Search-result caching in the director waterfall
- Provider health monitoring with skip-on-down behavior
- Broadcast event channels for progress, results, and provider health
- Deezer full-track acquisition is live-proven on this machine as of 2026-03-27 via `provider_acquire_probe_cli`

### Providers (7 active)

| Provider | Trust Rank | Capabilities | Status |
|----------|------------|-------------|--------|
| Local Archive | 0 | Filesystem walk + direct copy, batch support | Proven Working |
| Deezer | 5 | Search + acquire with Blowfish CBC decryption (FLAC/320/128) | Implemented |
| Qobuz | 10 | MD5-signed session, search + acquire (lossless) | Implemented |
| slskd/Soulseek | 10 | P2P search with queue recovery, transfer polling + filesystem fallback | Implemented |
| Usenet | 30 | NZBgeek search + SABnzbd execution, filesystem polling | Implemented |
| yt-dlp | 50 | Subprocess fallback, ytsearch1 + scsearch1 | Proven Working |
| Real-Debrid | 80 | TPB search + torrent resolution + 7z extraction | Implemented |

### Metadata Services

| Service | Usage | Auth |
|---------|-------|------|
| MusicBrainz | Release search, parent album lookup, track listing, tag fixes | None (User-Agent) |
| Last.fm | Artist/album context for now-playing | Public API key |
| LRCLIB | Synced/plain lyrics lookup | None |
| Spotify | History import, search, discography seeds | Optional OAuth |

### Data Pipeline
- Spotify play history import from external SQLite DB
- Missing album detection (Spotify albums not in local library)
- Album-level batch download submission
- Album and Spotify-missing queues now expand releases into per-track `TrackTask` submissions via MusicBrainz release tracklists instead of treating the album title as a single-track query
- Director task result persistence to `director_task_history` table
- Terminal history now retains the original `TrackTask` request payload and strategy for failed/cancelled/finalized results
- Pending director task persistence in `director_pending_tasks` for startup recovery
- Startup recovery filters stale pending rows against newer terminal history before resubmission
- Request-signature persistence now threads through pending tasks, terminal history, candidate sets, and provider memory
- Full candidate-set persistence now captures scored, rejected, and selected candidates in `director_candidate_sets` and `director_candidate_items`
- Provider search outcomes and normalized negative-result memory now persist in `director_provider_searches`, `director_provider_attempts`, and `director_provider_memory`

## Active Runtime Database Schema (14 tables)

| Table | Purpose |
|-------|---------|
| `library_roots` | Configured scan directories |
| `tracks` | Library track metadata + play stats |
| `queue_items` | Current playback queue |
| `settings` | Key-value app settings |
| `playlists` | Playlist definitions |
| `playlist_items` | Playlist track membership |
| `spotify_album_history` | Imported Spotify listening data |
| `director_task_history` | Completed acquisition results with provenance plus original request payload and request signature |
| `director_pending_tasks` | In-flight acquisition tasks for deterministic startup recovery |
| `director_candidate_sets` | One persisted candidate-set envelope per terminal director task |
| `director_candidate_items` | All searched/acquired/rejected/selected candidates for a terminal task |
| `director_provider_searches` | Per-provider search outcomes including empty-result and error paths |
| `director_provider_attempts` | Normalized provider attempt trail per terminal task |
| `director_provider_memory` | Latest durable negative-result memory per request signature and provider |

Separate richer schemas also exist in the `library`, `librarian`, and `gatekeeper` subsystems. Those are part of the architecture-convergence story, not the current active Tauri runtime table set.

## Concurrency Model

- Global worker semaphore: configurable (default 12 concurrent tasks)
- Per-provider semaphores: configurable via `ProviderPolicy` (default 1 per provider)
- slskd global search semaphore: OnceLock<Semaphore(1)> — one search at a time
- Two-pass provider acquisition: Pass 1 non-blocking try_acquire, Pass 2 blocking on deferred
- Download concurrency: configurable (default 16 parallel downloads)
- MusicBrainz rate limiting via `governor` (1 request/second)

## Configuration

Settings resolved in priority order (highest wins):
1. SQLite database (user-saved via settings UI)
2. Environment variables (`.env` file)
3. Streamrip config (`%APPDATA%/streamrip/config.toml`) — auto-imports Qobuz/Deezer credentials
4. slskd config (`%LOCALAPPDATA%/slskd/slskd.yml`)
5. Hardcoded defaults

## Known Limitations

- Frontend still keeps `get_download_jobs` as a catch-up and resume fallback even though push events are now primary
- Dual schema: richer librarian/library model exists but isn't wired to active runtime
- `MetadataRepairOnly` strategy is stubbed
- Discogs/Last.fm enrichers are no-op stubs
- Bandcamp source is placeholder-only
- Pending-task startup recovery is now proven with a deterministic probe, but a full UI-driven kill/relaunch capture is still worth recording
- Download supervision is proven by automated tests and local probes; a full UI-driven proof pass is still worth capturing
- Candidate persistence now exists in the active runtime path, but the app still does not reuse that memory for pre-acquisition review, query-cache TTLs, or user override/exclusion decisions
- Usenet remains partially configured on this machine: `provider_probe_cli` reports `SKIP` because `nzbgeek_api_key` and/or `usenet_host` are missing

## Verification Snapshot

Verified on 2026-03-28:

- `cargo check --workspace` passes
- `cargo test --workspace` passes
- `npm run build` in `ui/` passes
- `.\scripts\smoke_desktop.ps1` passes
- Active runtime DB migrations now create and populate `director_candidate_sets`, `director_candidate_items`,
  `director_provider_searches`, `director_provider_attempts`, and `director_provider_memory`
- A new DB test proves the terminal save path persists both selected/rejected candidates and normalized provider-negative memory on the request signature
- `cargo run --manifest-path src-tauri/Cargo.toml --bin recovery_probe_cli` proves:
  - pending jobs are restored visibly before replay
  - stale cancelled rows are filtered instead of resurrected
  - a resumed task finalizes successfully after startup recovery
- `cargo run --bin provider_probe_cli` shows `OK` for `slskd`, `qobuz`, `deezer`, `spotify`, and `yt-dlp` on this machine
- `cargo run --bin provider_acquire_probe_cli -- --provider deezer` acquired a full-length FLAC for `Brand New - Sic Transit Gloria... Glory Fades` (`24,324,054` bytes, `186.49s`)

## Documentation

| Document | Purpose |
|----------|---------|
| `docs/PROJECT_STATE.md` | This file — current truthful state |
| `docs/WORKLIST.md` | Prioritized architecture convergence tasks |
| `docs/CAPABILITY_AUDIT.md` | Gap analysis from initial audit |
| `docs/TOOL_AND_SERVICE_REGISTRY.md` | Tool/service usage vs potential |
| `docs/reference/` | 19 code-traced per-component reference docs |
| `docs/ARCHITECTURAL_RECOMMENDATIONS.md` | Architecture convergence recommendations |
| `docs/INTEGRATION_GAPS_AND_OPPORTUNITIES.md` | Integration gap analysis |
| `docs/REQUEST_CAPABILITY_MATRIX.md` | What the system can/cannot do today |
| `docs/CACHE_PROVENANCE_STRATEGY.md` | Cache and provenance persistence strategy |
