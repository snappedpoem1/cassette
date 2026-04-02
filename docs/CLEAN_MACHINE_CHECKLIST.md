# Clean Machine Checklist

Last updated: 2026-04-02

Use this before claiming Cassette is shippable on a fresh Windows machine.

## Required Tools

- Rust toolchain that can build the workspace
- Node/npm for `ui/`
- PowerShell 5+ or PowerShell 7+
- Tauri desktop prerequisites required by this repo

Optional but expected for the current machine-proven acquisition setup:

- `slskd` running on `http://localhost:5030`
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

That script currently proves:

- `cargo check --workspace`
- targeted request-contract tests
- targeted audit-trace test
- `cargo test -p cassette-core`
- `cargo test -p cassette --lib --no-run`
- `npm run build`
- `.\scripts\smoke_desktop.ps1`

## Known Gap

- `cargo test --workspace` is now part of the trustworthy clean-machine gate again. The old Windows `STATUS_ENTRYPOINT_NOT_FOUND` failure came from the Tauri lib-test harness starting without the desktop manifest; pure command/bootstrap tests now live in `src-tauri/tests/pure_logic.rs` instead of the Tauri-linked lib harness.
