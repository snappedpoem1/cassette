# Audit: LRCLIB

## Workspace Signal

- Active lyrics source in `src-tauri/src/commands/player.rs`.

## Complete Technical Blueprint

- Core utility: synced and plain lyrics lookup.
- Auth flow: none in Cassette's current usage.
- Webhooks/events: none.
- Public-doc reality: formal official documentation is lighter than the large vendor APIs; community wrappers document much of the surface.

## Autonomous Suggestions

- Cache by a compound key: normalized artist/title/album plus duration bucket and future fingerprint.
- Store lyric source, sync confidence, and fetch timestamp.
- Add negative-result caching so lyric misses stop hammering the same query.

## Critical Failings

- Community-operated surfaces can be load-sensitive and contract-light.
- Text search alone is fragile for remasters, edits, and live versions.
- If Cassette does not bind lyrics to stronger identity, lyric reuse will stay noisy.

## Sources

- https://lrclib.net/
- https://github.com/notigorwastaken/lrclib-api
