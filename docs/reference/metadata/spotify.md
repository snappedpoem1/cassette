# Spotify
> Listening history import from export files and album search/discography via Spotify Web API
**Status:** Proven Working (history import), Proven Working (search/discography)
**Code:** `src-tauri/src/commands/import.rs`, `crates/cassette-core/src/sources.rs`

## What It Does
Two distinct integration paths:

### 1. History Import (file-based, no API)
`parse_spotify_history()` reads Spotify Extended Streaming History JSON files (`Streaming_History_Audio_*.json`). Aggregates listening data by album: total listen time, play count, skip count. Saves results to the `spotify_album_history` database table. `queue_spotify_albums()` queries for albums not yet in the library and queues them for download.

### 2. Search & Discography (API-based)
`spotify_search()` and `spotify_discography()` in `sources.rs` query the Spotify Web API for album discovery. Uses OAuth2 client credentials flow for authentication.

## External Dependencies
- **reqwest** HTTP client
- **Spotify Web API** at `https://api.spotify.com/v1`
- **Spotify OAuth** at `https://accounts.spotify.com/api/token`
- **SQLite** (via cassette-core db) for history storage

## Authentication & Credentials
- **History import:** No API credentials needed; reads local JSON export files
- **Search/Discography:** OAuth2 client credentials flow
  - `spotify_client_id` (optional env var)
  - `spotify_client_secret` (optional env var)
  - `spotify_access_token` (optional env var, for pre-authenticated use)
  - Token endpoint: `POST https://accounts.spotify.com/api/token`
  - `spotify_bearer_token()` handles token acquisition

## Data Flow
### History Import
1. User provides path to Spotify export directory
2. `parse_spotify_history()` reads all `Streaming_History_Audio_*.json` files
3. Aggregates by album: listen time, play count, skip count
4. Writes to `spotify_album_history` table
5. `queue_spotify_albums()` diffs against existing library and queues missing albums for download

### Search/Discography
1. `spotify_bearer_token()` acquires OAuth2 token via client credentials
2. `spotify_search()` calls `GET https://api.spotify.com/v1/search` with query
3. `spotify_discography()` fetches artist's full album catalog

## Capabilities
- Parse and aggregate Spotify Extended Streaming History exports
- Identify listening patterns: total time, play count, skip count per album
- Automatically queue unowned frequently-played albums for download
- Search Spotify catalog by query string
- Retrieve full artist discography

## Limitations & Known Issues
- History import requires manual Spotify data export (GDPR request, not real-time)
- All three credential env vars are optional, meaning search silently fails without them
- No token refresh logic visible; tokens may expire during long operations
- No pagination handling documented for large discographies
- History aggregation is album-level only; no track-level analytics

## Untapped Potential
The Spotify Web API supports far more than search and discography:
- Audio features and audio analysis (tempo, key, energy, danceability per track)
- Recommendation engine (seed artists/tracks/genres)
- User library management (saved albums, saved tracks)
- Playlist read/write (create, modify, read collaborative playlists)
- Follow/unfollow artists
- Player control (play, pause, seek, queue)
- New releases and browse categories
- Market availability data

## Code Map
| Symbol | Location | Purpose |
|---|---|---|
| `parse_spotify_history()` | `src-tauri/src/commands/import.rs` | Read + aggregate Spotify export JSON |
| `queue_spotify_albums()` | `src-tauri/src/commands/import.rs` | Diff library, queue missing albums |
| `spotify_search()` | `crates/cassette-core/src/sources.rs` | Spotify catalog search |
| `spotify_discography()` | `crates/cassette-core/src/sources.rs` | Artist discography fetch |
| `spotify_bearer_token()` | `crates/cassette-core/src/sources.rs` | OAuth2 client credentials token |
| `spotify_album_history` | SQLite table | Aggregated listening history |
