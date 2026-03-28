# Qobuz
> Lossless music acquisition via the Qobuz streaming API with multi-query search and MD5-signed file URL generation

**Status:** Active
**Code:** `crates/cassette-core/src/director/providers/qobuz.rs`, `crates/cassette-core/src/sources.rs`
**Provider ID:** `"qobuz"`
**Trust Rank:** 10

## What It Does

Qobuz is a high-fidelity streaming provider that searches and acquires lossless audio through the Qobuz web API. It authenticates with email/password credentials, caches the session token behind a double-check `RwLock`, and automatically invalidates the session on 401/403 responses.

Search uses a multi-query strategy that fires the most specific query first (artist+album) and progressively broadens through build_query(artist, title, album), artist+title, and finally artist alone. Results are album-level candidates, deduplicated across query rounds. Confidence scoring starts at a base of 0.65, adds 0.20 for artist match and 0.10 for album match, clamped to a maximum of 0.95.

Acquisition fetches the full album metadata, locates the target track within the album's track listing (matching on task title, falling back to the first track), then generates a signed file URL using an MD5 signature scheme. The signed URL is downloaded and written to the temp directory.

## External Dependencies

| Dependency | Type | Endpoint |
|---|---|---|
| Qobuz Auth API | REST | `POST https://www.qobuz.com/api.json/0.2/user/login` |
| Qobuz Album API | REST | `GET https://www.qobuz.com/api.json/0.2/album/get` |
| Qobuz File URL API | REST | `GET https://www.qobuz.com/api.json/0.2/track/getFileUrl` |

## Authentication & Credentials

Authentication uses email + password posted to the Qobuz login endpoint via `qobuz_user_auth_token()` in `sources.rs`. The returned `user_auth_token` is cached in an `RwLock<Option<QobuzSessionCache>>` with double-check locking: the read lock checks for an existing session, and only if absent does the write lock acquire and re-check before performing the login request. On any 401 or 403 response, the cached session is invalidated, forcing re-authentication on the next request.

The app ID and app secret are required for API access. Multiple secrets can be provided as a CSV list; the provider tries each when generating file URL signatures.

## Data Flow

### Search
1. Authenticate (or reuse cached session)
2. Execute up to 4 queries in order: `[artist+album, build_query(artist, title, album), artist+title, artist]`
3. Each query calls `qobuz_search()` via `sources.rs`
4. Results are album-level candidates, deduplicated across queries
5. Confidence scored: base 0.65 + artist match 0.20 + album match 0.10, clamped to 0.95

### Acquire
1. `GET /api.json/0.2/album/get` with album_id, app_id, user_auth_token
2. Extract tracks from `/tracks/items`
3. Match task title against track list; fall back to first track
4. Generate signed file URL: format_id=27 (lossless), intent=stream
5. Signature: MD5 of `"trackgetFileUrlformat_id{format_id}intentstreamtrack_id{track_id}{timestamp}{secret}"`
6. Try multiple secrets if configured
7. Download file bytes
8. Write to temp as `"qobuz-{track_id}-{title}.{ext}"` (`.mp3` if URL contains `.mp3`, else `.flac`)

## Capabilities

- Lossless audio acquisition (FLAC, format_id=27)
- Multi-query search with progressive broadening
- Album-level candidate discovery
- Automatic session caching with invalidation on auth errors
- Multiple app secret rotation for URL signing resilience

## Configuration

| Setting | Source | Default | Description |
|---|---|---|---|
| `qobuz_app_id` | RemoteProviderConfig | None | Qobuz application ID |
| `qobuz_app_secret` | RemoteProviderConfig | None | Primary Qobuz app secret for URL signing |
| `qobuz_secrets` | RemoteProviderConfig | None | Comma-separated list of additional app secrets to try |
| `qobuz_email` | RemoteProviderConfig | None | Qobuz account email |
| `qobuz_password` | RemoteProviderConfig | None | Qobuz account password |

## Limitations & Known Issues

- Only acquires single tracks, not full albums (despite searching at album level)
- No batch download support (`supports_batch: false`)
- Confidence cap at 0.95 means it never reports full certainty
- Session invalidation is reactive (on 401/403), not proactive (no token expiry tracking)
- Extension detection is simplistic: checks for `.mp3` in URL string, defaults to `.flac`

## Untapped Potential

The Qobuz API supports significantly more than what is currently used. Album-level batch download would allow acquiring entire albums in one pass. Release metadata includes genre, label, catalog number, hi-res indicators, and goodies/booklets (digital liner notes). Artist pages, playlist endpoints, editorial recommendations, and purchase history are all available through the API but completely unused. These could enable richer metadata population, discovery features, and library-aware duplicate avoidance.

## Code Map

| File | Role |
|---|---|
| `crates/cassette-core/src/director/providers/qobuz.rs` | Provider implementation: search, acquire, session management, URL signing |
| `crates/cassette-core/src/sources.rs` | `qobuz_user_auth_token()` login, `qobuz_search()` search helper |
