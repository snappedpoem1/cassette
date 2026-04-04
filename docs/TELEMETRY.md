# Cassette Telemetry

This file tracks what we know about build, runtime, and operational confidence.
It is not a dashboard — it is a record of observed facts with dates.

**Last Updated**: 2026-04-03

---

## Build Baseline (as of 2026-03-27)

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
| Qobuz acquisition | Partial | Wired and provider probe passes on this machine |
| Deezer acquisition | Working | Live full-track FLAC probe succeeded on 2026-03-27 |
| slskd acquisition | Partial | Health/probe path passes when daemon is up; transfer acceptance still varies by peer |
| Usenet acquisition | Partial | SABnzbd handoff wired; end-to-end not formally proven |
| Jackett torrent search | Partial | Active in Director and CLI, but still needs broader live proof |
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

Baseline capture (2026-04-03, machine `DESKTOP-8TK5EVK`, 16 logical processors, 1 run, 0 warmup):

| Scenario | Median (s) | P95 (s) | Command |
|---|---:|---:|---|
| `scan_resume_queue_only` | 1.546 | 1.546 | `cargo run -p cassette --bin engine_pipeline_cli -- --resume --limit 0 --skip-post-sync --skip-organize-subset --skip-fingerprint-backfill` |
| `validation_targeted_suite` | 1.227 | 1.227 | `cargo test -p cassette-core validation::logging::tests:: -- --nocapture` |
| `bounded_coordinator_limit5` | 1.552 | 1.552 | `cargo run -p cassette --bin engine_pipeline_cli -- --resume --limit 5 --skip-post-sync --skip-organize-subset --skip-fingerprint-backfill` |
| `organize_dry_run` | 14.748 | 14.748 | `cargo run -p cassette --bin organize_cli -- --dry-run` |

Regression budget policy:

- Warning threshold and fail threshold are enforced per scenario in `docs/perf/BUDGETS.json`.
- Release gate fails if candidate median or P95 exceeds the fail threshold.
- Release gate command: `scripts/perf_regression_gate.ps1 -CandidateResultPath <artifact>`.
- Baseline promotion only happens after candidate gate passes.

---

## Metrics To Track Going Forward

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

---

## Known Gaps

- Baseline currently uses a single measured run per scenario; move to multi-run captures (`-Runs 3` minimum) before final release lock.
- Provider reliability is configuration-dependent and machine-dependent.
- Packaging confidence is not yet a repeatable telemetry artifact.
- Long-session stability has not been tested beyond a single smoke run.
- Full UI-driven crash/relaunch capture is still worth recording even though startup replay is now proven with a deterministic probe.
- Candidate persistence and provider-response memory are in the runtime path, but planner-stage reuse and explainability are not fully surfaced yet.

---

## Update Policy

Update this file when:

- a benchmark is added or re-run
- a command meaningfully slows down or speeds up
- a new reliability gate is introduced
- a confidence claim is verified or disproven
- a provider's status changes in a material way
