# Request Contract And Canonical Identity Plan

Last updated: 2026-04-04

## Goal

Make acquisition intent and identity fidelity explicit end-to-end so planner decisions are edition-aware, explainable, and durable across providers.

## Current State (Observed)

- Acquisition request contract supports explicit scopes: `track`, `album`, `artist`, `discography`, `selected_albums`.
- Request payloads already carry richer identity fields (`source_track_id`, `source_album_id`, `source_artist_id`, `isrc`, MB recording/release-group/release IDs, canonical IDs, policy fields).
- Runtime and sidecar persist canonical artist/release/recording and source-alias evidence surfaces.
- Planner and queue boundaries preserve many richer fields, but release-group identity is not yet used as a first-class planner decision lane.

## MusicBrainz-Backed Identity Plan

1. Identity capture phase
- Capture and persist MB artist/release-group/release/recording IDs as early as practical at ingest and planning boundaries.
- Preserve source-specific aliases (Spotify/Qobuz/Deezer/Discogs/local-file evidence) alongside MB IDs.

2. Identity normalization phase
- Normalize incoming request targets into a canonical identity envelope before planner search.
- Derive request signatures from this envelope to stabilize cache reuse and negative-result memory.

3. Planner decision phase
- Promote release-group identity to a planner-visible input for candidate narrowing and edition policy checks.
- Make edition mismatches explicit in rationale output rather than hidden in provider-specific scoring details.

4. Execution and closure phase
- Preserve canonical identity envelope across planner -> Director handoff without downcasting to loose text keys.
- Persist final selection lineage with request signature + canonical IDs + source aliases for audit replay.

## Hard Requirements

- No queue boundary may collapse to `artist + title + optional album` when richer identity already exists.
- Planner rationale must expose identity confidence and edition-policy implications.
- Request-contract tests must gate regressions at command boundaries.

## Verification Targets

- Contract coverage tests for every scope variant (`track`, `album`, `artist`, `discography`, `selected_albums`).
- Planner rationale snapshot proving release-group identity is queryable in review output.
- End-to-end trace proof from request creation to finalization preserving canonical IDs and source aliases.

## Out Of Scope (This Pass)

- New provider additions.
- Deep UI redesign beyond the existing planner review lane.
- Full multi-machine identity reconciliation.
