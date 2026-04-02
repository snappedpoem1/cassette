# Audit: MusicBrainz

## Workspace Signal

- Active in `crates/cassette-core/src/metadata.rs`.
- Used for release search, parent-album lookup, and release tracklists.

## Complete Technical Blueprint

- Core utility: canonical music identity, release-group/release/recording relationships, label/country/barcode metadata, tracklists.
- Current Cassette auth flow: none. Cassette uses a custom User-Agent and local rate limiter.
- Rate model: MusicBrainz explicitly asks clients to respect conservative request rates; Cassette already enforces 1 request/sec.
- Webhooks/events: none. This is a read-oriented metadata authority, not an event source.

## Autonomous Suggestions

- Persist `artist_mbid`, `release_group_mbid`, `release_mbid`, and `recording_mbid` as first-class local keys instead of optional side notes.
- Cache full release JSON and tracklists keyed by MBID, not just by search text.
- Split "identity resolution" from "tag writing" so all providers feed the same MBID graph before acquisition finalization.

## Critical Failings

- It will not solve audio acquisition, rights, or popularity by itself.
- Search is text-heavy and can return multiple plausible releases; without local tie-break rules, sovereignty degrades into repeated guessing.
- If you do not persist MBIDs locally, MusicBrainz becomes a lookup tax instead of a governing spine.

## Sources

- https://musicbrainz.org/doc/MusicBrainz_API
- https://musicbrainz.org/doc/MusicBrainz_API/Search
- https://musicbrainz.org/doc/MusicBrainz_API/Rate_Limiting
- https://musicbrainz.org/doc/Release_Group
