# Last.fm
> Artist bios, listener stats, album art, and tag data from Last.fm's read-only API
**Status:** Proven Working (player context), Stub (enricher)
**Code:** `src-tauri/src/commands/player.rs`, `crates/cassette-core/src/librarian/enrich/lastfm.rs`

## What It Does
Fetches contextual metadata for the currently playing track:
1. **artist.getinfo:** Retrieves artist biography summary, listener count, and genre tags.
2. **album.getinfo:** Retrieves album description and cover art URL (selects the largest available size).

Results are surfaced through the `get_now_playing_context()` Tauri command to enrich the player UI.

A separate stub enricher exists at `librarian/enrich/lastfm.rs` with an empty `enrich()` implementation that is never called.

## External Dependencies
- **reqwest** (via Tauri command context)
- **Last.fm API** at `https://ws.audioscrobbler.com/2.0/`

## Authentication & Credentials
- Hardcoded read-only API key: `b25b959554ed76058ac220b7b2e0a026`
- This is a public read-only key; no user authentication or session tokens are needed
- No OAuth flow, no write access

## Data Flow
1. `get_now_playing_context()` fires during playback
2. `GET` request to `https://ws.audioscrobbler.com/2.0/` with params: `method=artist.getinfo`, `artist={name}`, `api_key`, `format=json`
3. Response parsed: `/artist/bio/summary` (trailing HTML `<a href>` suffix stripped), `/artist/stats/listeners`, `/artist/tags/tag[]/name`
4. `GET` request with params: `method=album.getinfo`, `artist`, `album`, `api_key`, `format=json`
5. Response parsed: `/album/wiki/summary` (HTML suffix stripped), `/album/image` (last non-empty `#text` entry selected as largest size)

## Capabilities
- Artist biography summaries (HTML-cleaned)
- Listener count statistics
- Genre/tag lists per artist
- Album wiki summaries
- Album cover art URLs (largest available resolution)

## Limitations & Known Issues
- Implementation is inline in `player.rs`, not a reusable service
- No caching of responses; re-fetches on every playback context request
- HTML stripping is a simple suffix trim, not a full sanitizer
- Stub enricher in `librarian/enrich/lastfm.rs` is dead code
- API key is hardcoded rather than configurable

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
| `enrich()` (stub) | `crates/cassette-core/src/librarian/enrich/lastfm.rs` | Empty enricher, never called |
