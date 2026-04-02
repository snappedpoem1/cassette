# Audit: Beets

## Workspace Signal

- Not used at runtime.
- Explicitly referenced through the local `tools/beets-proof/` experiment.

## Complete Technical Blueprint

- Core utility: import pipeline, metadata normalization, path templating, plugin ecosystem.
- Auth flow: depends on plugin and upstream service.
- Webhooks/events: plugin/event hooks inside Beets itself.
- Strategic role for Cassette: reference implementation for modular metadata bridges.

## Autonomous Suggestions

- Borrow Beets' plugin split, not its control plane.
- Use Beets-like MBID sync, chroma/fingerprint logic, and Discogs secondary enrichment as design patterns inside Cassette.
- Keep Cassette's queue, audit, and rollback semantics sovereign instead of shelling out broad control.

## Critical Failings

- Beets is great at import-time normalization, but it is not your desktop governor.
- Its pipeline assumptions can be too opinionated for live multi-provider orchestration.
- If Cassette delegates too much to Beets, auditability fragments across tools.

## Sources

- https://docs.beets.io/en/latest/plugins/index.html
- https://docs.beets.io/en/stable/plugins/discogs.html
- https://beets.readthedocs.io/en/stable/plugins/chroma.html
- https://docs.beets.io/en/v1.1.0/plugins/mbsync.html
