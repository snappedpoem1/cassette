# Cassette Remaining Work Order

Last updated: 2026-04-07
Owner: Christian (single-owner personal project)

Current execution status:

- WO-01: complete
- WO-02: complete
- WO-03: complete (candidate review UX, approve/reject rationale path, explicit provider exclusion toggles, and exclusion-memory reuse are implemented and validated)
- WO-04: complete (credentialed bounded probe captured: 25 tracks probed, Discogs 25/25 hits, Last.fm 0/25 context hits on sampled corpus)
- WO-05: complete (fresh perf capture + regression gate evidence recorded; telemetry policy and cadence updated)

Open work-order count: 0

Purpose:

- Preserve closure evidence for the latest work-order wave.
- Keep a compact record of what closed and what follow-on hardening still remains outside this completed wave.

## Closed Work-Order Board

| Work Order | Status | Theme |
| --- | --- | --- |
| WO-01 | Complete | Canonical docs truth-sync |
| WO-02 | Complete | Proof-status drift cleanup |
| WO-03 | Complete | Candidate-review and exclusion-memory closure |
| WO-04 | Complete | Credentialed enrichment proof closure |
| WO-05 | Complete | Telemetry maturity closure |

No work orders remain open in this sheet. Active follow-on work is tracked in `docs/TODO.md`, `docs/HIT_LIST.md`, and `docs/TELEMETRY.md`.

## Now

### WO-01: Canonical documentation truth-sync

Why now:

Execution docs currently disagree on what is open versus done, which risks rework.

Deliverables:

1. Reconcile open/closed status across TODO, HIT_LIST, WORKLIST, PROJECT_STATE, and TELEMETRY.
2. Remove or reclassify stale unchecked items in WORKLIST that are already completed elsewhere.
3. Correct stale remaining-item summary counts in HIT_LIST.

Verification commands:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\check_docs_state.ps1
if (Get-Command rg -ErrorAction SilentlyContinue) { rg -n "\[ \]|in_progress|Known gaps|Known Limitations" docs/TODO.md docs/HIT_LIST.md docs/WORKLIST.md docs/PROJECT_STATE.md docs/TELEMETRY.md } else { Select-String -Path docs/TODO.md,docs/HIT_LIST.md,docs/WORKLIST.md,docs/PROJECT_STATE.md,docs/TELEMETRY.md -Pattern "\[ \]|in_progress|Known gaps|Known Limitations" }
```

Docs to update:

- docs/WORKLIST.md
- docs/HIT_LIST.md
- docs/TODO.md
- docs/PROJECT_STATE.md
- docs/TELEMETRY.md

Done criteria:

- No stale unchecked item remains for work already proven complete.
- Status and ordering are mutually consistent across all canonical backlog/state docs.

### WO-02: Proof-status drift cleanup in limitations sections

Why now:

Limitations text currently conflicts with newer evidence, reducing trust in the canonical state.

Deliverables:

1. Remove or re-scope contradictory pending statements in PROJECT_STATE known limitations.
2. Remove or re-scope contradictory pending statements in TELEMETRY known gaps.
3. Resolve in-progress status in TODO where acceptance is already complete.

Verification commands:

```powershell
if (Get-Command rg -ErrorAction SilentlyContinue) { rg -n "still pending|still no|single measured run|single smoke run|in_progress" docs/PROJECT_STATE.md docs/TELEMETRY.md docs/TODO.md } else { Select-String -Path docs/PROJECT_STATE.md,docs/TELEMETRY.md,docs/TODO.md -Pattern "still pending|still no|single measured run|single smoke run|in_progress" }
powershell -ExecutionPolicy Bypass -File scripts\check_docs_state.ps1
```

Docs to update:

- docs/PROJECT_STATE.md
- docs/TELEMETRY.md
- docs/TODO.md

Done criteria:

- Every pending/limitation statement aligns with latest evidence or is explicitly scoped as a genuine remaining gap.

## Next

### WO-03: Candidate-review and exclusion-memory closure

Why now:

This is the largest remaining runtime/UX debt still identified in canonical docs.

Deliverables:

1. Define final UX and command contract for candidate review and exclusions.
2. Implement candidate-review and exclusion-memory path end-to-end.
3. Add targeted tests and runtime proof entries.
4. Mark closure in canonical docs.

Verification commands:

```powershell
cargo test --workspace
Set-Location ui; npm run build; Set-Location ..
.\scripts\smoke_desktop.ps1
```

Docs to update:

- docs/WORKLIST.md
- docs/PROJECT_STATE.md
- docs/TODO.md
- docs/HIT_LIST.md

Done criteria:

- Candidate-review and exclusion-memory path is defined, implemented, tested, and proven in docs.

### WO-04: Credentialed Discogs/Last.fm enrichment proof closure

Why now:

Credentialed bounded probe evidence is now captured and can be treated as closed baseline.

Deliverables:

1. Run bounded credentialed enrichment probe with non-zero track sample.
2. Record expected outcomes and fallback behavior.
3. Update status wording to distinguish implemented behavior from credentialed live proof.

Verification commands:

```powershell
cargo run --bin enrich_probe_cli -- --limit 25
cargo test --workspace
```

Docs to update:

- docs/PROJECT_STATE.md
- docs/TELEMETRY.md
- docs/TODO.md

Done criteria:

- At least one credentialed non-zero enrichment probe is captured and reflected in canonical docs.

Closure evidence (2026-04-07):

- `enrich_probe_cli --limit 25` against live runtime DB reported `25 tracks probed | Discogs hits 25/25 | Last.fm hits 0/25` with credentials configured.

## Later

### WO-05: Telemetry maturity closure

Why now:

Closed with repeatable artifact/gate execution and telemetry cadence hardening.

Deliverables:

1. Define repeatable packaging-confidence telemetry artifact policy.
2. Promote KPI stubs into captured metrics with cadence.
3. Keep telemetry known-gap list strictly limited to active gaps.

Verification commands:

```powershell
.\scripts\verify_trust_spine.ps1
scripts\perf_baseline_capture.ps1 -Runs 3 -WarmupRuns 1
scripts\perf_regression_gate.ps1 -CandidateResultPath <artifact>
```

Docs to update:

- docs/TELEMETRY.md
- docs/HIT_LIST.md
- docs/TODO.md

Done criteria:

- Packaging and KPI telemetry become repeatable evidence artifacts with clear gate usage.

Closure evidence (2026-04-07):

- `scripts/verify_trust_spine.ps1` passed (workspace checks/tests, UI build, strict smoke).
- `scripts/perf_baseline_capture.ps1 -Runs 3 -WarmupRuns 1` produced `artifacts/perf/run-20260406-232508/results.json`.
- `scripts/perf_regression_gate.ps1 -CandidateResultPath artifacts/perf/run-20260406-232508/results.json` passed all scenarios with no fail-level regressions.

## Immediate Docs Consistency Fixes

1. HIT_LIST summary count says items remain while nearly all tracked lanes are complete.
2. TODO includes an in-progress item whose acceptance list is already fully checked.
3. WORKLIST contains unchecked items that are already marked complete in TODO/HIT_LIST/PROJECT_STATE.
4. PROJECT_STATE known limitations include statements that conflict with Stage C/Stage D completion and current runtime behavior.
5. TELEMETRY known gaps still include statements that conflict with current multi-run and soak evidence.
6. TODO last-updated date lags behind entries already stamped with 2026-04-07 completion updates.
