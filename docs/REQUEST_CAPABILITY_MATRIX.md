# Request Capability Matrix

Last audited: 2026-04-03

Packet 1 implementation note (2026-04-07):

- `GAP-B01` is complete. Contract grammar and validation/error model are implemented in `src-tauri/src/commands/planner.rs` and specified in `PACKET_1_CONTRACT_SPEC.md`.

Status values:

- `Supported Today`
- `Could Support with Existing Building Blocks`
- `Blocked by Missing Integration`
- `Blocked by Missing Data Model`
- `Blocked by Missing UX Contract`
- `Not Realistic / Wrong Tool`

| User request | Status | Why |
| --- | --- | --- |
| Fetch one song by exact artist/title | Supported Today | `start_download` accepts artist/title and providers can search/acquire. Exactness is still mostly text-driven. |
| Fetch one album by exact artist/title | Supported Today | Album-level queueing exists, but release identity is not canonicalized first. |
| Fetch one artist discography | Supported Today | Discography metadata exists and can be queued with coarse filters. |
| Fetch only studio albums, exclude singles/EPs/compilations | Supported Today | Coarse release-type filters exist in `queue_discography_with_rules`. |
| Fetch these 4 albums only | Could Support with Existing Building Blocks | UI/command contract can already send specific album lists, but product grammar is informal. |
| Fetch these 4 albums but exclude these 2 named albums | Blocked by Missing UX Contract | No explicit include/exclude request grammar exists. |
| Fetch only non-live / non-remaster / non-deluxe editions | Blocked by Missing Data Model | Active runtime does not persist release-group/release identity or edition attributes robustly enough. |
| Fetch one track, but resolve parent album if needed | Supported Today | MusicBrainz parent-album helper exists. |
| Fetch one album by exact edition / country / label / catalog no. | Blocked by Missing Data Model | No canonical edition object in active runtime flow. |
| Compare multiple candidate releases before choosing | Blocked by Missing Integration | Internal candidate collection exists conceptually, but candidate persistence/review UX is absent. |
| Show me candidates before acquisition | Blocked by Missing UX Contract | No command/UI contract for candidate review. |
| Validate candidate before final download decision | Blocked by Missing Integration | Validation exists after acquire, but not as a user-facing preflight decision stage. |
| Reuse earlier search results for the same artist/album | Could Support with Existing Building Blocks | Provider response cache and candidate persistence exist, but planner-stage read APIs are still being surfaced. |
| Avoid re-querying providers already checked recently | Could Support with Existing Building Blocks | Runtime provider memory and response cache exist; planner-stage reuse is the active wiring task. |
| Detect already owned before download | Could Support with Existing Building Blocks | Duplicate/owned checks exist partially, but canonical identity mapping is missing. |
| Detect attempted before and failed | Could Support with Existing Building Blocks | `director_task_history` stores final results, but not normalized request signatures and provider-failure memory broadly enough. |
| Explain why this provider/result was chosen | Could Support with Existing Building Blocks | Score reason and attempt records exist in final result JSON, but are not surfaced well and candidate-set context is missing. |
| Explain why a lower-trust provider was chosen | Could Support with Existing Building Blocks | Internal attempt waterfall exists; UI/API exposure is missing. |
| Bulk acquire missing catalog for library artists | Could Support with Existing Building Blocks | Building blocks exist in Spotify/library queues and richer reconciliation DB, but they are not unified. |
| Normalize tags after import using canonical metadata | Supported Today | MusicBrainz + Lofty tag-fix flow exists. |
| Enrich artwork/label/catalog number/country/edition type | Could Support with Existing Building Blocks | Discogs and Last.fm integration paths exist; cover-art and deeper edition fidelity remain incomplete. |
| Maintain one canonical release identity across sources | Blocked by Missing Data Model | No unified MBID/source-ID mapping store in active runtime path. |
| Prove why this exact edition was chosen | Blocked by Missing Data Model | No edition object and no durable candidate comparison memory. |
| Reuse prior user exclusions/preferences in future requests | Blocked by Missing Data Model | No preference/exclusion persistence model. |
| Ask for manual confirmation before using yt-dlp or TPB/Real-Debrid fallback | Blocked by Missing UX Contract | Current waterfall is internal. |
| Download only after candidate review and approval | Blocked by Missing Integration | Needs staged candidate set + approval command. |
| Support "metadata repair only" as a real operation | Supported Today | `MetadataRepairOnly` strategy is implemented in the Director pipeline. |
