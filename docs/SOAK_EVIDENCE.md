# Desktop Soak Evidence Log

Last updated: 2026-04-05

## Run 2026-04-05-A (Automation-backed preflight)

Type: Baseline pre-soak evidence

Executed:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/verify_ci_gate.ps1
cargo test -p cassette --test pure_logic -- --nocapture
cargo check --workspace
Set-Location ui; npm run build; Set-Location ..
```

Observed:

- All baseline validation gates passed.
- Command/tray/boundary regression suite passed (17 tests).
- UI build passed with no active a11y warning regression.
- No crash/hang observed during command/test/build cycle.

Known pain points found in this run:

- None observed in automation-backed preflight.

Notes:

- This run is a prerequisite confidence pass, not a full multi-hour interactive soak profile.
- Full interactive soak runs should be logged as Run B/C entries following `docs/SOAK_TEST_PROCEDURE.md`.
