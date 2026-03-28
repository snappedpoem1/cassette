# Integration Gaps And Opportunities

Last audited: 2026-03-27

## Highest-Value Gaps

| Gap | Why it matters | Existing parts that already help |
| --- | --- | --- |
| No canonical release identity in active runtime | Blocks edition-aware selection, provenance, and dedupe | MusicBrainz resolver, richer `librarian` schema |
| Metadata search and acquisition search are split | Causes re-querying, weak planning, and poor explainability | `sources.rs`, `director`, `metadata.rs` |
| Candidate sets are not persisted | Blocks user review and reuse | `director` scoring/attempt records already exist |
| Rich reconciliation DB is not the active app spine | Wastes existing desired-track/delta/provenance work | `librarian`, `gatekeeper`, `custodian`, `orchestrator` |
| Torrent path uses TPB inside Real-Debrid provider | Conflates search and resolve responsibilities | Jackett config placeholders, RD API integration |
| Validation and quarantine are split across two pipelines | Prevents one auditable intake path | `director::validation`, `custodian`, `gatekeeper` |

## Concrete Opportunities

### 1. Make metadata resolution the first planning stage

Use:

- MusicBrainz for canonical artist/release-group/release/recording identity
- Spotify/Qobuz/Deezer for additional discoverability and source ID mapping

Then hand the normalized request to acquisition providers. This will shrink fuzzy text-only matching and make downstream explanation possible.

### 2. Separate search owner from acquisition owner

Recommended split:

- Jackett: torrent search owner
- Real-Debrid: cached torrent resolver / hoster owner
- slskd: P2P search + transfer owner
- NZBGeek: NZB search owner
- SABnzbd: NZB execution owner
- yt-dlp: last-resort fallback owner

Today, Real-Debrid is doing double duty because its provider searches TPB directly.

### 3. Reuse the richer reconciliation pipeline for acquisition memory

The repo already has tables for:

- desired tracks
- reconciliation results
- delta queue
- operation log
- invariant violations

That is exactly where "already owned", "missing", "manual review", and "attempted before" logic should live.

### 4. Turn internal candidate scoring into a user-visible review contract

Needed command shape:

- `plan_acquisition(request) -> candidate_set`
- `review_candidate_set(candidate_set_id)`
- `approve_candidate(candidate_item_id)`
- `apply_request_rules(request_rule_set_id)`

The engine already knows how to score/validate. The product just does not expose the stage.

### 5. Close the loop after acquisition

After finalization, automatically:

- validate
- tag/normalize
- map to canonical identities
- index/import
- mark ownership/reconciliation state
- persist provenance and user-visible rationale

Today those stages exist in pieces but not as one canonical closure path.
