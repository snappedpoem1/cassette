# Audit: CPAL

## Workspace Signal

- Active dependency in `player/mod.rs`.

## Complete Technical Blueprint

- Core utility: cross-platform audio output.
- Auth flow: none.
- Webhooks/events: none in the web-service sense; callback-driven audio output is the relevant model.
- Role in Cassette: playback engine plumbing, not metadata or acquisition logic.

## Autonomous Suggestions

- Persist output-device choices, failures, and recovery events if playback reliability becomes a formal quality gate.
- Keep playback metrics distinct from library identity metrics.

## Critical Failings

- CPAL adds no sovereignty by itself.
- If playback failures are not logged with device context, the desktop layer will feel less trustworthy than the library layer.

## Sources

- Public-doc gap note: the canonical public surface is primarily crate documentation and repository materials.
- https://crates.io/crates/cpal
