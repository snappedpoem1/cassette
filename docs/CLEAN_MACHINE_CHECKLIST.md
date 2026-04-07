# Clean Machine Checklist

Last updated: 2026-04-06

Use this before claiming Cassette is shippable on a fresh Windows machine.

## Required Tools

- Rust toolchain that can build the workspace
- Node/npm for `ui/`
- PowerShell 5+ or PowerShell 7+
- Tauri desktop prerequisites required by this repo

Optional but expected for the current machine-proven acquisition setup:

- `yt-dlp` available on `PATH`
- `7-Zip` at `C:\Program Files\7-Zip\7z.exe` for torrent/Real-Debrid archive extraction
- Provider credentials in app settings, `.env`, or external config files as documented in `PROJECT_STATE.md`

## First-Run Bootstrap Expectations

On first app boot, Cassette should create:

- runtime DB: app-data `cassette.db`
- control-plane sidecar DB: app-data `cassette_librarian.db`

Bootstrap behavior currently recovers/persists:

- `library_base`
- `staging_folder`
- `slskd_*` connection defaults
- recoverable provider settings from `.env`, `streamrip/config.toml`, and `slskd.yml` when present

## Verification Pass

Run:

```powershell
.\scripts\verify_trust_spine.ps1
```

For clean-room bootstrap verification after installer launch, run:

```powershell
.\scripts\verify_cleanroom_local.ps1 -Mode DisposableProfile
```

Use `-Mode Sandbox` when validating inside Windows Sandbox.

That script currently proves:

- `cargo check --workspace`
- targeted request-contract tests
- targeted audit-trace test
- `cargo test -p cassette-core`
- `cargo test -p cassette --lib --no-run`
- `npm run build`
- `.\scripts\smoke_desktop.ps1 -Strict`

`smoke_desktop.ps1` now validates managed `slskd` readiness by running
`cargo run -p cassette --bin slskd_runtime_probe_cli -- --json`, which reuses the same
runtime startup contract Cassette uses for bundled `slskd.exe` instead of checking port `5030`
in isolation.

Optional integrated run (includes local clean-room bootstrap checks):

```powershell
.\scripts\verify_trust_spine.ps1 -RunCleanroomLocal -CleanroomMode DisposableProfile
```

## Packaging (verified 2026-04-03)

`cargo tauri build` produces installable bundles on Windows:

```
target/release/bundle/msi/Cassette_0.1.0_x64_en-US.msi
target/release/bundle/nsis/Cassette_0.1.0_x64-setup.exe
```

Required fix: `default-run = "cassette"` in `src-tauri/Cargo.toml`. Without this, Tauri fails to find the main binary when multiple `[[bin]]` entries exist.

## Optional Provider Configuration

| Provider | Required Settings | Notes |
|----------|------------------|-------|
| Jackett | `JACKETT_URL`, `JACKETT_API_KEY`, `REAL_DEBRID_KEY` | Multi-indexer torrent search; RD resolves magnets |
| Real-Debrid (TPB) | `REAL_DEBRID_KEY` | Standalone TPB search + RD resolve |
| Jackett config keys | Set via DB settings or `.env` | `jackett_url` defaults to `http://localhost:9117` |

## Known Gap

- `cargo test --workspace` is now part of the trustworthy clean-machine gate again. The old Windows `STATUS_ENTRYPOINT_NOT_FOUND` failure came from the Tauri lib-test harness starting without the desktop manifest; pure command/bootstrap tests now live in `src-tauri/tests/pure_logic.rs` instead of the Tauri-linked lib harness.
- For this single-machine personal setup, clean-room proof is satisfied by a same-machine clean-room run (Windows Sandbox or disposable local profile). If only app-data reset fallback is used, record it as lower-confidence evidence in release notes.
