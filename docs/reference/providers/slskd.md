# slskd
> Peer-to-peer music acquisition through the slskd Soulseek daemon API with search queue management and filesystem-level transfer detection

**Status:** Active
**Code:** `crates/cassette-core/src/director/providers/slskd.rs`
**Provider ID:** `"slskd"`
**Trust Rank:** 10

## What It Does

slskd is the most complex provider (~864 lines), interfacing with a locally-hosted slskd daemon that bridges the Soulseek peer-to-peer network. It manages search lifecycle, transfer initiation, download polling, and connection health, all through the slskd REST API.

A global search semaphore (`OnceLock<Arc<Semaphore(1)>>`) ensures only one search runs at a time across all tasks. If the semaphore is already held, the provider returns `TemporaryOutage` immediately (non-blocking `try_acquire`), allowing the task to fall through to the next provider without waiting. Before each search, the provider runs queue recovery: if the search queue has more than 500 entries or stale in-progress searches older than 10 minutes, it bulk-deletes them (up to 16 concurrent DELETE requests).

Search tries up to 4 query variations, polling each for results over 30 seconds (10 polls at 3-second intervals). Results are ranked with a weighted scoring system that heavily favors FLAC format (+120 points) and penalizes long upload queues. The acquire phase initiates a transfer and then polls for completion using both the transfer API and direct filesystem scanning of configured scan roots as a fallback.

## External Dependencies

| Dependency | Type | Endpoint |
|---|---|---|
| slskd Search API | REST | `POST /api/v0/searches`, `GET /api/v0/searches/{id}/responses` |
| slskd Search Management | REST | `GET /api/v0/searches`, `DELETE /api/v0/searches/{id}` |
| slskd Transfer API | REST | `POST /api/v0/transfers/downloads/{username}` |
| slskd Transfer Status | REST | `GET /api/v0/transfers/downloads/{username}` |
| slskd Server API | REST | `GET /api/v0/server`, `PUT /api/v0/server` |
| Local filesystem | Disk | Scan roots for completed downloads |

## Authentication & Credentials

Authentication to the slskd daemon uses either HTTP basic auth (username + password) or an API key, depending on configuration. The connection config (`SlskdConnectionConfig`) carries url, username, password, and api_key fields. Soulseek network authentication is handled by the slskd daemon itself, not by this provider.

## Data Flow

### Search
1. Acquire global search semaphore (non-blocking `try_acquire`; returns TemporaryOutage if busy)
2. `maybe_recover_search_queue()` - GET `/api/v0/searches`. If >500 queued or stale in-progress (>10min), delete them (16 concurrent DELETEs)
3. For each query candidate `[build_query, artist+title, artist+album, artist]`:
   - `create_search_with_recovery()` - POST `/api/v0/searches` with `{"searchText": query}`
   - On "must be connected" error: `ensure_slskd_connected()` → PUT `/api/v0/server` to reconnect
   - Poll 10 times at 3-second intervals: GET `/api/v0/searches/{id}/responses`
   - `rank_slskd_candidates()`: artist terms +15pts, title terms +25pts, album terms +10pts, FLAC +120, MP3 +20, free upload slot +20, queue length penalty (-queue_length/2)
4. Fallback: `rank_slskd_candidates_from_history()` - search completed searches via GET `/api/v0/searches`
5. Return top 8 candidates sorted by score

### Acquire
1. Parse candidate_id as `"username::filename::size"`
2. POST `/api/v0/transfers/downloads/{username}` with `[{filename, size}]`
3. Poll 24 times at 5-second intervals (2 minutes total):
   - Primary: `fetch_slskd_user_transfers()` → `find_completed_transfer()` checks state for "completed"/"succeeded" or code 2
   - Fallback: filesystem scan of `scan_roots` via `walkdir` in `spawn_blocking`
4. `copy_to_temp` with 1KB minimum file size check

## Capabilities

- Lossless audio acquisition (FLAC preferred with +120 score bonus)
- Global search serialization preventing Soulseek rate limiting
- Automatic search queue cleanup (>500 entries or stale searches)
- Connection health monitoring with automatic reconnection
- Dual transfer detection: API polling + filesystem scanning
- History-based fallback search for previously completed results
- Audio format support: flac, mp3, m4a, aac, wav, ogg, opus, aiff, alac

## Configuration

| Setting | Source | Default | Description |
|---|---|---|---|
| `url` | SlskdConnectionConfig | None | Base URL of the slskd daemon |
| `username` | SlskdConnectionConfig | None | HTTP basic auth username |
| `password` | SlskdConnectionConfig | None | HTTP basic auth password |
| `api_key` | SlskdConnectionConfig | None | slskd API key (alternative to basic auth) |
| `scan_roots` | Provider config | None | Filesystem paths where slskd writes completed downloads |

## Limitations & Known Issues

- Only one search can run globally at a time due to the semaphore; concurrent tasks fall through to other providers
- 2-minute acquire timeout (24 polls x 5 seconds) may be too short for slow peers
- Search queue recovery threshold of 500 is hardcoded
- Filesystem fallback scan uses walkdir which can be slow on large directory trees
- Candidate ID format (`username::filename::size`) is string-parsed with split, fragile if filenames contain `::`
- Connection recovery polls 10 times at 2-second intervals; no exponential backoff
- No batch download support (`supports_batch: false`)

## Untapped Potential

The slskd API exposes several endpoints that are not currently used: user browsing (`/api/v0/users/{username}/browse`) could enable full library exploration of high-quality peers, room chat could be used for discovery, distributed search configuration could improve result quality, file share management could expose local shares, and the options/config endpoint could enable runtime tuning. Currently only search, transfer, and server endpoints are used.

## Code Map

| File | Role |
|---|---|
| `crates/cassette-core/src/director/providers/slskd.rs` | Full provider implementation: search queue management, multi-query search, candidate ranking, transfer initiation, dual-mode completion detection, connection management |
