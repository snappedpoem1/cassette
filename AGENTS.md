# Cassette Agent Instructions

Repo-local instruction alignment for this workspace.

## Read First

Canonical project docs live under `docs/`, not the repo root.

Before substantial work, read:

1. `docs/PROJECT_INDEX.md`
2. `docs/PROJECT_STATE.md`
3. `docs/TODO.md`
4. `docs/DECISIONS.md`
5. `docs/AGENT_BRIEFING.md`
6. `docs/AGENT_CODEX.md`

## Current Ground Truth

- The app is a Tauri 2 shell in `src-tauri/`, a SvelteKit UI in `ui/`, and shared Rust domain code in `crates/cassette-core/`.
- The active desktop runtime stores its main SQLite database in the Tauri app-data directory as `cassette.db`.
- Repo-root `*.db` files may exist for probes, tests, or local inspection. Do not assume they are the live desktop runtime database.
- `cargo check` currently succeeds with warnings.
- `cargo test` is not currently green.
- The top hardening priorities are Deezer full-track proof, audit completeness proof, async/recovery hardening, and packaging confidence.

## Verification Rule

Do not inherit pass/fail claims from stale docs. Re-verify current state when the task depends on it.

Primary commands:

```powershell
cargo check
cargo test
Set-Location ui; npm run build; Set-Location ..
.\scripts\smoke_desktop.ps1
```
