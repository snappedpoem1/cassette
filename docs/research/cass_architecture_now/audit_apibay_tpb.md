# Audit: apibay / The Pirate Bay Search Surface

## Workspace Signal

- Active torrent-search upstream in `director/providers/real_debrid.rs` and `torrent_album_cli.rs`.

## Complete Technical Blueprint

- Core utility: public torrent result search.
- Auth flow: none in Cassette's current usage.
- Webhooks/events: none.
- Role in Cassette: brittle public-search adapter feeding a stronger resolver layer.

## Autonomous Suggestions

- Treat results as disposable search evidence only.
- Persist the search query, selected infohash, and result title, then hand off to Jackett or Real-Debrid-aware logic.
- Replace direct public-search dependence with Jackett where possible.

## Critical Failings

- It is the weakest part of the torrent lane.
- Search quality and availability are outside Cassette's control.
- Sovereignty improves when Cassette reduces dependence on this surface, not when it leans harder into it.

## Sources

- Public-doc gap note: this surface is operationally usable but not a strong documented developer platform.
