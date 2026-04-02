# Audit: Cover Art Archive

## Workspace Signal

- Not wired in runtime code.
- Repeatedly implied by Cassette's MusicBrainz-first architecture.

## Complete Technical Blueprint

- Core utility: release-keyed artwork retrieval using MusicBrainz entities.
- Auth flow: none for ordinary fetches.
- Webhooks/events: none.
- Practical dependency: only becomes reliable after release identity is canonical.

## Autonomous Suggestions

- Store artwork records in a separate `release_artifacts` table keyed by `release_mbid`.
- Track source URL, checksum, fetch time, and size so artwork is auditable and replaceable.
- Defer art fetch until after release selection; never search for cover art on free text first.

## Critical Failings

- It is downstream of MBID quality. Bad release identity means bad art.
- It does not help with acquisition or edition disambiguation by itself.

## Sources

- https://musicbrainz.org/doc/Cover_Art_Archive/API
