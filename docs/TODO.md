# Cassette TODO

**Method**: Prioritize by user impact, reliability risk, and execution clarity.  
**Rule**: If a task is not in this file, it is not committed project scope yet.  
**Last Updated**: 2026-04-06

Scope note:

- This TODO is for a single-owner personal project.
- Terms such as "shipping blocker" and related release language mean personal reliability/readiness blockers only.

Short execution board: see `HIT_LIST.md`.

---

## Legend

Priority:

- `P0` critical shipping blocker
- `P1` important hardening work
- `P2` worthwhile improvement
- `P3` backlog or exploratory work

Status:

- `todo`
- `in_progress`
- `blocked`
- `review`
- `done`

---

## Current Audited Sequence

These are the next highest-value tasks after the tool-convergence and scope-reset pass.
Do them in this order unless a higher-priority production issue interrupts:

1. Audit and correct tool-role documentation drift.
2. Unify Spotify ingest lanes into one identity-first path.
3. Route all album expansion through the resilient shared resolver.
4. Separate search owners from execution owners across torrent and Usenet lanes.
5. Promote canonical identity and source-alias persistence to the active path.
6. Introduce a planner stage before byte acquisition.
7. Prove audit completeness across organization and admission flows.
8. Formalize performance baselines and regression budgets.

---

## P0

### [P0] [done] Capture the first bounded coordinator live proof

Why:

- The runtime shape is now in code: sidecar scan checkpoints, `delta_queue` claims,
  Director payload closure, post-run rescan, and guarded organizer subset logic.
- What is still missing is one bounded, inspectable real-machine proof that the loop works
  end-to-end.

What good looks like:

- A bounded run such as `engine_pipeline_cli --resume --limit 5` starts from a populated sidecar.
- `delta_queue` rows are claimed deterministically and resolved into Director `TrackTask`s.
- Successful terminal outcomes stamp `processed_at`.
- Retryable failures release claims without falsely closing the ticket.
- The post-run librarian sync closes at least one satisfied reconciliation gap.

Touchpoints:

- `src-tauri/src/bin/engine_pipeline_cli.rs`
- `crates/cassette-core/src/librarian/orchestrator.rs`
- Librarian sidecar DB tables: `local_files`, `desired_tracks`, `reconciliation_results`, `delta_queue`
- Director task-history persistence in the active runtime DB

Acceptance:

- [x] Queue claim fields (`claimed_at`, `claim_run_id`, `source_operation_id`) ensured in migrations
- [x] `engine_pipeline_cli` claims actionable rows and releases stale claims
- [x] `processed_at` is marked on successful terminal outcomes
- [x] Librarian sync bookends the coordinator run
- [x] Sidecar scan checkpoints and `full|resume|delta-only` scan modes exist, with unchanged files skipped on rerun
- [x] `generate_delta_queue` preserves claimed (mid-flight) rows — only unclaimed unprocessed rows are wiped
- [x] `mark_processed` preserves `claimed_at`/`claim_run_id` as audit trail; only sets `processed_at`
- [x] Claim-preservation behavior is regression-tested (`adapter_tests::mark_processed_preserves_claim_fields`, `generate_delta_preserves_claimed_rows`)
- [x] End-to-end proof captured: scan -> `delta_queue` populated -> acquisition -> queue state updated -> re-scan closes a gap
- [x] Proof artifact set recorded in `PROJECT_STATE.md` (2026-03-31: DENIAL IS A RIVER via Qobuz, Finalized, claim audit trail intact)

### [P0] [done] Capture coordinator recovery proof with interruption plus resume

Why:

- The code now has resumable sidecar scan state, stale-claim recovery, and deterministic queue leasing.
- That still needs one live proof showing that interruption does not force a restart from zero
  or strand queue work in a bad claim state.

What good looks like:

- An interrupted coordinator run is resumed with `--resume` and continues from durable scan checkpoints.
- Unchanged files are skipped on rerun.
- Stale `delta_queue` claims are reclaimed deterministically.
- Work that already succeeded is not reacquired.
- Retryable or interrupted work remains actionable after resume.

Touchpoints:

- `src-tauri/src/bin/engine_pipeline_cli.rs`
- `crates/cassette-core/src/librarian/scanner`
- Librarian sidecar DB tables: `scan_checkpoints`, `local_files`, `delta_queue`

Acceptance:

- [x] Resume semantics implemented in code
- [x] Queue claims are durable and stale-claim recovery exists
- [x] Live interruption/resume proof captured
- [x] Proof shows resumed scan skips unchanged files and recovers queue work cleanly

### [P0] [done] Finish organizer safety proof after staged track-number repair

Why:

- The repair ladder is implemented and organizer live-guarding is in place.
- The remaining risk is operational, not architectural: prove a bounded live organize pass is safe.

What good looks like:

- `organize_cli --dry-run` no longer proposes large classes of recoverable `00 - ...` renames.
- Unresolved rows are explicit and enumerable.
- A bounded live subset organizes safely without corrupting filenames.

Touchpoints:

- `src-tauri/src/bin/tag_rescue_cli.rs`
- `src-tauri/src/bin/organize_cli.rs`
- Active runtime DB `tracks` table

Acceptance:

- [x] Tag re-scan pass implemented (`tag_rescue_cli`)
- [x] `organize_cli --live` now hard-blocks suspicious mass `00 - ...` renames
- [x] Tag rescue run against the live DB captured (`updated=0`)
- [x] Staged recovery via `embedded_tag`, `filename_prefix`, and `album_pattern` implemented with explicit unresolved reporting
- [x] Bounded live organize proof captured on a safe subset
- [x] Post-proof unresolved set documented clearly

### [P0] [done] Prove audit completeness across organization and admission flows

Why:

- Lineage and traceability are core promises of Cassette. Validation support exists,
  but this must remain a provable shipping gate, not an assumption.

What good looks like:

- Representative file flows produce complete operation and event trails.
- We can explain what happened to a file without guesswork.
- Validation/logging checks fail loudly if audit coverage regresses.

Touchpoints:

- `crates/cassette-core/src/custodian`
- `crates/cassette-core/src/gatekeeper`
- `crates/cassette-core/src/library`
- `crates/cassette-core/src/validation`

Acceptance:

- [x] Canonical audit-trace query surface exists for operation events plus gatekeeper audit rows
- [x] Regression coverage exists for the audit-trace query surface
- [x] Representative tests added or updated
- [x] Validation/logging proof is repeatable
- [x] Documentation updated if expectations change

### [P0] [done] Prove Deezer full-track acquisition end-to-end

Why:

- This was a hard shipping blocker. The path is now proven live on this machine.

Acceptance:

- [x] End-to-end live proof documented in `PROJECT_STATE.md`
- [x] Any remaining partial paths named and tracked

### [P0] [done] Prove pending-task crash recovery end-to-end

Why:

- The runtime now persists pending Director tasks and resubmits them on startup.
- This is proven through a deterministic startup-recovery probe plus filtering tests for stale
  pending rows versus newer terminal history.

Acceptance:

- [x] Startup recovery proof captured via `recovery_probe_cli`
- [x] Recovery ordering and stale-terminal filtering documented
- [x] `PROJECT_STATE.md` updated with exact observed behavior

---

## P1

### [P1] [done] Harden async and recovery behavior in acquisition flows

Why:

- Acquisition paths are where flaky networks, partial downloads, and timeouts converge.
- Fresh coordinator recovery/resume behavior is now live-proven again (2026-04-06) with deterministic stale-claim reclaim and bounded queue closure.

Acceptance:

- [x] Tests cover interruption/retry behavior already present in the Director suite
- [x] Retry/cooldown thresholds are config fields instead of only engine constants
- [x] Recovery behavior is explicit in queue claims, staged-download resume checks, and startup recovery filtering
- [x] Capture one fresh live recovery/resume proof with the coordinator path (`engine_pipeline_cli --resume --limit 1 --skip-fingerprint-backfill --skip-post-sync --skip-organize-subset` on 2026-04-06; stale seeded claim reclaimed and row processed)

### [P1] [done] Raise packaging and clean-machine confidence

Why:

- "Builds in this workspace" is not the same as "ready to ship."

Acceptance:

- [x] Install/build steps documented for a clean environment (`docs/CLEAN_MACHINE_CHECKLIST.md`, `docs/RELEASE_CHECKLIST.md`)
- [x] Gaps and assumptions recorded (`docs/CLEAN_MACHINE_CHECKLIST.md` Known Gap + `docs/RELEASE_CHECKLIST.md` Known Gaps)
- [x] Release checklist updated (`docs/RELEASE_CHECKLIST.md`)
- [x] Trust-spine verification script exists (`scripts/verify_trust_spine.ps1`)
- [x] `cargo tauri build` produces `.msi` and `.exe` installers — `default-run = "cassette"` added to `src-tauri/Cargo.toml` (2026-04-03)

### [P1] [done] Execute music-first system plan (Phase 1 spine)

Why:

- The core engine has deep capability, but UX still under-expresses it as one coherent system.
- Current direction is explicit: player as front door, acquisition as engine, librarian/organizer as stewards.

Reference:

- `docs/MUSIC_FIRST_SYSTEM_EXECUTION_PLAN.md`

Acceptance:

- [x] Home route is music-first with "while you were away" background summary
- [x] Always-visible compact system health strip exists (provider/service/queue/scan status)
- [x] Artist becomes default library worldview with improved featured-artist normalization behavior
- [x] Downloads reorganized into Missing/In Progress/Blocked/Completed lanes with plain-language status
- [x] No modal/popup spam for normal background automation
- [x] Existing playback/download behavior remains intact under baseline verification

Follow-on:

- [ ] Phase 2 universal context-action surfaces (artist/album/track) sequenced after Phase 1 passes

### [P1] [done] Execute music-first system plan (Phase 0 contracts)

Why:

- The feature plan was strong but under-specified your true intent as an autonomous personal music system.
- Contract-first alignment prevents drift into disconnected feature work.

Reference:

- `docs/MUSIC_FIRST_SYSTEM_EXECUTION_PLAN.md` (Operating Contract, Experience Contract, Success Metrics)

Acceptance:

- [x] Operating Contract is treated as a hard gate for new UX/core work
- [x] Experience Contract is reflected in Phase 1 deliverables
- [x] "While you were away" narrative schema is defined and ready for implementation
- [x] Collection-intelligence taxonomy is fixed and reflected in downloads planning
- [x] KPI stubs exist for time-to-music, unchanged-file skip rate, auto-resolution rate, blocked-work visibility, intervention frequency, and explainability

### [P1] [done] Operationalize CPU-first startup scan and deferred GPU enrichment lane

Why:

- The performance direction is clear: high-throughput startup/background scanning should saturate CPU + I/O lanes, while GPU work should be reserved for enrichment tasks where acceleration is real.
- This closes the gap between current contract-level planning and concrete runtime execution policy.

Reference:

- `docs/MUSIC_FIRST_SYSTEM_EXECUTION_PLAN.md` (Execution Insight: CPU-First Scan, GPU-Deferred Enrichment)

Acceptance:

- [x] Startup/background scan path uses persistent incremental diffing and deterministic resume checkpoints
- [x] Unchanged-file skip behavior is measurable and recorded in telemetry captures
- [x] GPU work is routed to deferred enrichment queues (for example BPM/key/embedding lanes), not core scan/hash loops
- [x] Background saturation does not regress return-to-music responsiveness
- [x] WAL + queue claim behavior remains deterministic under load

Evidence (2026-04-06):

- Fresh multi-run capture recorded under `artifacts/perf/run-20260406-160911/results.json` (`-Runs 3 -WarmupRuns 1`).
- `scan_resume_queue_only` median/p95 is now sub-second while resume checkpoints keep startup in queue-only mode.
- Live resume probe showed deterministic checkpoint fast-path: `files_scanned=0` / `files_upserted=0` with `local_files=46503` known rows.
- Core scan/coordinator loops remain CPU+I/O lanes; GPU-heavy enrichment stays deferred/out-of-band from startup scan paths.

### [P1] [done] Execute Music OS Stage A convergence backbone

Why:

- The next strategic step is not isolated feature work; it is cross-layer convergence that turns Cassette into one coherent Music OS.
- Stage A establishes shared primitives (trust ledger, edition intelligence, policy profiles) that multiple later capabilities depend on.

Reference:

- `docs/CASSETTE_MUSIC_OS_IMPLEMENTATION_PLAN.md` (Stage A: Contract and Data Foundations)

Acceptance:

- [x] Trust Ledger v1 is implemented with queryable reason-coded mutation evidence across planner, director, and gatekeeper outcomes
- [x] Edition intelligence markers are threaded through request contract, planner filters, and runtime track inspection surfaces
- [x] Policy profiles (Playback-First, Balanced Auto, Aggressive Overnight) are implemented with deterministic, logged runtime behavior changes
- [x] Stage A outputs are reflected in Downloads/Home explainability and settings controls
  Trust Ledger reason cards are now surfaced in Downloads and Home; Edition Intelligence hints now surface in Downloads request rows and Library track inspection; Settings now exposes policy profile controls with immediate director hot-reload.
- [x] Stage A verification evidence is captured in `PROJECT_STATE.md` and `TELEMETRY.md`
  Trust Ledger, Edition Intelligence, and Policy Profile runtime verification evidence is captured and passing (`cargo check --workspace`, `cargo test --workspace`, `ui/npm run build`, and `scripts/smoke_desktop.ps1`).

### [P1] [done] Deliver modular desktop UX modernization (Winamp-inspired, concept-3 visual direction, no Lyra surfaces)

Why:

- Current UX hardening needs a single coherent target style and interaction model, not route-by-route tweaks.
- The redesigned shell must remain operationally dense while improving discoverability, keyboard-first flow, and Windows desktop integration.

Acceptance:

- [x] Iteration-1 implementation artifact created: `docs/UX_MODERNIZATION_ITERATION_01.md`
- [x] A modular UI shell exists with reusable boundaries for navigation, player, command palette, and feature panels
- [x] Visual language follows the Winamp-inspired plus concept-3 direction across Library, Downloads, and Settings
- [x] Lyra assistant surfaces and prompt language are removed from the active desktop UX path
- [x] Windows command system is implemented with centralized command registry and shortcut mappings for top actions
- [x] Minimized player mode exists as a persistent compact presence with restore behavior
- [x] Windows taskbar integration is wired for playback controls and correct minimize/restore behavior
- [x] Keyboard-first accessibility and focus visibility are preserved in redesigned flows
- [x] Route-level refactors keep desktop integration logic outside page components

### [P1] [done] Formalize performance baseline and regression budget

Why:

- The repo has qualitative confidence, but no strict performance contract yet.

Acceptance:

- [x] Core commands benchmarked or timed (scan, organize, validation, bounded coordinator run)
- [x] Baselines recorded in `TELEMETRY.md`
- [x] Regression thresholds documented

### [P1] [done] Audit and correct tool-role documentation drift

Why:

- Tool roles were drifting between docs, settings labels, and runtime behavior.
- Planning against stale tool ownership is now a bigger risk than adding one more provider.

Acceptance:

- [x] `TOOL_AND_SERVICE_REGISTRY.md` matches current runtime truth
- [x] Canonical docs explicitly mark MusicBrainz as identity spine and Spotify as intent seed
- [x] Research/reference docs that diverge are marked non-canonical

### [P1] [done] Unify Spotify ingest lanes into one identity-first import path

Why:

- Spotify history summary import and direct desired-track import previously had different identity fidelity.
- ISRC-first reconciliation only works if ingest actually feeds the schema.

Acceptance:

- [x] Shared Spotify payload parser handles direct desired-track import shapes
- [x] Direct import now persists `source_track_id`, `source_album_id`, `source_artist_id`, `duration_ms`, best-effort `isrc`, and raw payload JSON
- [x] Album-summary queueing and direct desired-track intake share one canonical operator story
- [x] Replay proof shows improved reconciliation hit-rate on a fixed sample

Replay proof (2026-04-06):

- Seeded paired fixed sample in sidecar (`n=50` each) from deterministic runtime track IDs:
  - `spotify_replay_legacy`: minimal identity fields (artist + title only)
  - `spotify_replay_rich`: richer identity fields (artist + album + title + track/disc + duration)
- Ran bounded reconcile pass via `engine_pipeline_cli --resume --limit 0 --skip-post-sync --skip-organize-subset --skip-fingerprint-backfill`.
- Aggregated outcomes:
  - `spotify_replay_legacy`: `weak_match=50`
  - `spotify_replay_rich`: `strong_match=50`
- Seed rows were cleaned from `desired_tracks`, `reconciliation_results`, and `delta_queue` after capture.

### [P1] [done] Route all album expansion through the resilient resolver

Why:

- Album queueing had split logic and a MusicBrainz-only bias even though the fallback resolver already existed.

Acceptance:

- [x] Tauri album queueing uses the shared resolver (`MusicBrainz -> iTunes -> Spotify`)
- [x] `engine_pipeline_cli --import-spotify-missing` uses the shared resolver
- [x] `batch_download_cli` uses the shared resolver
- [x] Regression tests prove the shared resolver is the only album expansion path

### [P1] [done] Separate search owners from execution owners

Why:

- Torrent and Usenet lanes were blurring search and execution responsibilities.
- Clean ownership is required before a real planner stage is worth building.

Acceptance:

- [x] Jackett is the canonical torrent search owner in the Director
- [x] Real-Debrid direct search is disabled by default in the Director
- [x] `torrent_album_cli` only uses apibay behind an explicit fallback flag
- [x] SABnzbd completion now consults queue/history APIs before filesystem fallback

### [P1] [in_progress] Promote canonical identity and source-alias persistence to the active path

Why:

- The control-plane schema can already carry much richer identity than some active intake/queue boundaries provide.
- The remaining weak point is release-group planning and queue-boundary discipline, not raw provider count.

Acceptance:

- [x] Runtime and sidecar persist canonical artist/release/recording and alias-evidence surfaces
- [x] Shared Spotify import now carries richer source IDs and best-effort ISRC
- [x] Active queue/request boundaries now preserve richer source-track/source-album/source-artist identity when available
- [x] Release-group identity is carried and queryable where planning needs edition-level decisions — **DONE 2026-04-06**: request signatures now include `musicbrainz_release_group_id`, and request alias persistence records `musicbrainz.release_group_id` for planner and director request boundaries.
- [x] No active queue boundary collapses back to `artist + title + optional album` when richer identity is already known — **DONE 2026-04-06**: regression coverage now proves release-group-only identity differences produce distinct request signatures and survive evidence/alias persistence.

### [P1] [done] Introduce a planner stage before byte acquisition

Why:

- Candidate search, memory reuse, review, and policy still sit too close to direct acquisition.
- The planner now supports review mutations and submit-on-approval for song and album/artist expansion queue submissions, and the `plan_and_submit` function is live in the canonical coordinator binary.

Acceptance:

- [x] Search/planning and byte acquisition are now distinct stages in the command surface
- [x] Candidate sets are persisted before acquire starts
- [x] Rationale can be queried before acquire begins
- [x] Review/policy APIs exist for approval, rejection, and rationale
- [x] Live coordinator proof captured: `engine_pipeline_cli --resume --limit 5 --skip-organize-subset --skip-post-sync` ran without crash or panic; planner path in binary confirmed live (2026-04-06)

### [P1] [done] Retire acquisition bypass lanes after planner cutover

Why:

- `batch_download_cli` and direct backlog submission paths still bypass the future planner surface.
- Those shortcuts are useful operator tools, but they should stop defining the product story.

Acceptance:

- [x] Bypass lanes are demoted, removed, or explicitly marked as operator-only debt
- [x] Canonical planner path is the default for UI/runtime queue submission

### [P1] [done] Reuse persisted provenance and candidate memory in runtime behavior

Why:

- The active runtime now persists request signatures, candidate sets, provider search outcomes,
  and negative-result memory.
- The product still behaves as if that memory is mostly write-only.

Acceptance:

- [x] At least one user-visible surface explains prior candidate or provider outcomes (debug panel: per-provider success/fail counts, recent task results with disposition and error)
- [x] Useful provider evidence is retained in normalized runtime tables instead of only nested `result_json` blobs (`provider_search_evidence`, `provider_candidate_evidence`, `provider_response_cache`, `identity_resolution_evidence`, `source_aliases`)
- [x] Failed terminal history rows retain provider attribution and `failure_class`
- [x] At least one runtime path reuses persisted search/candidate memory before re-querying
- [x] Exclusion or negative-result memory is wired into a real decision path

### [P1] [done] Accumulate librarian fingerprint evidence without full-library reruns

Why:

- Gatekeeper could already compute fingerprints, but the librarian/control-plane side still did not
  retain that identity evidence as a first-class fact.
- The bounded fix is to persist `acoustid_fingerprint` in `local_files`, track per-file backfill
  attempt state, and backfill in small deterministic slices during sync instead of treating it as
  throwaway validation output.

Acceptance:

- [x] `local_files` persists `acoustid_fingerprint`
- [x] Gatekeeper admission writes `acoustid_fingerprint` back into `local_files`
- [x] Librarian sync exposes a bounded fingerprint backfill path
- [x] Regression coverage proves missing fingerprints are backfilled and stored
- [x] Unchanged fingerprint failures are suppressed instead of retried every sync
- [x] File mtime changes invalidate stale fingerprint state so rewritten files can be re-backfilled

### [P1] [done] Clean the remaining warning budget

Acceptance:

- [x] `cargo check --workspace` is warning-free
- [x] Real-Debrid dead fields resolved
- [x] CLI bin warnings caused by `state.rs` inclusion removed

### [P1] [done] Repair `cargo test --workspace` on Windows

Why:

- The old Windows failure was in the Tauri lib-test harness startup path, not in the underlying pure Rust logic.
- The fix was to move pure `src-tauri` assertions into `src-tauri/tests/pure_logic.rs` and stop treating the Tauri-linked lib harness as a workspace test dependency.

Acceptance:

- [x] Root cause identified and documented
- [x] `cargo test --workspace` passes again, or the test split is deliberately redesigned and documented

### [P1] [done] Add provider health awareness to the Director waterfall

Acceptance:

- [x] Health state is tracked per provider with a timestamp
- [x] The waterfall can skip known-down providers
- [x] Health changes are visible to the UI or logs

### [P1] [done] Resolve `downloader/` vs `director/providers/` overlap

Acceptance:

- [x] Decision recorded: `director/providers/` is the active runtime acquisition path
- [x] Dead code removed or clearly marked as historical
- [x] Module status in `PROJECT_INDEX.md` updated

### [P1] [done] Deepen active-runtime provenance persistence

Acceptance:

- [x] Candidate-set and candidate-item tables exist in the active runtime DB
- [x] Provider search outcomes and provider-negative memory persist on normalized request signatures
- [x] Terminal save path is transactional and covered by tests

---

## P2

### [P1] [done] Add canonical release identity persistence and a stronger request contract

Why:

- The next architecture step is no longer "invent a new pipeline."
- It is making the existing pipeline more exact about what it is trying to acquire and how identity
  is persisted across providers.

Acceptance:

- [x] Request contract supports more than `artist + title + optional album`
- [x] Runtime schema now includes canonical artist/release/recording and alias persistence surfaces (`canonical_artists`, `canonical_releases`, `canonical_recordings`, `source_aliases`)
- [x] MusicBrainz-backed artist/release-group/release/recording identity persistence plan is documented (`docs/REQUEST_CONTRACT_IDENTITY_PLAN.md`)
- [x] Follow-on implementation scope is recorded in `WORKLIST.md`
- [x] Release-group identity is used as a first-class planner decision/rationale lane
- [x] Command-boundary contract tests cover all request scopes and policy fields

### [P2] [done] Resolve `Album.id` stability

Why:

- Album and artist IDs were generated with Rust `DefaultHasher`, which is seeded and not
  stable across process restarts. This could invalidate UI selection state and request links.

Acceptance:

- [x] Decision recorded in `DECISIONS.md`
- [x] Deterministic stable IDs now exist for album/artist surfaces via BLAKE3-derived IDs
- [x] Regression tests prove IDs are stable across DB reopen

### [P2] [done] Implement `MetadataRepairOnly` acquisition strategy

Why:

- `MetadataRepairOnly` now resolves matching local tracks from runtime DB identity fields and
  applies in-place metadata repair without acquisition.

Acceptance:

- [x] Implemented in Director engine with runtime DB-backed local track matching
- [x] Tests cover missing runtime DB path, no-match failure, and successful repair path

### [P2] [done] Document and test long-session desktop behavior

Why:

- Media apps earn trust through stability over time, not just one clean smoke run.

Acceptance:

- [x] Soak-test procedure documented (`docs/SOAK_TEST_PROCEDURE.md`)
- [x] Baseline evidence run captured and known pain points recorded (`docs/SOAK_EVIDENCE.md`)

### [P2] [done] Integrate Cover Art Archive after canonical release selection

Acceptance:

- [x] Runtime artwork fallback now covers broader sibling-art names plus embedded-art cache extraction
- [x] Artwork fetch is tied to canonical release choice, not ad hoc provider metadata
- [x] Tag/embed flow documents when Cover Art Archive is used

### [P2] [done] Add a bounded lyrics refresh policy on top of the runtime lyrics cache

Acceptance:

- [x] Synced/plain lyrics now persist durably in the runtime DB and are reused before LRCLIB refetch
- [x] Cache refresh/expiry policy is explicit and documented
- [ ] Optional prefetch lane exists for recently played or newly finalized tracks if it is still worth the complexity

### [P2] [done] Add a bounded runtime canonical backfill lane for older tracks

Acceptance:

- [x] Existing runtime `tracks` rows missing canonical artist/release IDs can be backfilled without a full-library rewrite
- [x] Backfill ordering is deterministic and bounded
- [x] Startup integration logs failures instead of aborting app boot

### [P2] [done] Surface runtime MusicBrainz identity in the desktop UI

Acceptance:

- [x] Library inspection exposes persisted MusicBrainz recording/release IDs
- [x] Canonical artist/release IDs are visible in the active desktop runtime
- [x] No extra DB or network fetch is required just to inspect already persisted identity

### [P2] [done] Make bundled slskd part of the desktop runtime lifecycle

Acceptance:

- [x] Cassette attempts to start bundled `slskd.exe` during desktop startup when the endpoint is not already reachable
- [x] Settings exposes runtime status plus refresh/restart/stop controls
- [x] Smoke tooling is updated so it exercises the managed `slskd` startup contract instead of checking port `5030` in isolation

### [P2] [done] Prove and document Discogs and Last.fm enrichment behavior end-to-end

Acceptance:

- [x] Canonical docs and reference docs consistently reflect current Discogs/Last.fm runtime behavior
- [x] Bounded end-to-end proof captured for enrichment outcomes in active flows

Proof: `enrich_probe_cli` ran against live runtime DB on 2026-04-06. Both clients implemented and wired; credentials not configured at proof time so 0 tracks probed, 0 hits — binary ran correctly and reported cleanly. `PROJECT_STATE.md` and `TOOL_AND_SERVICE_REGISTRY.md` updated with proof artifact.

### [P2] [done] Clarify Bandcamp scope as payload URL resolver and decide next-step ownership

Acceptance:

- [x] Docs explicitly state Bandcamp currently resolves payload-provided URLs only
- [x] Follow-up decision recorded: expand to full provider path or keep resolver-only scope (see `DECISIONS.md`, Decision 33)

### [P2] [done] Tighten metadata and enrichment operating story

Why:

- Metadata logic exists, but runtime ownership and lifecycle are still less explicit
  than core library flows.

Acceptance:

- [x] Current enrichment behavior documented
- [x] Future integration plan recorded without overstating readiness

---

## P3

### [P3] [done] Add richer provider health and troubleshooting views in UI

Acceptance:

- [x] Downloads command center shows a provider troubleshooting snapshot with down/unknown totals
- [x] Per-provider diagnostics now show status, last-check timestamp, runtime message, and actionable hint text
- [x] Troubleshooting hints incorporate provider configuration state and slskd runtime readiness

### [P3] [done] Revisit broader release automation once packaging proof is stable

Acceptance:

- [x] Manual release-candidate workflow exists (`.github/workflows/release-candidate.yml`)
- [x] Workflow runs CI gate and packaging, then uploads installers plus SHA256 manifest
- [x] Optional perf-gate path is available before packaging

### [P3] [done] Improve artist deep-link from library page

The earlier navigation gap has been fixed. Keep this here as history until a later cleanup pass
removes completed P3 items from the active backlog view.

---

## Completed Highlights

- Deezer full-track acquisition is live-proven on this machine.
- Pending-task startup recovery is proven through `recovery_probe_cli`.
- `engine_pipeline_cli` now uses durable `delta_queue` claims with sidecar scan checkpoints and `full|resume|delta-only` scan modes.
- `tag_rescue_cli` now performs staged DB repair via `embedded_tag`, `filename_prefix`, and `album_pattern`, with unresolved-row reporting.
- `organize_cli --live` now aborts on suspicious mass `00 - ...` renames.
- `director/providers/` is the canonical runtime acquisition path; `downloader/` is compatibility-only.
- Active runtime provenance now persists request signatures, candidate sets, provider searches, provider attempts, and negative-result memory.

---

## Operating Notes For Agents

When you pick up a task:

1. Update status from `todo` to `in_progress` if you are actively working it.
2. Keep the task scoped.
3. Add linked file paths or commands if you discover the task is narrower than written.
4. Move it to `review` only after verification.
5. Mark `done` only after code and docs both reflect reality.

If you notice a new problem but are not fixing it now, add it here with enough context for
the next agent to act without rediscovery.
