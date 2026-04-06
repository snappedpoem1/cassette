# Cassette Hit List

Last updated: 2026-04-06

This is the short-form execution board for the current mission.
For full context and acceptance details, see `TODO.md` and `PROJECT_STATE.md`.

---

## Results Snapshot

Current audited backlog shape:

- [x] 51 completed items
- [ ] 4 remaining items
- [x] Green verification snapshot recorded on 2026-03-30
- [x] Runtime/control-plane split documented: `cassette.db` + `cassette_librarian.db`

New (2026-04-03):

- [x] Jackett multi-indexer torrent search provider added to Director waterfall (trust_rank 40, between Usenet and Real-Debrid)
- [x] `torrent_album_cli --seed-sidecar` feeds failed albums into sidecar delta_queue for coordinator retry
- [x] `cargo tauri build` produces `.msi` + `.exe` installer bundles
- [x] `db_converge_cli --overwrite` re-proven against app-data DBs (`desired_tracks=4`, `delta_queue=11`, `acquisition_requests=0`)
- [x] Album/artist projection IDs switched to deterministic BLAKE3-derived IDs (replaces seeded `DefaultHasher` behavior)
- [x] Trust-spine now fails fast on native command failures and runs smoke in strict mode
- [x] `MetadataRepairOnly` strategy now performs runtime DB-backed local metadata repair (no longer stubbed)

Already proven:

- [x] Deezer full-track acquisition is live-proven on this machine
- [x] Pending-task startup recovery is proven through `recovery_probe_cli`
- [x] `engine_pipeline_cli` uses durable `delta_queue` claims
- [x] Sidecar scan checkpoints and `full|resume|delta-only` scan modes exist
- [x] `tag_rescue_cli` performs staged DB repair with unresolved-row reporting
- [x] `organize_cli --live` aborts on suspicious mass `00 - ...` renames
- [x] `director/providers/` is the canonical runtime acquisition path
- [x] Active runtime provenance persists request signatures, candidate sets, provider searches, provider attempts, and negative-result memory
- [x] `generate_delta_queue` no longer wipes claimed (mid-flight) rows — only unclaimed unprocessed rows are deleted
- [x] `mark_processed` preserves `claimed_at`/`claim_run_id` as audit trail
- [x] Both behaviors regression-tested in `orchestrator::tests::adapter_tests`

---

## Mission Checklist

### P0 Now

- [x] Capture the first bounded end-to-end `engine_pipeline_cli` live proof — **DONE 2026-03-31**: scan→delta→Qobuz acquisition→`processed_at` stamped→post-run delta-only closes gap. Proof in `PROJECT_STATE.md`.
- [x] Capture coordinator interruption plus `--resume` recovery proof — **DONE 2026-03-31**: stale claims reclaimed (3), checkpoint fast-path (0 files scanned), 3 tracks re-acquired (Qobuz/Deezer), already-finalized row 1 not re-acquired. Proof in `PROJECT_STATE.md`.
- [x] Capture the bounded safe organizer live proof after staged track-number repair — **DONE 2026-03-31**: 23,393 files organized live, 0 zero-track renames, 1 stale-path error handled gracefully. 1,833 unresolved zero-track rows documented (1,371 singles, 389 zero-prefix albums, 73 other — no embedded tag recovery possible).
- [x] Prove audit completeness across organization and admission flows

### P1 Next

- [x] Phase close: all items in this `P1 Next` block are complete as of 2026-04-06

- [x] Execute Music OS Stage A convergence backbone (Trust Ledger v1 + Edition Intelligence v1 + Policy Profiles complete 2026-04-06)
- [x] Wire Stage A outputs into Home/Downloads explainability surfaces and settings profile controls
- [x] Execute `docs/MUSIC_FIRST_SYSTEM_EXECUTION_PLAN.md` Phase 0 contracts (Operating Contract, Experience Contract, KPI stubs, while-you-were-away schema) - **DONE 2026-04-06**: canonical contract docs synced, while-you-were-away schema defined, telemetry KPI stubs recorded.
- [x] Execute `docs/MUSIC_FIRST_SYSTEM_EXECUTION_PLAN.md` Phase 1 spine (music-first home, background summary, status strip, artist-first default, download lanes) - **DONE 2026-04-06**: `/` is now Home, `/library` preserves deep library view, bottom status strip landed, Downloads reorganized into Missing/In Progress/Blocked/Completed lanes.
- [x] Unify Spotify intake operator story - **DONE 2026-04-06**: Import now exposes one Spotify intake surface with history-backed album backlog and direct desired-track JSON feeding the same identity-first control-plane path.
- [x] `cargo tauri build` produces `.msi` + `.exe` — packaging is buildable — **DONE 2026-04-03**: `default-run = "cassette"` added to `src-tauri/Cargo.toml`
- [x] Install/build steps documented for clean environment; release checklist updated — **DONE 2026-04-03**: `docs/CLEAN_MACHINE_CHECKLIST.md` + `docs/RELEASE_CHECKLIST.md`
- [x] Formalize performance baselines and regression budgets — **DONE 2026-04-03**: `scripts/perf_baseline_capture.ps1`, `scripts/perf_regression_gate.ps1`, `docs/perf/BUDGETS.json`, `docs/perf/BASELINE.latest.json`
- [x] Deliver modular desktop UX modernization (Winamp-inspired + concept-3 direction, no Lyra surfaces) with Windows command palette, minimized player mode, and taskbar integration
- [x] Stabilization pass landed for playback auto-start, duplicate handling (including handle-all), tools metadata/ingest clarity, Spotify completeness detection, and artist variant grouping
- [x] Reuse persisted provenance and candidate memory in runtime behavior - **DONE 2026-04-06**: persisted provider memory and candidate evidence now influence runtime search decisions (dead-end skip + cached candidate hydration), with explainability surfaced through Downloads pre-acquisition review and trust/disposition summaries.
- [x] Capture one fresh live recovery/resume proof with the coordinator path as part of async hardening - **DONE 2026-04-06**: seeded stale `delta_queue` claim reclaimed (`Reclaimed 1 stale queue claims`), bounded `--resume --limit 1` run finalized `delta-1-belly of the beast` via Deezer, and stamped `processed_at` while preserving claim audit fields.

### P2 After That

- [x] Phase transition: `P2 After That` is now the active execution phase
- [x] Phase close: all items in this `P2 After That` block are complete as of 2026-04-06

- [x] Add canonical release identity persistence and a stronger request contract — **DONE**
- [x] Resolve `Album.id` stability — **DONE**
- [x] `MetadataRepairOnly` strategy implemented (runtime DB-backed local metadata repair) — **DONE**
- [x] Document and test long-session desktop behavior — **DONE 2026-04-06**: soak procedure and baseline evidence are captured in `docs/SOAK_TEST_PROCEDURE.md` and `docs/SOAK_EVIDENCE.md`.
- [x] Tighten metadata and enrichment operating story — **DONE 2026-04-06**: runtime metadata posture, ownership boundaries, and bounded enrichment behavior documented in canonical state/docs.

### P3 Later

- [x] Phase transition: `P3 Later` is now the active execution phase
- [x] Phase close: all items in this `P3 Later` block are complete as of 2026-04-06

- [x] Add richer provider health and troubleshooting views in UI — **DONE 2026-04-06**: Downloads now exposes provider troubleshooting cards with status totals, provider-specific health messages, config-aware hints, and timestamped health snapshots.
- [x] Revisit broader release automation once packaging proof is stable — **DONE 2026-04-06**: `.github/workflows/release-candidate.yml` adds a manual release-candidate gate (CI verification, optional perf gate, packaging, artifact + SHA256 manifest upload).

### Next Phase Queue (Reopened)

- [x] Operationalize CPU-first startup scan and deferred GPU enrichment lane — **DONE 2026-04-06**: fresh 3-run perf capture recorded (`artifacts/perf/run-20260406-160911/results.json`), queue-only resume probe showed `files_scanned=0` / `files_upserted=0` with `local_files=46503`, and telemetry updated with unchanged-file skip evidence.
- [x] Capture Spotify ingest replay proof for improved reconciliation hit-rate on a fixed sample — **DONE 2026-04-06**: paired fixed sample replay (`n=50` per cohort) upgraded reconciliation strength from `weak_match=50` (legacy-minimal identity) to `strong_match=50` (rich identity fields), then cleaned seed rows from sidecar tables.
- [x] Finish release-group identity threading and prevent identity collapse at all active queue boundaries — **DONE 2026-04-06**: request signatures now include `musicbrainz_release_group_id`, and source-alias persistence now stores `musicbrainz.release_group_id` across planner/director request boundaries with regression tests.
- [x] Complete planner-stage cutover for all remaining runtime/operator lanes — **DONE 2026-04-06**: `plan_and_submit` live in coordinator binary; proof run confirmed planner path executes without crash (`engine_pipeline_cli --resume --limit 5 --skip-organize-subset --skip-post-sync`, no pending delta_queue work at proof time). See `PROJECT_STATE.md` for proof artifact.
- [ ] Prove and document Discogs and Last.fm enrichment behavior end-to-end
- [x] Clarify Bandcamp scope as payload URL resolver and record next-step ownership — **DONE 2026-04-06**: Bandcamp remains resolver-only for payload URLs and ownership scope is recorded in `DECISIONS.md` Decision 33.

---

## Immediate Win Conditions

The mission meaningfully advances when these three boxes are checked:

- [x] One bounded coordinator proof is captured with queue before/after evidence — **DONE 2026-03-31**
- [x] One interrupted coordinator run resumes cleanly with durable scan and queue state — **DONE 2026-03-31**
- [x] One bounded organizer live pass completes without bad `00 - ...` renames — **DONE 2026-03-31**

---

## Source Docs

- `TODO.md` - full backlog and acceptance detail
- `PROJECT_STATE.md` - current runtime truth and proof snapshots
- `WORKLIST.md` - longer-arc architecture sequence
- `DECISIONS.md` - rationale for the current runtime shape
