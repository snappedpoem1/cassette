# Worklist

Last updated: 2026-04-04

## Execution Rule

Prioritize architecture convergence, request fidelity, and provenance memory over adding more providers or UI chrome.

## Highest-Leverage Sequence

1. [ ] Prove the sidecar control plane end-to-end before any deeper schema convergence.
       The deliberate current shape is `cassette.db` for playback/runtime state and `cassette_librarian.db`
       for librarian/orchestrator reconciliation, queueing, and scan checkpoints. Capture the bounded
       coordinator proof and recovery proof before reopening schema-convergence questions.
2. [ ] Define and implement a first-class acquisition request contract.
       It must support track/album/artist/discography/selected-albums scopes, exclusions, edition policy, quality policy, provider policy, and confirmation policy.
3. [ ] Add canonical identity persistence.
       Persist MusicBrainz artist/release-group/release/recording IDs plus source-specific mappings for Spotify/Qobuz/Deezer/Discogs/local files.
4. [ ] Reuse the persisted candidate sets and provider attempts already present in the active runtime.
       This unlocks candidate review, query reuse, negative-result memory, and explainability.
5. [ ] Introduce a planning stage before acquisition.
       Metadata resolution, cache reuse, and candidate search should happen before any provider acquires bytes.
6. [ ] Separate torrent search from torrent resolving.
       If torrent acquisition stays, use Jackett as search owner and Real-Debrid as resolve/hoster owner. Retire TPB-inside-RD search ownership.
7. [ ] Wire post-acquisition closure through one canonical path.
       Acquire -> validate -> quarantine if needed -> tag/normalize -> import -> mark ownership/reconciliation -> persist provenance.
8. [ ] Expose candidate review and rationale in the UI.
       Support "show me candidates", "approve this one", "why was this chosen", and "don't use this edition/provider again".
9. [ ] Document and prove Discogs/Last.fm/Bandcamp runtime behavior in canonical docs.
       The implementations now exist, but canonical docs still need consistency updates and
       end-to-end proof coverage language.
10. [ ] Re-run proof and quality gates after coordinator proof capture.
        The Rust test gate is green and Deezer full-track is live-proven on this machine.
        The remaining top-level quality work is audit completeness, coordinator recovery proof,
        organizer safe-subset proof, and packaging confidence.

## Confirmed Gaps To Track

- [ ] `MetadataRepairOnly` needs stronger runtime proof and coverage documentation.
- [ ] No candidate-review UX contract exists.
- [ ] Terminal request payload is now retained in `director_task_history`, but the UI still lacks
      candidate-review and exclusion-memory flows that reuse persisted provenance.
- [ ] No canonical release identity spine exists in the active runtime path.

## Follow-On Scope: Request Contract And Identity Spine

1. [x] Thread release-group identity through planner ranking and candidate explanation.
       Carry `musicbrainz_release_group_id` into planner-visible rationale so edition-level review
       is queryable before approval, not only in persisted request payload fields.
2. [x] Enforce richer identity at all queue boundaries.
       Prevent regressions where queue/request creation falls back to `artist + title + optional album`
       when source IDs or MusicBrainz IDs are already available.
3. [x] Add explicit request-contract coverage tests at command boundaries.
       Validate that song/album/artist/discography/selected-albums scopes preserve source IDs,
       MB recording/release-group/release IDs, policies, and signatures through planner -> Director handoff.
4. [x] Add a planner-time edition policy lane.
       Use release-group identity plus edition policy constraints to narrow candidate sets before
       byte acquisition in review flows.

## Deferred But Preserved

- `/playlists`, `/import`, and `/tools` routes remain implemented and callable.
- Additional provider expansion should stay behind architecture convergence, not happen ahead of it.
