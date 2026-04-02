# Audit: yt-dlp

## Workspace Signal

- Active fallback provider in `director/providers/ytdlp.rs`.
- Configured as a tool/binary surface, not a metadata authority.

## Complete Technical Blueprint

- Core utility: extractor-driven media retrieval across many upstream sites, including YouTube and SoundCloud.
- Auth flow: mostly cookies/session state depending on extractor; no single unified service auth model.
- Webhooks/events: none.
- Operational reality: extractor behavior changes with upstream site changes.

## Autonomous Suggestions

- Record extractor name, command line, source URL, media format, and postprocessing result for every finalized file.
- Cache negative extractor outcomes by source URL and version so repeated failures do not look mysterious.
- Treat yt-dlp as a low-trust last-resort backend only.

## Critical Failings

- It is structurally brittle because it chases other platforms.
- Metadata coming through yt-dlp is not a canonical release graph.
- A sovereign Cassette must never confuse "extractable" with "authoritative."

## Sources

- https://github.com/yt-dlp/yt-dlp
- https://github.com/yt-dlp/yt-dlp/wiki/Extractors
