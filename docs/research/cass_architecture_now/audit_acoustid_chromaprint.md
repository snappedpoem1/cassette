# Audit: AcoustID / Chromaprint

## Workspace Signal

- `acoustid_fingerprint` appears in gatekeeper/orchestrator schema and collision logic.
- The runtime clearly wants fingerprint-backed identity, but the graph is not fully activated.

## Complete Technical Blueprint

- Core utility: audio fingerprint generation and fingerprint-to-recording bridge.
- Auth flow: local fingerprinting via Chromaprint; network lookup/submission via AcoustID key.
- Webhooks/events: none.
- Rate model: AcoustID is intentionally scoped and rate-limited; it is a bridge service, not a bulk metadata warehouse.

## Autonomous Suggestions

- Compute and persist fingerprints at ingest time for all stable local files.
- Use fingerprint -> MBID as the first recovery step for messy or poorly tagged files.
- Keep fingerprint evidence in a separate immutable table so re-tagging never erases acoustic proof.

## Critical Failings

- It will not provide rich release metadata by itself.
- Commercial-scale assumptions do not fit the default service posture.
- Without local Chromaprint execution, the `acoustid_fingerprint` columns stay aspirational.

## Sources

- https://acoustid.org/webservice
- https://acoustid.org/chromaprint
- https://acoustid.org/license
