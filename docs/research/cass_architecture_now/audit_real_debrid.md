# Audit: Real-Debrid

## Workspace Signal

- Active in `director/providers/real_debrid.rs`.
- Also used in `torrent_album_cli.rs`.

## Complete Technical Blueprint

- Core utility: magnet ingestion, torrent status, cached-availability checks, link unrestriction.
- Auth flow: bearer/API token.
- Webhooks/events: no native webhook story in Cassette's current usage; polling dominates.
- Important boundary: Real-Debrid is a resolver/execution plane, not a metadata authority.

## Autonomous Suggestions

- Persist infohash, torrent ID, selected file IDs, unrestrict URL, and cache-hit/miss state per request.
- Separate "search owner" from "resolver owner" in the schema so RD never silently becomes your search truth.
- Add a local resolved-archive manifest so repeated extractions are idempotent and auditable.

## Critical Failings

- RD can tell you whether a torrent resolves, not whether it is the right release.
- Account state, hoster state, and cache state are all external volatility.
- If Cassette stores only the final file path and not the RD resolution chain, the lane stops being reconstructible.

## Sources

- https://api.real-debrid.com/
- https://app.real-debrid.com/
