# Audit: Spotify

## Workspace Signal

- Active in `sources.rs`, import flows, runtime settings, and album-history backlog tables.
- Desired-state import logic is structurally Spotify-shaped even when the current sidecar rows are manual.

## Complete Technical Blueprint

- Core utility: search, artist albums, track and album IDs, import/history intent, playlist/desired-state bridge.
- Auth flow: client credentials for app-level search; richer user flows require OAuth.
- Webhooks/events: none. Polling / import ingestion is the practical model.
- Current volatility: Spotify has narrowed several API capabilities for newer apps and enforces rolling-window rate limits.

## Autonomous Suggestions

- Treat Spotify IDs as intent aliases, never canonical truth.
- Snapshot imported playlist/history payloads locally at ingest time so future API contractions do not erase provenance.
- Store Spotify-to-MBID-to-local-track mappings explicitly.

## Critical Failings

- Spotify does not give you sovereign ownership truth.
- Several historically useful endpoints or data lanes have narrowed for new applications.
- Audio availability, previews, recommendations, and derived analysis are policy-shaped surfaces, not stable archive primitives.

## Sources

- https://developer.spotify.com/documentation/web-api
- https://developer.spotify.com/documentation/web-api/reference/search
- https://developer.spotify.com/documentation/web-api/reference/get-an-artists-albums
- https://developer.spotify.com/documentation/web-api/concepts/rate-limits
- https://developer.spotify.com/blog/2024-11-27-changes-to-the-web-api
