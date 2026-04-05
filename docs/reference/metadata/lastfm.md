# Last.fm
> Artist bios, listener stats, album art, and tag data from Last.fm's read-only API
**Status:** Implemented but Unverified
**Code:** `src-tauri/src/commands/player.rs`, `crates/cassette-core/src/librarian/enrich/lastfm.rs`

## What It Does
Fetches contextual metadata for the currently playing track:
1. **artist.getinfo:** Retrieves artist biography summary, listener count, and genre tags.
2. **album.getinfo:** Retrieves album description and cover art URL (selects the largest available size).
3. **track.getinfo:** Retrieves track duration for librarian enrichment.
4. **user.getrecenttracks:** Syncs play history for a configured Last.fm username.

Results are surfaced through the `get_now_playing_context()` Tauri command to enrich the player UI.
The `sync_lastfm_history` command records play events and artist/song rollups in local runtime tables.

## External Dependencies
- **reqwest** (via Tauri command context)
- **Last.fm API** at `https://ws.audioscrobbler.com/2.0/`

## Authentication & Credentials
- Runtime API key from provider settings (`lastfm_api_key`)
- Optional username from settings (`lastfm_username`) for history sync
- This integration currently uses read-only API methods; no session auth flow is used
- No OAuth flow, no write access

## Data Flow
1. `get_now_playing_context()` fires during playback
2. `GET` request to `https://ws.audioscrobbler.com/2.0/` with params: `method=artist.getinfo`, `artist={name}`, `api_key`, `format=json`
3. Response parsed: `/artist/bio/summary` (trailing HTML `<a href>` suffix stripped), `/artist/stats/listeners`, `/artist/tags/tag[]/name`
4. `GET` request with params: `method=album.getinfo`, `artist`, `album`, `api_key`, `format=json`
5. Response parsed: `/album/wiki/summary` (HTML suffix stripped), `/album/image` (last non-empty `#text` entry selected as largest size)
6. `GET` request with params: `method=track.getinfo`, `artist`, `track`, `api_key`, `format=json` for duration enrichment
7. `GET` request with params: `method=user.getrecenttracks`, `user`, `api_key`, `format=json`, `limit`, `extended=0` for history sync
8. History sync writes to `play_history_events`, `artist_play_history`, and `song_play_history`

## Capabilities
- Artist biography summaries (HTML-cleaned)
- Listener count statistics
- Genre/tag lists per artist
- Album wiki summaries
- Album cover art URLs (largest available resolution)
- Track duration enrichment (`duration_ms`) through librarian enrichment
- Recent-track history sync into local play history tables

## Limitations & Known Issues
- No caching of responses; re-fetches on every playback context request
- HTML stripping is a simple suffix trim, not a full sanitizer
- Full end-to-end production proof for history sync and enrichment paths is still pending

## Untapped Potential
Last.fm's API offers extensive capabilities beyond current usage:
- **Scrobbling:** `track.scrobble`, `track.updateNowPlaying` for listen history tracking
- User library and listening history queries
- Similar artists and similar tracks recommendations
- Chart data (top tracks, top artists, top tags)
- Geo-based charts and data
- `user.getTopAlbums`, `user.getTopArtists` for personalized data
- `tag.getTopTracks` for genre-based discovery
- Track-level info including play counts and listener counts

## Code Map
| Symbol | Location | Purpose |
|---|---|---|
| `get_now_playing_context()` | `src-tauri/src/commands/player.rs` | Tauri command; fetches artist+album info inline |
| artist.getinfo call | `src-tauri/src/commands/player.rs` | Bio, listeners, tags |
| album.getinfo call | `src-tauri/src/commands/player.rs` | Album summary, cover art URL |
| `enrich()` | `crates/cassette-core/src/librarian/enrich/lastfm.rs` | Librarian track-duration enrichment via `track.getinfo` |
| `fetch_recent_tracks()` | `crates/cassette-core/src/librarian/enrich/lastfm.rs` | Recent-track pull via `user.getrecenttracks` |
| `sync_lastfm_history()` | `src-tauri/src/commands/player.rs` | Sync Last.fm history into local play-history tables |
