# Audit: iTunes Search API

## Workspace Signal

- Active fallback in `crates/cassette-core/src/metadata.rs`.

## Complete Technical Blueprint

- Core utility: album lookup and tracklist fallback when MusicBrainz resolution fails.
- Auth flow: none in Cassette's current usage.
- Webhooks/events: none.
- Role in Cassette: emergency metadata bridge, not canonical authority.

## Autonomous Suggestions

- Cache iTunes fallback responses with explicit "fallback-source" labeling.
- Never persist iTunes IDs as canonical IDs; treat them as temporary bridge aliases.

## Critical Failings

- It is convenient, not structurally rich.
- If Cassette leans on iTunes too heavily, it will quietly degrade from identity graph to storefront heuristics.

## Sources

- https://developer.apple.com/library/archive/documentation/AudioVideo/Conceptual/iTuneSearchAPI/index.html
