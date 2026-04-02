# Audit: slskd / Soulseek

## Workspace Signal

- Active provider in `director/providers/slskd.rs`.
- Session/login helper and transfer fetchers in `sources.rs`.
- slskd config is loaded from local daemon state and app settings.

## Complete Technical Blueprint

- Core utility: P2P search, browse, queue, transfer polling, and daemonized automation.
- Auth flow: API key or session token from username/password against the daemon.
- Webhooks/events: the daemon is API-driven; Cassette currently polls and manages state itself.
- Operational reality: peer availability and queue state are transient by nature.

## Autonomous Suggestions

- Persist search-result snapshots, chosen remote paths, queue timestamps, and transfer hashes locally.
- Store remote-share evidence apart from final local file records so retries remain explainable.
- Add a provider-memory policy that cools down exhausted peers and reuses recent successful user/path heuristics.

## Critical Failings

- Search results are ephemeral and peer-specific.
- Queue saturation and daemon wedging are real operational hazards; Cassette already works around some of this.
- Soulseek gives you files, not trusted metadata identity.

## Sources

- https://github.com/slskd/slskd
