# Cassette Release Checklist

Last updated: 2026-04-06

Use this checklist before calling a Windows release candidate shippable.

## Latest Verification Snapshot (2026-04-06)

- `./scripts/verify_trust_spine.ps1` completed successfully
- `cargo tauri build` completed and produced both installer bundles
- `cargo run -p cassette --bin db_converge_cli -- --overwrite` completed with:
  - `desired_tracks=4`
  - `delta_queue=11`
  - `acquisition_requests=0`
- Desktop smoke now validates managed `slskd` readiness through `slskd_runtime_probe_cli` instead of a raw `localhost:5030` socket check

## Build And Verification Gate

- [ ] `cargo check --workspace`
- [ ] `cargo test --workspace`
- [ ] `Set-Location ui; npm install; npm run build; Set-Location ..`
- [ ] `.\scripts\smoke_desktop.ps1 -Strict`
- [ ] `.\scripts\verify_trust_spine.ps1`
- [ ] `.\scripts\perf_baseline_capture.ps1 -Runs 3 -WarmupRuns 1`
- [ ] `.\scripts\perf_regression_gate.ps1 -CandidateResultPath artifacts\perf\run-<timestamp>\results.json`

## Packaging Gate

- [ ] `cargo tauri build`
- [ ] Optional automation path: run `Release Candidate` workflow (`.github/workflows/release-candidate.yml`) with a label to produce installers + release manifest artifact
- [ ] Confirm bundle artifacts exist:
  - [ ] `target/release/bundle/msi/Cassette_0.1.0_x64_en-US.msi`
  - [ ] `target/release/bundle/nsis/Cassette_0.1.0_x64-setup.exe`
- [ ] Confirm `default-run = "cassette"` is present in `src-tauri/Cargo.toml`

## Clean-Room Install Gate (Windows)

- [ ] Choose one isolation path:
  - [ ] Separate Windows machine
  - [ ] Windows Sandbox on the same machine
  - [ ] Disposable local Windows user profile on the same machine
- [ ] Install from `.msi` or `.exe` bundle inside the chosen clean-room path
- [ ] Launch app once and confirm bootstrap created:
  - [ ] `%APPDATA%/dev.cassette.app/cassette.db`
  - [ ] `%APPDATA%/dev.cassette.app/cassette_librarian.db`
- [ ] Run `./scripts/verify_cleanroom_local.ps1 -Mode <Sandbox|DisposableProfile>`
- [ ] Confirm first-run settings bootstrap (`library_base`, `staging_folder`, provider defaults)
- [ ] If true isolation is unavailable, run app-data reset fallback on the same machine and record this as lower-confidence proof in release notes

## Unified Datastore Convergence Gate

When shipping convergence-related changes, run:

`cargo run -p cassette --bin db_converge_cli -- --overwrite`

- [ ] Confirm output file exists: `%APPDATA%/dev.cassette.app/cassette_unified.db`
- [ ] Confirm command output reports non-error table counts
- [ ] Optional sanity query (SQLite shell):
  - [ ] `SELECT COUNT(*) FROM control_desired_tracks;`
  - [ ] `SELECT COUNT(*) FROM control_delta_queue;`
  - [ ] `SELECT COUNT(*) FROM control_acquisition_requests;`

## Provider Configuration Gate (Machine-Dependent)

- [ ] Required provider credentials are configured in DB settings, `.env`, or external config
- [ ] If Jackett is expected, confirm:
  - [ ] `JACKETT_URL`
  - [ ] `JACKETT_API_KEY`
  - [ ] `REAL_DEBRID_KEY`
- [ ] If Real-Debrid TPB fallback is expected, confirm:
  - [ ] `REAL_DEBRID_KEY`

## Known Gaps (Must Be Explicit)

- [ ] Any remaining audit-completeness uncertainty is documented in `docs/TODO.md`
- [ ] Any machine-specific provider assumptions are recorded in `docs/PROJECT_STATE.md`
- [ ] If clean-machine install proof was not run in this cycle, explicitly state that in release notes
