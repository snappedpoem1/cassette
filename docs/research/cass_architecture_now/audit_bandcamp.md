# Audit: Bandcamp

## Workspace Signal

- Placeholder source adapter in `director/sources/bandcamp.rs`.
- Not operationally wired today.

## Complete Technical Blueprint

- Core utility: purchase/account/business surfaces for owned material, not a broad public metadata API.
- Auth flow: official developer surfaces are account/business oriented.
- Webhooks/events: not central to current Cassette use.
- Strategic fit: owner-side purchase sync, not general catalog authority.

## Autonomous Suggestions

- Model Bandcamp as "owned-purchase provenance" when you have it.
- Persist order/download evidence separately from release identity.
- Use Bandcamp URLs as source breadcrumbs but reconcile into MBIDs locally.

## Critical Failings

- It is not a general discovery or catalog API.
- Treating Bandcamp like Spotify or MusicBrainz would be a category error.
- A placeholder adapter should stay placeholder until Cassette decides whether this is a purchase-sync lane or a scrape experiment.

## Sources

- https://bandcamp.com/developer
- https://bandcamp.com/developer/account
- https://bandcamp.com/developer/sales
- https://bandcamp.com/developer/merch
