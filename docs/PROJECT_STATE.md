# Cassette Project State

Last updated: 2026-03-27

## Architecture

- **Desktop shell**: Tauri 2.10.3 (`src-tauri/`)
- **Frontend**: SvelteKit (`ui/`)
- **Core domain**: Rust workspace (`crates/cassette-core/`)
- **Database**: SQLite via rusqlite (runtime) + sqlx (custodian subsystem)
- **Active DB location**: Tauri app-data directory (`cassette.db`)

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
- Symphonia-based audio validation (format probing, magic bytes, duration extraction)
- Post-acquisition metadata tagging via Lofty (artist, album, title, track#, disc#, year, cover art)
- Atomic finalization with dedup policy (KeepExisting or ReplaceIfBetter)
- Per-task temp directories with stale recovery and quarantine
- Retry with linear backoff (configurable max attempts and base delay)
- Broadcast event channel for real-time progress tracking

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
- Director task result persistence to `director_task_history` table

## Database Schema (9 tables)

| Table | Purpose |
|-------|---------|
| `library_roots` | Configured scan directories |
| `tracks` | Library track metadata + play stats |
| `queue_items` | Current playback queue |
| `settings` | Key-value app settings |
| `playlists` | Playlist definitions |
| `playlist_items` | Playlist track membership |
| `spotify_album_history` | Imported Spotify listening data |
| `director_task_history` | Completed acquisition results with provenance |

## Concurrency Model

- Global worker semaphore: configurable (default 12 concurrent tasks)
- Per-provider semaphores: configurable via `ProviderPolicy` (default 1 per provider)
- slskd global search semaphore: OnceLock<Semaphore(1)> — one search at a time
- Two-pass provider acquisition: Pass 1 non-blocking try_acquire, Pass 2 blocking on deferred
- Download concurrency: configurable (default 16 parallel downloads)

## Configuration

Settings resolved in priority order (highest wins):
1. SQLite database (user-saved via settings UI)
2. Environment variables (`.env` file)
3. Streamrip config (`%APPDATA%/streamrip/config.toml`) — auto-imports Qobuz/Deezer credentials
4. slskd config (`%LOCALAPPDATA%/slskd/slskd.yml`)
5. Hardcoded defaults

## Known Limitations

- No cancellation support for in-flight tasks (must wait for timeouts)
- Frontend polls for download status instead of receiving push events
- Hand-rolled rate limiting (fixed sleep) instead of proper token bucket
- No search result caching across provider fallthrough
- No provider health monitoring (discovers dead providers only on timeout)
- No crash recovery for in-flight tasks (only completed results persisted)
- Dual schema: richer librarian/library model exists but isn't wired to active runtime
- `MetadataRepairOnly` strategy is stubbed
- Discogs/Last.fm enrichers are no-op stubs
- Bandcamp source is placeholder-only
- `cargo test` has failures; `cargo check` has warnings

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
