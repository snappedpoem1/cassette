# Audit: NZBGeek

## Workspace Signal

- API key surface exists in settings and runtime bootstrap.
- Used as the search side of Cassette's Usenet lane.

## Complete Technical Blueprint

- Core utility: NZB indexing and search.
- Auth flow: API key.
- Webhooks/events: not Cassette's current fit.
- Public-doc reality: indexer documentation is thinner and more operational than the major open APIs.

## Autonomous Suggestions

- Archive the exact NZB metadata or search result signature for every accepted candidate.
- Store indexer source separately from SAB execution state.
- Add negative-result caching per album/track request so indexer misses become durable memory.

## Critical Failings

- Indexer availability is a sovereignty weak point; if the indexer disappears, Cassette loses discovery but should not lose truth.
- Search quality depends on naming discipline outside your control.
- NZBGeek is search infrastructure, not identity infrastructure.

## Sources

- Public-doc gap note: current official searchable developer documentation is limited; treat this surface as operationally useful but contract-light.
