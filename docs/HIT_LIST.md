# Cassette Hit List

Last updated: 2026-03-31

This is the short-form execution board for the current mission.
For full context and acceptance details, see `TODO.md` and `PROJECT_STATE.md`.

---

## Results Snapshot

Current audited backlog shape:

- [x] 10 completed items
- [ ] 12 remaining items
- [x] Green verification snapshot recorded on 2026-03-30
- [x] Runtime/control-plane split documented: `cassette.db` + `cassette_librarian.db`

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
- [ ] Prove audit completeness across organization and admission flows

### P1 Next

- [ ] Raise packaging and clean-machine confidence
- [ ] Formalize performance baselines and regression budgets
- [ ] Reuse persisted provenance and candidate memory in runtime behavior
- [ ] Capture one fresh live recovery/resume proof with the coordinator path as part of async hardening

### P2 After That

- [ ] Add canonical release identity persistence and a stronger request contract
- [ ] Resolve `Album.id` stability
- [ ] Implement or remove `MetadataRepairOnly`
- [ ] Document and test long-session desktop behavior
- [ ] Tighten metadata and enrichment operating story

### P3 Later

- [ ] Add richer provider health and troubleshooting views in UI
- [ ] Revisit broader release automation once packaging proof is stable

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
