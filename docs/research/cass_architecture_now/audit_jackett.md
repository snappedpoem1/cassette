# Audit: Jackett

## Workspace Signal

- Optional in `src-tauri/src/bin/torrent_album_cli.rs`.
- URL and API key have persisted settings, but the main Director runtime does not use it yet.

## Complete Technical Blueprint

- Core utility: broker many torrent indexers behind Torznab-style search.
- Auth flow: API key against the local Jackett service.
- Webhooks/events: not the primary model; search and capability polling are the normal fit.
- Key value: reduces per-indexer scraping logic inside Cassette.

## Autonomous Suggestions

- Cache indexer capability snapshots and normalize which search fields are actually supported.
- Persist the originating indexer name and Torznab payload for every accepted candidate.
- Move torrent search into a dedicated adapter layer so Cassette can swap Jackett/apibay without rewriting queue logic.

## Critical Failings

- Torznab normalization is helpful, not magical; upstream indexers still vary wildly.
- Jackett only brokers search. It does not solve torrent resolution, import, or metadata identity.
- If you mix search, resolve, and import in one step, failures become hard to reconstruct.

## Sources

- https://github.com/Jackett/Jackett
- https://github.com/Jackett/Jackett/wiki/Jackett-API
