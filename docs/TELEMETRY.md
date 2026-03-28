# Cassette Telemetry

This file tracks what we know about build, runtime, and operational confidence.
It is not a dashboard — it is a record of observed facts with dates.

**Last Updated**: 2026-03-28

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
| Tidal acquisition | Not started | OAuth device flow not implemented |
| slskd acquisition | Partial | Health/probe path passes when daemon is up; transfer acceptance still varies by peer |
| Usenet acquisition | Partial | SABnzbd handoff wired; end-to-end not formally proven |
| yt-dlp acquisition | Wired | Depends on yt-dlp binary in PATH |
| Spotify import | Working | JSON export parsing and album queue confirmed |
| Organizer / duplicate finder | Working | Dry-run confirmed |
| Tag fixes | Working | Propose + apply flow confirmed |
| Playlists | Working | CRUD and playback confirmed |

---

## Known Performance Observations

No formal benchmarks exist yet. Qualitative observations:

- Library scan on a ~10,000 track collection: order of seconds (not minutes)
- UI responsiveness after scan: responsive; no observed stalls
- App startup time: fast; no measured baseline

**TODO**: Add formal benchmarks and record results here when they are run.

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

### Performance (to be established)

- Library scan duration (by track count)
- Organize duration (by file count)
- Validation duration (by file count)
- App startup time (cold)
- UI render time for large libraries

---

## Known Gaps

- No formal benchmark suite exists yet.
- No numeric regression budget is enforced.
- Provider reliability is configuration-dependent and machine-dependent.
- Packaging confidence is not yet a repeatable telemetry artifact.
- Long-session stability has not been tested beyond a single smoke run.
- Full UI-driven crash/relaunch capture is still worth recording even though startup replay is now proven with a deterministic probe.
- Full candidate-set persistence and negative-result memory are still absent from the active runtime path.

---

## Update Policy

Update this file when:

- a benchmark is added or re-run
- a command meaningfully slows down or speeds up
- a new reliability gate is introduced
- a confidence claim is verified or disproven
- a provider's status changes in a material way
