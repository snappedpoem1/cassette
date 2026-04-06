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

## Ownership And Product Framing

- This project is single-owner and personal-use only.
- Do not frame work as commercial productization, go-to-market, customer launch, or team-scale rollout.
- Treat terms like "release", "ship", and "production" in existing docs as personal reliability/readiness gates for the owner's machine and workflows.
- Prioritize trust, reversibility, auditability, and day-to-day personal utility over product-facing concerns.

## Current Ground Truth

- The app is a Tauri 2 shell in `src-tauri/`, a SvelteKit UI in `ui/`, and shared Rust domain code in `crates/cassette-core/`.
- The active desktop runtime stores its main SQLite database in the Tauri app-data directory as `cassette.db`.
- The integrated librarian/orchestrator control-plane database lives alongside it as `cassette_librarian.db`.
- Repo-root `*.db` files may exist for probes, tests, or local inspection. Do not assume they are the live desktop runtime database.
- `cargo check --workspace` passes.
- `cargo test --workspace` passes.
- `npm run build` passes in `ui/` with an existing accessibility warning in `src/routes/downloads/+page.svelte`.
- `.\scripts\smoke_desktop.ps1` passes.
- Deezer full-track acquisition is live-proven on this machine.
- The top hardening priorities are audit completeness proof, bounded coordinator proof, organizer safe-subset proof, and packaging confidence.

## Verification Rule

Do not inherit pass/fail claims from stale docs. Re-verify current state when the task depends on it.

Primary commands:

```powershell
cargo check --workspace
cargo test --workspace
Set-Location ui; npm install; npm run build; Set-Location ..
.\scripts\smoke_desktop.ps1
```
