# Worklist

Last updated: 2026-03-27

## Execution Rule

Prioritize architecture convergence, request fidelity, and provenance memory over adding more providers or UI chrome.

## Highest-Leverage Sequence

1. [ ] Unify the active runtime and the richer reconciliation/provenance schema.
       Decision needed: either migrate the Tauri runtime onto the richer `librarian`/`library` model, or port the missing provenance/cache tables into the active app DB. Do not keep growing two parallel truths.
2. [ ] Define and implement a first-class acquisition request contract.
       It must support track/album/artist/discography/selected-albums scopes, exclusions, edition policy, quality policy, provider policy, and confirmation policy.
3. [ ] Add canonical identity persistence.
       Persist MusicBrainz artist/release-group/release/recording IDs plus source-specific mappings for Spotify/Qobuz/Deezer/Discogs/local files.
4. [ ] Persist full candidate sets and provider attempts.
       This unlocks candidate review, query reuse, negative-result memory, and explainability.
5. [ ] Introduce a planning stage before acquisition.
       Metadata resolution, cache reuse, and candidate search should happen before any provider acquires bytes.
6. [ ] Separate torrent search from torrent resolving.
       If torrent acquisition stays, use Jackett as search owner and Real-Debrid as resolve/hoster owner. Retire TPB-inside-RD search ownership.
7. [ ] Wire post-acquisition closure through one canonical path.
       Acquire -> validate -> quarantine if needed -> tag/normalize -> import -> mark ownership/reconciliation -> persist provenance.
8. [ ] Expose candidate review and rationale in the UI.
       Support "show me candidates", "approve this one", "why was this chosen", and "don't use this edition/provider again".
9. [ ] Replace placeholder enrichers and explicitly retire dead ones.
       Either implement Discogs/Last.fm enrichers for real or leave them clearly dormant. Remove or absorb `downloader/` and `ProviderBridge` overlap.
10. [ ] Re-run proof and quality gates after architecture convergence.
        Fix the failing Rust test, remove current warnings, then document real end-to-end provider proof on this machine.

## Confirmed Gaps To Track

- [ ] `cargo test` is currently failing; fix and keep green.
- [ ] `cargo check` is currently warning-bearing; clean it up.
- [ ] `MetadataRepairOnly` remains stubbed.
- [ ] No candidate-review UX contract exists.
- [ ] No durable request/query/candidate cache exists in the active runtime path.
- [ ] No canonical release identity spine exists in the active runtime path.

## Deferred But Preserved

- `/playlists`, `/import`, and `/tools` routes remain implemented and callable.
- Additional provider expansion should stay behind architecture convergence, not happen ahead of it.
