# Cass//ette

> A local-first music system for acquisition, organization, audit, and playback.

Cass//ette is a desktop application built with Rust, Tauri 2, SvelteKit, and SQLite. It is designed around a deterministic pipeline: scan what you have, reconcile what is missing, acquire safely, validate results, and keep an auditable trail of what changed.

## Status

Active hardening, audit proof, and packaging confidence.

Current verified state on this machine, as of 2026-03-30:

- `cargo check --workspace` passes
- `cargo test --workspace` passes
- `npm run build` passes in `ui/`
- `.\scripts\smoke_desktop.ps1` passes

## Architecture

```text
UI (SvelteKit)
  ->
Tauri command layer (`src-tauri/`)
  ->
Shared Rust domain (`crates/cassette-core/`)
  ->
SQLite + local filesystem + external providers
```

## Core Runtime Areas

| Area | Role |
|------|------|
| Librarian | Scan, import, normalize, and classify local library state |
| Custodian | Organize, validate, quarantine, and stage files safely |
| Orchestrator | Reconcile desired vs local state and plan work |
| Director | Acquire media through provider backends |
| Gatekeeper | Validate and admit staged arrivals |
| Library | Operational locking, recovery, and observability |
| Metadata | Tag repair, enrichment, and supporting lookups |
| Validation | Logging and sandbox validation flows |

## Providers

The active acquisition runtime lives under `director/providers/`.

- Local Archive
- Deezer
- Qobuz
- slskd / Soulseek
- Usenet
- yt-dlp
- Real-Debrid

## Clone And Verify

Cassette is developed and verified on Windows 11. Use PowerShell examples unless you are deliberately adapting them.

```powershell
git clone https://github.com/snappedpoem1/cassette.git
Set-Location cassette

cargo check --workspace
cargo test --workspace

Set-Location ui
npm install
npm run build
Set-Location ..

.\scripts\smoke_desktop.ps1
```

For local desktop development:

```powershell
cargo tauri dev
```

## Runtime Databases

- The active desktop runtime DB lives in the Tauri app-data directory as `cassette.db`.
- The integrated librarian/orchestrator sidecar DB lives in the same app-data directory as `cassette_librarian.db`.
- Repo-root `*.db` files are local artifacts for probes, tests, or inspection; they are not automatically the live desktop runtime database.

## Documentation

Canonical project docs live in [`docs/`](docs/):

- `PROJECT_INDEX.md` - project map, status, architecture
- `PROJECT_STATE.md` - current runtime capabilities and verified state
- `TODO.md` - living task list
- `DECISIONS.md` - architectural rationale
- `AGENT_CODEX.md` - agent operating manual
- `AGENT_BRIEFING.md` - quick onboarding
