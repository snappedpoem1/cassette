# Audit: Lofty

## Workspace Signal

- Active dependency in `cassette-core`.
- Used in tag writing, gatekeeper, and metadata repair flows.

## Complete Technical Blueprint

- Core utility: tag read/write across common audio formats, picture/tag item handling.
- Auth flow: none.
- Webhooks/events: none.
- Role in Cassette: metadata mutation primitive after identity is resolved.

## Autonomous Suggestions

- Persist pre-write and post-write tag snapshots for meaningful repair operations.
- Separate "proposed fix" from "applied fix" as immutable audit rows.
- Use Lofty writes only after release identity or fingerprint confidence crosses a named threshold.

## Critical Failings

- Lofty can mutate files correctly while still writing the wrong truth if upstream identity is weak.
- Tag writing without evidence persistence is just silent mutation with better manners.

## Sources

- https://docs.rs/lofty/latest/lofty/
