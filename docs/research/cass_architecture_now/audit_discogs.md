# Audit: Discogs

## Workspace Signal

- Stub enricher in `crates/cassette-core/src/librarian/enrich/discogs.rs`.
- `discogs_id` fields already exist in richer schema paths.
- Token surface exists in settings, but the active runtime does not use it.

## Complete Technical Blueprint

- Core utility: release variants, masters, labels, format/country/catalog data, marketplace-adjacent metadata.
- Auth flow: personal token / OAuth depending on use case.
- Webhooks/events: not a primary fit for Cassette's current needs.
- Rate/terms reality: useful secondary metadata, but not a clean unrestricted data source.

## Autonomous Suggestions

- Use Discogs only after MBID resolution as a variant disambiguator for country/pressing/format edges.
- Persist only stable IDs and normalized variant facts you actually use.
- Keep Discogs snapshots TTL-bound because policy and data access are more constrained than MusicBrainz.

## Critical Failings

- Restricted-data terms make it a poor canonical store.
- It is richer for release commerce than for sovereign local truth.
- If Cassette lets Discogs outrank MusicBrainz, the identity graph will become edition-heavy but structurally inconsistent.

## Sources

- https://www.discogs.com/developers
- https://support.discogs.com/hc/en-us/articles/360009334593-API-Terms-of-Use
