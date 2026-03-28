# MusicBrainz
> Album/track metadata lookup and tag correction via the MusicBrainz open database
**Status:** Proven Working
**Code:** `crates/cassette-core/src/metadata.rs`

## What It Does
Provides three core operations for music metadata resolution:
1. **search_release(artist, album):** Searches for album releases matching an artist+album pair. Returns up to 5 candidates as `Vec<MbRelease>`.
2. **find_parent_album(artist, track_title):** Given a single track, walks recording results through releases and release-groups to find the parent album. Prefers Album/EP primary type over Single/Compilation. Returns `Option<MbRelease>`.
3. **get_release_tracks(release_id):** Fetches full tracklist for a known release. Returns `MbReleaseWithTracks`.

Additionally, `propose_tag_fixes(artist, album, local_tracks)` searches for a release, fetches its tracks, matches local tracks by track_number+disc_number (fallback: position index), and proposes fixes for title, artist, album, year, and track_number fields. `apply_tag_fix(fix)` writes proposed changes to files using lofty.

## External Dependencies
- **reqwest** HTTP client with 10-second timeout
- **MusicBrainz API** at `https://musicbrainz.org/ws/2`
- **lofty** crate for writing tag corrections to audio files

## Authentication & Credentials
No authentication required. Identifies via User-Agent header: `"CassettePlayer/0.1 (https://github.com/cassette-music)"`.

## Data Flow
1. `MetadataService` struct initialized with reqwest client and User-Agent
2. Hardcoded 1100ms sleep enforced before `find_parent_album` and `get_release_tracks` calls (rate limiting)
3. Search: `GET /release?query=artist:"{artist}" AND release:"{album}"&fmt=json&limit=5`
4. Recording lookup: `GET /recording?query=recording:"{title}" AND artist:"{artist}"&fmt=json&limit=5&inc=releases+release-groups`
5. Release detail: `GET /release/{id}?inc=recordings+artist-credits&fmt=json`
6. Tag fix application: lofty opens file, sets fields, calls `save_to_path` with `WriteOptions::default()`

## Capabilities
- Album search by artist + album name
- Reverse track-to-album resolution with type preference (Album/EP over Single/Compilation)
- Full tracklist retrieval with artist credits
- Automated tag fix proposal with field-level matching
- Tag writing for: title, artist, album, year, track_number, album_artist

**Data models:**
- `MbRelease`: id, title, artist, year, track_count, release_group_type, label, country, barcode
- `MbTrack`: title, artist, track_number, disc_number, duration_ms

## Limitations & Known Issues
- Rate limiting is a hardcoded 1100ms sleep, not adaptive or token-bucket based
- Search limited to 5 results per query
- Track matching falls back to position index when track_number+disc_number don't match, which can misalign on multi-disc or bonus-track releases
- No retry logic on failed requests
- No caching of API responses

## Untapped Potential
MusicBrainz supports significantly more than what Cassette currently uses:
- Release-group search for edition disambiguation
- Area/place entities
- Work entities (compositions, not just recordings)
- ISRC lookups
- AcoustID fingerprint matching
- Cover Art Archive links (coverartarchive.org)
- Relationship queries (artist-to-artist, recording-to-work, etc.)
- Full Lucene query syntax for advanced search
- Label and artist browsing endpoints
- Annotation API
- Collection management

## Code Map
| Symbol | Location | Purpose |
|---|---|---|
| `MetadataService` | `crates/cassette-core/src/metadata.rs` | Main struct: reqwest client + User-Agent |
| `search_release()` | `crates/cassette-core/src/metadata.rs` | Album search by artist+album |
| `find_parent_album()` | `crates/cassette-core/src/metadata.rs` | Track-to-album reverse lookup |
| `get_release_tracks()` | `crates/cassette-core/src/metadata.rs` | Full tracklist for a release |
| `propose_tag_fixes()` | `crates/cassette-core/src/metadata.rs` | Diff local tags against MusicBrainz data |
| `apply_tag_fix()` | `crates/cassette-core/src/metadata.rs` | Write tag corrections via lofty |
| `MbRelease` | `crates/cassette-core/src/metadata.rs` | Release data model |
| `MbTrack` | `crates/cassette-core/src/metadata.rs` | Track data model |
