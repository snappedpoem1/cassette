# LRCLIB
> Synchronized and plain-text lyrics lookup via the LRCLIB open API
**Status:** Proven Working
**Code:** `src-tauri/src/commands/player.rs`

## What It Does
Fetches lyrics for the currently playing track. Returns both plain text lyrics and LRC-format synchronized (timed) lyrics when available. Used as part of the now-playing context enrichment.

## External Dependencies
- **reqwest** (via Tauri command context)
- **LRCLIB API** at `https://lrclib.net/api`

## Authentication & Credentials
- No authentication required
- Identifies via User-Agent header: `"Cassette Music Player v0.1"`

## Data Flow
1. `get_now_playing_context()` fires during playback
2. `GET https://lrclib.net/api/get` with query params: `artist_name`, `track_name`, optional `album_name`
3. Response contains `plainLyrics` (plain text) and `syncedLyrics` (LRC timed format)
4. Returns `None` if both fields are empty or the request fails

## Capabilities
- Exact-match lyrics lookup by artist + track name
- Optional album name for disambiguation
- Plain text lyrics retrieval
- LRC synchronized/timed lyrics retrieval (line-by-line timestamps)

## Limitations & Known Issues
- Implementation is inline in `player.rs`, not a reusable service
- Only uses exact match endpoint; no fuzzy search fallback
- No caching; re-fetches on every playback context request
- Silently returns `None` on any error (no distinction between "not found" and "network failure")
- No retry logic

## Untapped Potential
LRCLIB offers additional endpoints beyond exact match:
- `GET /api/search` for fuzzy/partial lyrics search (useful when exact metadata doesn't match)
- `POST /api/publish` for contributing lyrics back to the community database
- `GET /api/get/{id}` for direct ID-based lookup (useful if IDs are cached from prior searches)

## Code Map
| Symbol | Location | Purpose |
|---|---|---|
| `get_now_playing_context()` | `src-tauri/src/commands/player.rs` | Tauri command; lyrics fetch is inline |
| LRCLIB GET call | `src-tauri/src/commands/player.rs` | Exact-match lyrics lookup |
