# Audit: ListenBrainz

## Workspace Signal

- Not wired today.
- Strong strategic fit because Cassette wants an AI-ready listening graph.

## Complete Technical Blueprint

- Core utility: listen history, MBID-linked behavioral context, recommendation and metadata endpoints.
- Auth flow: user token for write/personalized access; some endpoints are public.
- Webhooks/events: not the main model; polling and API ingestion fit better.
- Special semantic: listen counting has explicit thresholds, which matters for modeling confidence.

## Autonomous Suggestions

- Persist listens as MBID-linked facts, not raw service payloads only.
- Use ListenBrainz as the "behavior spine" beside MusicBrainz's identity spine.
- Feed AI ranking from MBID-linked listen recency, repetition, skips, and recommendation overlap.

## Critical Failings

- It is behavioral context, not catalog truth.
- Some upstream-connected listens can have latency and service-dependency quirks.
- If Cassette stores only raw usernames or track text instead of MBIDs, most of ListenBrainz's value gets lost.

## Sources

- https://listenbrainz.readthedocs.io/en/latest/users/api/index.html
- https://listenbrainz.readthedocs.io/en/latest/users/api/core.html
- https://listenbrainz.readthedocs.io/en/latest/users/api/metadata.html
- https://listenbrainz.readthedocs.io/en/latest/users/api/recommendation.html
