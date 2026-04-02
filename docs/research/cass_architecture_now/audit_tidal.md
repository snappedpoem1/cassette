# Audit: Tidal

## Workspace Signal

- Mentioned in project docs as an idea.
- No active provider, source adapter, or settings surface found in the current runtime.

## Complete Technical Blueprint

- Core utility: premium streaming catalog and playback ecosystem.
- Auth flow: account- and client-bound API surfaces.
- Webhooks/events: not relevant to Cassette's current code because no integration exists.
- Strategic fit: possible future premium metadata/acquisition lane, but purely hypothetical in the present repo.

## Autonomous Suggestions

- Do not add Tidal until the MBID/source-alias graph exists.
- If it ever lands, model it exactly like Spotify/Qobuz/Deezer: alias layer, cached snapshots, no canonical authority.

## Critical Failings

- Right now Tidal is conceptual clutter more than architecture.
- A doc mention without a runtime plan is not research debt, it is scope bait.

## Sources

- Official-doc gap note for the present repo context: no current Cassette integration exists, so this file is here to close the inventory loop, not to propose immediate implementation.
