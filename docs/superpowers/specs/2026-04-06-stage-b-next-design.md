# Stage B Next: Planner Cutover, Enrichment Proof, Adaptive Orchestrator, Dead-Letter Center

Date: 2026-04-06  
Owner: Christian (single-owner personal project)  
Status: Approved for implementation

## Scope

Four sequential work items in dependency order:

1. Planner cutover for `engine_pipeline_cli` coordinator loop
2. Discogs and Last.fm enrichment end-to-end proof
3. Adaptive Provider Orchestrator (Stage B item 4)
4. Dead-Letter Command Center (Stage B item 5)

Each item is fully independent of items that follow it but depends on items before it. Execute in order.

---

## Item 1: Planner Cutover for `engine_pipeline_cli`

### Context

Song and album/artist UI submissions already use the canonical planner path:
`plan_acquisition` → `approve_planned_request` → Director submission.

The coordinator loop in `engine_pipeline_cli` still builds `TrackTask`s directly from `delta_queue`
rows and submits them to the Director engine, bypassing the planner entirely. This means
coordinator-originated acquisitions have no candidate sets, no rationale, no source-alias refresh,
and no identity-evidence persistence — they are invisible to the provenance trail.

### What Changes

When the coordinator processes a `delta_queue` row it must:

1. Call `plan_acquisition` with the row's identity fields (artist, title, album, ISRC, MusicBrainz
   IDs, source IDs, release-group ID, policies) to populate candidate sets, provider search
   evidence, source aliases, and identity resolution evidence in the runtime DB.
2. Immediately call `approve_planned_request` with reason code `coordinator_auto_approve` — no
   UI gate, no wait. The Director submission happens through this approval path.
3. The approval path already persists audit events and pending-task rows. No extra persistence work
   is needed.

The coordinator's outer loop, scan/checkpoint/claim logic, stale-claim reclaim, post-sync, and
organize-subset steps are untouched.

The `--operator-direct-submit` flag remains as an explicit escape hatch for debugging and
operator use. It bypasses the planner path and submits directly, exactly as today.

### Touchpoints

- `src-tauri/src/bin/engine_pipeline_cli.rs` — coordinator submission path
- `src-tauri/src/commands/planner.rs` — `plan_acquisition`, `approve_planned_request`
- `crates/cassette-core/src/librarian/orchestrator.rs` — delta_queue claim/process loop

### Acceptance

- [ ] Coordinator loop calls `plan_acquisition` per delta row before submission
- [ ] Coordinator loop calls `approve_planned_request` with `coordinator_auto_approve` reason code
- [ ] `--operator-direct-submit` still bypasses planner (no regression)
- [ ] Bounded run (`--resume --limit 5`) produces `acquisition_requests` rows with candidate sets
      and rationale in identical shape to UI-originated requests
- [ ] Proof captured and recorded in `PROJECT_STATE.md`
- [ ] `TODO.md` and `HIT_LIST.md` updated to mark planner cutover done

---

## Item 2: Discogs and Last.fm Enrichment End-to-End Proof

### Context

Both enrichment clients are implemented and wired:

- `crates/cassette-core/src/librarian/enrich/discogs.rs` — fetches release context (year, genres,
  styles, labels, country), writes `discogs_id` back to track
- `crates/cassette-core/src/librarian/enrich/lastfm.rs` — fetches artist context (tags, listeners,
  summary) and album context (summary, image URL)

Both clients have live API-backed implementations. What is missing is a proof artifact and canonical
doc coverage. `PROJECT_STATE.md` currently notes: "a full automatic background enrichment queue
worker is still pending."

### What Changes

A new CLI binary `enrich_probe_cli` is added to `src-tauri/src/bin/`. It:

1. Opens the live runtime DB at the Tauri app-data path (or accepts `--db-path` override).
2. Loads Discogs token and Last.fm API key from runtime config.
3. Selects a fixed sample of 5–10 tracks from `tracks` where `artist` and `album` are non-empty,
   ordered by `updated_at DESC` (recently scanned tracks are most likely to have clean metadata).
4. For each track, calls `DiscogsClient::fetch_release_context` and
   `LastFmClient::fetch_artist_context` with the track's artist/album fields.
5. Prints a structured outcome table: track identity, Discogs outcome (release_id, year, genres),
   Last.fm outcome (tags, listeners), and any errors.
6. Does not write to the DB — probe only, no side effects.

The probe is run live, outcomes are recorded in `PROJECT_STATE.md` as a proof artifact (same style
as the Spotify replay proof). Canonical docs (`PROJECT_STATE.md` enrichment section,
`TOOL_AND_SERVICE_REGISTRY.md` Discogs/Last.fm rows) are updated to reflect observed runtime
behavior.

**No background enrichment worker is introduced.** That remains explicitly pending per
`PROJECT_STATE.md`.

### Touchpoints

- `src-tauri/src/bin/enrich_probe_cli.rs` — new binary
- `src-tauri/Cargo.toml` — register new binary
- `crates/cassette-core/src/librarian/enrich/discogs.rs` — consumed as-is
- `crates/cassette-core/src/librarian/enrich/lastfm.rs` — consumed as-is
- `docs/PROJECT_STATE.md` — proof artifact recorded
- `docs/TOOL_AND_SERVICE_REGISTRY.md` — Discogs/Last.fm rows updated

### Acceptance

- [ ] `enrich_probe_cli` compiles as part of workspace (`cargo check --workspace`)
- [ ] Probe runs against live DB with real credentials and prints structured outcome table
- [ ] Discogs enrichment outcomes recorded for ≥5 tracks
- [ ] Last.fm enrichment outcomes recorded for ≥5 tracks
- [ ] At least one successful Discogs release context fetch (non-None result)
- [ ] At least one successful Last.fm artist context fetch (non-None result)
- [ ] Proof artifact recorded in `PROJECT_STATE.md`
- [ ] `TOOL_AND_SERVICE_REGISTRY.md` Discogs/Last.fm rows reflect observed runtime behavior
- [ ] `TODO.md` and `HIT_LIST.md` updated to mark enrichment proof done

---

## Item 3: Adaptive Provider Orchestrator

### Context

The Director waterfall orders providers by static `trust_rank` (lower = higher priority). The engine
already consults `director_provider_memory` to skip dead-end providers (`PersistedMemory` skip
variant) and hydrate cached candidates. What is missing is using recent success memory to nudge
provider order upward for requests where a provider has recently succeeded.

This is Stage B item 4 from `CASSETTE_MUSIC_OS_IMPLEMENTATION_PLAN.md`.

### What Changes

After the static trust_rank sort and before the first provider is invoked, the engine applies a
lightweight adaptive nudge pass:

1. For each provider in the sorted waterfall, read its most recent `director_provider_memory` row
   for the current request signature.
2. Apply nudge:
   - `last_outcome = finalized` and `updated_at` within 7 days → apply a trust_rank bonus of `-3`
     (promotes the provider 3 positions worth in rank space)
   - `last_outcome` triggers existing dead-end skip → no change (skip path already handles this)
   - No memory row, stale memory (>7 days), or any other outcome → no nudge, static order preserved
3. Hard boundary: a nudge cannot move a provider past a provider with trust_rank ≤ 10 (Qobuz,
   slskd). High-trust providers are never demoted by another provider's success nudge.
4. Re-sort the waterfall by `effective_rank = trust_rank + nudge` after applying all nudges.
5. Log any reorder at `tracing::debug` level with reason code `adaptive_nudge` and the before/after
   provider order.

The nudge value (3) and recency window (7 days) are constants in `director/config.rs`, not
hardcoded at the call site, so they can be tuned without touching engine logic.

No schema changes. All reads are from existing `director_provider_memory` rows. The existing
`StoredProviderMemory` query surface is sufficient.

### Touchpoints

- `crates/cassette-core/src/director/engine.rs` — waterfall ordering pass
- `crates/cassette-core/src/director/config.rs` — nudge constants
- `crates/cassette-core/src/director/models.rs` — add `adaptive_nudge` to attempt trail reason
  codes if not already representable

### Acceptance

- [ ] Adaptive nudge pass applied after static sort, before first provider invocation
- [ ] Recent success (≤7 days, `finalized`) produces a `-3` effective rank nudge
- [ ] Nudge cannot promote a provider past a provider with trust_rank ≤ 10
- [ ] Reorders logged at debug level with `adaptive_nudge` reason and before/after order
- [ ] Nudge value and recency window are config constants, not magic numbers in engine
- [ ] `cargo test --workspace` passes (no regression to existing provider memory skip tests)
- [ ] `PROJECT_STATE.md` updated with adaptive orchestrator evidence
- [ ] `TODO.md` Stage B item 4 marked done

---

## Item 4: Dead-Letter Command Center

### Context

Failed tasks land in `director_task_history` with `disposition = Failed`, `failure_class`, and
`provider` set. The existing `failure_class` values from `classify_failure` are:

- `auth_failed`
- `rate_limited`
- `provider_busy`
- `validation_failed`
- `metadata_only`
- `provider_exhausted` (catch-all)

The Downloads UI has a Blocked lane for active/pending blocked requests. Permanently failed
terminal history has no dedicated surface — it is invisible unless you query the DB directly.

This is Stage B item 5 from `CASSETTE_MUSIC_OS_IMPLEMENTATION_PLAN.md`.

### What Changes

**Backend — new Tauri command `get_dead_letter_summary`:**

Queries `director_task_history` where `disposition IN ('Failed', 'Cancelled')` and returns:

```
DeadLetterSummary {
    groups: Vec<DeadLetterGroup>,
    total_count: usize,
}

DeadLetterGroup {
    failure_class: String,          // e.g. "auth_failed"
    label: String,                  // plain-language label
    suggested_fix: String,          // plain-language hint
    count: usize,
    recent_items: Vec<DeadLetterItem>,  // top 5 most recent
}

DeadLetterItem {
    task_id: String,
    artist: String,
    title: String,
    album: Option<String>,
    provider: Option<String>,
    failed_at: String,              // ISO timestamp
    request_json: Option<String>,   // for replay
}
```

Plain-language labels and suggested fixes per class:

| failure_class | label | suggested_fix |
|---|---|---|
| auth_failed | Authentication failed | Check provider credentials in Settings |
| rate_limited | Rate limited | Provider is throttling requests — wait and retry |
| validation_failed | File failed validation | Candidate audio was corrupt or mismatched |
| provider_busy | Provider busy | Provider was at capacity — will retry automatically |
| metadata_only | No downloadable file found | Provider returned metadata but no audio |
| provider_exhausted | All providers exhausted | No provider had a matching file |

**Backend — new Tauri command `replay_dead_letter`:**

Accepts a `task_id`. Reads the stored `request_json` from `director_task_history`, reconstructs an
`AcquisitionRequest`, calls `plan_acquisition` → `approve_planned_request` with reason code
`dead_letter_replay`. The original `task_id` is recorded in the new request's lineage as
`replayed_from`. Returns the new `acquisition_request_id`.

**Frontend — Downloads dead-letter section:**

Below the existing Blocked lane, add a collapsible "Dead Letters" section. It is collapsed by
default to avoid visual noise during normal operation. When expanded it renders the grouped cards
from `get_dead_letter_summary`. Each card shows:

- Failure class label and count badge
- Suggested fix text
- List of recent items (artist – title, provider, timestamp)
- Per-item "Retry" button that calls `replay_dead_letter`

The dead-letter section is visually distinct from the active Blocked lane (different header,
muted color treatment) so in-progress blocked requests and permanently failed ones are never
confused.

**No new schema.** Reads `director_task_history`. Replay uses the existing planner path.

### Touchpoints

- `crates/cassette-core/src/db/mod.rs` — `get_dead_letter_summary` query
- `src-tauri/src/commands/` — new `dead_letter.rs` command module
- `src-tauri/src/lib.rs` — register new commands
- `ui/src/routes/downloads/+page.svelte` — dead-letter section
- `ui/src/lib/` — `DeadLetterCard` component (or inline if simple enough)

### Acceptance

- [ ] `get_dead_letter_summary` returns grouped failure classes with counts and recent items
- [ ] `replay_dead_letter` reconstructs and resubmits through planner path with `dead_letter_replay`
      reason code and `replayed_from` lineage
- [ ] Downloads page renders collapsible dead-letter section below Blocked lane
- [ ] Dead-letter section is collapsed by default
- [ ] Each group shows label, suggested fix, count, and recent items
- [ ] Per-item Retry button calls `replay_dead_letter` and surfaces success/error feedback
- [ ] Dead-letter section is visually distinct from active Blocked lane
- [ ] `cargo test --workspace` passes
- [ ] `npm run build` passes
- [ ] `PROJECT_STATE.md` updated with dead-letter center evidence
- [ ] `TODO.md` Stage B item 5 marked done

---

## Verification Contract (all items)

For each item before marking done:

1. `cargo check --workspace` clean
2. `cargo test --workspace` passes
3. `npm run build` passes (items 1, 4)
4. `.\scripts\smoke_desktop.ps1` passes (items 1, 4)
5. One deterministic failure-path proof or live proof captured
6. `PROJECT_STATE.md`, `TODO.md`, `HIT_LIST.md` updated

## Execution Order

1 → 2 → 3 → 4. Do not begin an item until the previous item's acceptance criteria are fully met
and verification contract passes.
