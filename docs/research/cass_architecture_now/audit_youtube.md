# Audit: YouTube

## Workspace Signal

- Desired-source adapter in `director/sources/youtube.rs`.
- Active fallback surface through yt-dlp.

## Complete Technical Blueprint

- Core utility: discovery and media hosting surface, not a clean music-library identity graph.
- Auth flow: official Data API uses API key/OAuth; Cassette's extraction lane depends on yt-dlp rather than the official API.
- Webhooks/events: none that fit Cassette's current architecture.
- Quota reality: official APIs are quota-driven and video-centric.

## Autonomous Suggestions

- Persist original video IDs/URLs as source evidence only.
- Keep YouTube-derived metadata quarantined from canonical release identity until MBID or fingerprint confirmation.
- Add an explicit "community/unverified source" flag for AI and UI ranking.

## Critical Failings

- Video-first metadata is noisy for album/release work.
- Official quota and unofficial extraction are two different failure planes.
- YouTube should help fill gaps, not define your library.

## Sources

- https://developers.google.com/youtube/v3/getting-started
- https://github.com/yt-dlp/yt-dlp
