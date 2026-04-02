# Audit: Last.fm

## Workspace Signal

- Active in `src-tauri/src/commands/player.rs` for artist and album context.
- Separate enricher exists as a stub.

## Complete Technical Blueprint

- Core utility: tags, bios/wiki summaries, related artists, popularity-style context.
- Auth flow: API key.
- Webhooks/events: none relevant to current Cassette use.
- Rate/terms reality: good contextual layer, not canonical release truth.

## Autonomous Suggestions

- Cache artist and album summaries with explicit TTL and source attribution.
- Store tags separately from canonical genre so user-facing discovery does not contaminate identity truth.
- Use it as "taste context" for the future AI layer, not as metadata authority.

## Critical Failings

- It can change or limit API behavior on its own terms.
- It is not trustworthy enough for release identity or track sequencing.
- Wiki/tag data are descriptive and crowd-shaped, not deterministic.

## Sources

- https://www.last.fm/api/show/artist.getInfo
- https://www.last.fm/api/show/album.getInfo
- https://www.last.fm/api/tos
