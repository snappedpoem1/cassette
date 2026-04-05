# Desktop Soak Test Procedure

Last updated: 2026-04-05

## Purpose

Validate long-session desktop reliability (stalls, crashes, command drift, playback degradation) over sustained runtime, not just one-shot smoke runs.

## Scope

- Desktop runtime process stability
- Playback continuity and control responsiveness
- Command palette and global shortcut stability over time
- Tray/taskbar quick action behavior after long idle/active windows
- Download supervision continuity (event updates and UI status refresh)

## Preflight

1. Run baseline gates:

```powershell
cargo check --workspace
cargo test -p cassette --test pure_logic -- --nocapture
Set-Location ui; npm run check; npm run build; Set-Location ..
```

2. Confirm runtime prerequisites:

```powershell
.\scripts\smoke_desktop.ps1 -Strict
```

3. Record environment snapshot:

- OS version
- App version/commit hash
- Audio output device
- Library size (tracks/albums)
- Provider availability (if testing downloads)

## Soak Profiles

### Profile A: Playback Stability (2 hours)

1. Start app and begin playback queue with at least 50 tracks.
2. Every 10 minutes execute:
   - play/pause toggle
   - next/previous
   - seek
   - volume change
3. Every 20 minutes execute:
   - command palette open/close
   - compact mode toggle
   - minimize/restore
   - tray play/pause + next
4. Collect observations:
   - control latency
   - dropped events or desync
   - playback stalls
   - memory/CPU trend if observable

### Profile B: Download + UI Supervision (90 minutes)

1. Queue a bounded set of downloads (10-20 requests).
2. Keep Downloads view open for first 30 minutes, then background app for 30 minutes, then restore.
3. Verify:
   - status progression remains coherent
   - no silent failure loops
   - provider health chips continue to update
   - request review panel still opens and loads evidence

### Profile C: Idle + Resume (8 hours overnight)

1. Keep app open and idle for 8 hours (optionally paused playback).
2. In morning verify:
   - app responsive
   - command palette responsive
   - tray restore works
   - playback resumes correctly

## Exit Criteria

A soak run passes if:

1. No crash/hang requiring process kill.
2. No persistent command/control desync.
3. Tray/taskbar actions still operate correctly after long idle.
4. Any transient issue is recoverable without restart.

## Failure Recording

If a failure occurs, capture:

- Timestamp
- Action being executed
- Expected vs actual behavior
- Recovery behavior
- Relevant logs/screenshots

Record in `docs/SOAK_EVIDENCE.md` under a new run entry.
