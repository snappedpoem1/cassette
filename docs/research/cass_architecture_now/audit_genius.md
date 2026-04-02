# Audit: Genius

## Workspace Signal

- Token surface exists in settings and provider config.
- No active runtime integration found beyond configuration.

## Complete Technical Blueprint

- Core utility: song/artist metadata, annotations, and ecosystem visibility.
- Auth flow: token/OAuth-centered API patterns.
- Webhooks/events: not relevant to current Cassette needs.
- Public-doc reality: clean official lyric-text API guarantees are weak; the ecosystem often relies on scrape-adjacent clients.

## Autonomous Suggestions

- Treat Genius as annotation/context only.
- If Cassette ever touches Genius, persist only the small normalized facts it truly needs.
- Do not make Genius the primary lyric backbone; LRCLIB is a better synced-lyric fit.

## Critical Failings

- Lyric text availability is the sovereignty trap here.
- Rights and scraping friction make Genius poor as a durable local lyric source.
- A configured token without a deliberate integration plan is just attack surface.

## Sources

- Public-doc gap note: official public developer documentation is less directly durable than the surrounding client ecosystem.
- https://lyricsgenius.readthedocs.io/
