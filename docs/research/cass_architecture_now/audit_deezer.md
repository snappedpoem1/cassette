# Audit: Deezer

## Workspace Signal

- Active provider in `director/providers/deezer.rs`.
- Metadata search in `sources.rs`.
- Private gateway/media token flow is present in code.

## Complete Technical Blueprint

- Core utility: public catalog search plus a private-media acquisition lane in Cassette's current runtime.
- Auth flow: public search can work unauthenticated; Cassette's acquisition path depends on `arl` session state and private token exchange.
- Webhooks/events: none.
- Public/private split: public metadata APIs are materially more stable than the private media path.

## Autonomous Suggestions

- Persist exact search payloads, track tokens, selected format, and failure reasons for every Deezer attempt.
- Keep Deezer metadata cached separately from Deezer acquisition state so search remains useful when media auth breaks.
- Use Deezer as an acquisition adapter behind a normalized candidate schema, not as truth.

## Critical Failings

- Cassette's strongest Deezer value depends on brittle unofficial/private behavior.
- Rights and quality availability vary per track and region.
- If Deezer changes private token or media URL behavior, your sovereign layer must fall back without identity loss.

## Sources

- https://developers.deezer.com/api
- Public-doc gap note: the acquisition path used by Cassette is not covered cleanly by stable public developer docs.
