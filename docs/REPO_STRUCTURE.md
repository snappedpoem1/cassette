# Repository Structure Contract

Last audited: March 21, 2026

## Source-Of-Truth Directories

- `src-tauri/` - Tauri app host and command layer
- `crates/cassette-core/` - core Rust library (db, player, library, downloader, metadata, models)
- `ui/` - SvelteKit renderer source
- `scripts/` - operational PowerShell scripts
- `docs/` - project docs and recovery references
- `binaries/` - bundled third-party runtime binaries (for example `slskd`)

## Runtime Or Generated Directories

These are machine or build artifacts and should not be treated as source truth:

- `target/` - Rust build output
- `ui/node_modules/` - frontend dependencies
- `ui/build/` and `ui/.svelte-kit/` - frontend build output
- `.slskd/` - local daemon app-dir state
- `staging/` - local staging workspace

## Validation Commands

```powershell
cargo check
Set-Location ui; npm run build; Set-Location ..
powershell -ExecutionPolicy Bypass -File scripts\smoke_desktop.ps1
```

If these pass, the repo is in a buildable state for backend and renderer.
