# Audit: Symphonia

## Workspace Signal

- Active dependency in `cassette-core`.
- Used for playback decode and validation/probing.

## Complete Technical Blueprint

- Core utility: audio probe, codec/container decode, duration extraction.
- Auth flow: none.
- Webhooks/events: none.
- Role in Cassette: local truth validator, not a remote metadata source.

## Autonomous Suggestions

- Persist probe outcomes and container/codec evidence beside every ingest/finalization event.
- Use Symphonia-derived duration and readability as hard facts in candidate scoring and duplicate detection.
- Add a normalized media-tech table so playback, validation, and AI context share one representation.

## Critical Failings

- It cannot write tags or give you release identity.
- If its evidence stays trapped in logs and not tables, you waste one of the cleanest local truth surfaces in the stack.

## Sources

- https://docs.rs/symphonia/latest/symphonia/
