# Audit: SoundCloud

## Workspace Signal

- Reached indirectly through yt-dlp `scsearch1` behavior.
- Not a first-class runtime provider module.

## Complete Technical Blueprint

- Core utility: discovery and edge-case audio availability.
- Auth flow: official API requires OAuth-centered app usage.
- Webhooks/events: not relevant to current Cassette runtime.
- Rate reality: official stream access and API usage are constrained; Cassette currently relies on yt-dlp instead.

## Autonomous Suggestions

- Persist SoundCloud URLs/IDs only as source breadcrumbs.
- Keep acquisition provenance and validation outcomes because metadata quality is uneven.
- Rank SoundCloud below canonical stores and below fingerprint-confirmed local truth.

## Critical Failings

- It is not a sovereign identity system.
- Official API access is narrower than the public site experience suggests.
- SoundCloud is ideal for "found it somewhere," not "this is the canonical release."

## Sources

- https://developers.soundcloud.com/docs/api/introduction
- https://developers.soundcloud.com/docs/api/rate-limits.html
