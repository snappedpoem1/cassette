# Cassette Telemetry

This file tracks what we know about build, runtime, and operational confidence.
It is not a dashboard — it is a record of observed facts with dates.

**Last Updated**: 2026-04-07

---

## Build Baseline (as of 2026-03-27)

Recent docs/runtime evidence alignment (2026-04-06):

- Music-first Phase 0 and Phase 1 plan execution status is reflected in `PROJECT_STATE.md`, `TODO.md`, and `HIT_LIST.md`.
- P1 tracking now includes explicit CPU-first startup scan plus deferred GPU enrichment operationalization.
- Trust Ledger v1 now derives request reason codes from existing planner/director/gatekeeper evidence and exposes a queryable reason-distribution surface for Home/Downloads explainability.
- Edition Intelligence v1 verification now includes runtime/UI coverage: Library inspection shows release-group plus derived edition bucket/markers, and Downloads request rows surface release-group/policy hints.

| Check | Status | Notes |
|---|---|---|
| `cargo check` | Passing | Warning-free on 2026-03-28 |
| `cargo test` | Passing | All unit and integration tests pass |
| `npm run build` (ui) | Passing | Production build clean |
| Desktop smoke (`scripts/smoke_desktop.ps1`) | Passing | Confirmed on developer machine |
| Startup recovery probe (`recovery_probe_cli`) | Passing | Resumed job finalized; stale cancelled row was filtered |

---

## Runtime Confidence

| Area | Status | Notes |
|---|---|---|
| Library scan | Working | Parallel workers, WAL-backed upserts |
| Playback | Working | Symphonia decode + CPAL output, seek confirmed |
| Queue | Working | Persist/restore across sessions |
| Downloads dashboard | Working | Director events surface correctly |
| Pending-task startup recovery | Working | Deterministic startup replay proven with `recovery_probe_cli` |
| Qobuz acquisition | unverified | Latest `provider_probe_cli` run failed auth refresh; credentials/session require refresh |
| Deezer acquisition | local-proven | Live full-track FLAC probe succeeded on 2026-03-27; provider probe still returns search OK |
| slskd acquisition | unverified | Latest `provider_probe_cli` run returned HTTP 403 (daemon reachable, credentials rejected) |
| Usenet acquisition | unverified | `provider_probe_cli` indicates missing `nzbgeek_api_key` and/or `usenet_host`; SAB completion runbook now captured in `docs/LANE_C_PROBE_RUNBOOK.md` |
| Jackett torrent search | unverified | Active in Director/CLI path, but still awaiting broader bounded probe coverage |
| LRCLIB endpoint probe | bounded-probe | Direct probe artifact `docs/probes/lane_c_probe_2026-04-07.json` confirms endpoint reachability with plain+synced lyrics payload |
| Discogs/Last.fm enrichment probe | Working (bounded) | 2026-04-07 credentialed probe (`--limit 25`) reported `Discogs 25/25` and `Last.fm 0/25` on sampled corpus |
| Provider reliability snapshot cadence | Working (repeatable) | `scripts/capture_provider_reliability_snapshot.ps1` now writes timestamped artifacts (latest: `docs/probes/provider_probe_2026-04-07_174445.txt`, `docs/probes/lane_c_probe_2026-04-07_174445.json`) |
| Cleanroom packaging verification | Working (local) | 2026-04-07 `scripts/verify_cleanroom_local.ps1` passed in DisposableProfile mode; installer bundle and runtime/sidecar DB checks passed |
| yt-dlp acquisition | Wired | Depends on yt-dlp binary in PATH |
| Spotify import | Working | JSON export parsing and album queue confirmed |
| Organizer / duplicate finder | Working | Dry-run confirmed |
| Tag fixes | Working | Propose + apply flow confirmed |
| Playlists | Working | CRUD and playback confirmed |

---

## Performance Baseline Contract

Baseline artifacts:

- `docs/perf/BASELINE.latest.json`
- `docs/perf/BUDGETS.json`
- `scripts/perf_baseline_capture.ps1`
- `scripts/perf_regression_gate.ps1`

Baseline capture (2026-04-06, machine `DESKTOP-8TK5EVK`, 16 logical processors, 3 runs, 1 warmup):

| Scenario | Median (s) | P95 (s) | Command |
|---|---:|---:|---|
| `scan_resume_queue_only` | 0.847 | 0.847 | `cargo run -p cassette --bin engine_pipeline_cli -- --resume --limit 0 --skip-post-sync --skip-organize-subset --skip-fingerprint-backfill` |
| `validation_targeted_suite` | 0.890 | 0.890 | `cargo test -p cassette-core validation::logging::tests:: -- --nocapture` |
| `bounded_coordinator_limit5` | 0.829 | 0.829 | `cargo run -p cassette --bin engine_pipeline_cli -- --resume --limit 5 --skip-post-sync --skip-organize-subset --skip-fingerprint-backfill` |
| `organize_dry_run` | 7.472 | 7.472 | `cargo run -p cassette --bin organize_cli -- --dry-run` |

Latest capture (2026-04-07, artifact `artifacts/perf/run-20260406-232508/results.json`, 3 runs, 1 warmup):

| Scenario | Median (s) | P95 (s) | Gate result |
|---|---:|---:|---|
| `scan_resume_queue_only` | 0.860 | 0.860 | pass |
| `validation_targeted_suite` | 0.763 | 0.763 | pass |
| `bounded_coordinator_limit5` | 0.861 | 0.861 | pass |
| `organize_dry_run` | 10.720 | 10.720 | pass |

Regression gate outcome (2026-04-07):

- Candidate artifact `artifacts/perf/run-20260406-232508/results.json` passed `scripts/perf_regression_gate.ps1` with no fail-level regressions.

Queue-only unchanged-skip evidence (2026-04-06):

- Live resume probe reported: `files_scanned=0`, `files_upserted=0`, mode=`queue-only` with completed checkpoints.
- Sidecar denominator at probe time: `local_files=46503`.
- Operational interpretation: startup/background resume stayed in deterministic incremental mode and skipped unchanged files.

Regression budget policy:

- Warning threshold and fail threshold are enforced per scenario in `docs/perf/BUDGETS.json`.
- Release gate fails if candidate median or P95 exceeds the fail threshold.
- Release gate command: `scripts/perf_regression_gate.ps1 -CandidateResultPath <artifact>`.
- Baseline promotion only happens after candidate gate passes.

---

## Metrics To Track Going Forward

### Music-First Experience KPI Stubs

- Time-to-music: launch to first resume/play action from Home
- Unchanged-file skip rate: unchanged files skipped during startup/background scans
- Auto-resolution rate: acquisition or metadata work completed without user intervention
- Blocked-work visibility rate: blocked items surfaced with plain-language reason and next action
- Intervention frequency: review prompts or manual approvals per day
- Trust explainability score: major mutations paired with human-readable outcome summaries
- Trust reason-code distribution: top recent reason codes from `get_trust_reason_distribution` for regression spotting and explainability drift

### Build Health

- Rust compile success
- Rust test success
- UI build success
- Warning count (target: zero)

### Runtime Health

- Desktop smoke success
- Provider status visibility (all providers report a status, even if unconfigured)
- Validation pass/fail for representative sandbox workflows
- Director task history: proportion of `Done` vs `Failed` dispositions

### Performance

- Library scan duration (queue-only and bounded coordinator paths)
- Organize duration (dry-run)
- Validation duration (targeted validation suite)
- App startup time (cold)
- UI render time for large libraries

## KPI Cadence And Artifact Policy

- Capture cadence: run `scripts/perf_baseline_capture.ps1 -Runs 3 -WarmupRuns 1` for release-candidate validation and for any major coordinator/performance-sensitive changes.
- Gate cadence: run `scripts/perf_regression_gate.ps1 -CandidateResultPath <artifact>` on every capture; store candidate artifacts under `artifacts/perf/run-YYYYMMDD-HHMMSS/`.
- Trust cadence: run `scripts/verify_trust_spine.ps1` alongside perf capture to pair behavior correctness with performance evidence.
- Promotion rule: only promote candidate numbers into canonical baseline after a passing regression gate.

---

## Known Gaps

- Baseline now uses multi-run captures (`-Runs 3`, `-WarmupRuns 1`) for core scenarios; next improvement is broader scenario coverage over time.
- Provider reliability is configuration-dependent and machine-dependent.
- Packaging confidence is now policy-defined, script-gated, and locally proven via cleanroom verification; maintain this through repeatable local clean-room runs on this machine.
- Long-session stability now has documented soak procedure and baseline evidence; additional soak depth remains useful as follow-on hardening.
- Full UI-driven crash/relaunch capture is still worth recording even though startup replay is now proven with a deterministic probe.
- Candidate persistence and provider-response memory are in the runtime path, and Trust Ledger + Edition Intelligence + Policy Profiles now surface in Home/Downloads/Library/Settings; planner-stage vocabulary reuse remains incomplete.

---

## Update Policy

Update this file when:

- a benchmark is added or re-run
- a command meaningfully slows down or speeds up
- a new reliability gate is introduced
- a confidence claim is verified or disproven
- a provider's status changes in a material way
