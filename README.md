# Cass//ette

> A music system that understands music the way a deeply obsessive listener does.

Cass//ette is a desktop music application built in Rust + Tauri 2 + SvelteKit. It handles music acquisition, library organization, metadata enrichment, and intelligent playback — with a pipeline architecture designed for speed, reliability, and sonic obsession.

## Status

Active — hardening, audit proof, and packaging confidence. Core acquisition pipeline is live with 7 providers, full library management, playback, and metadata enrichment operational.

## Architecture

```
UI (SvelteKit)
  → Tauri command layer (src-tauri/)
    → Shared Rust domain (crates/cassette-core/)
      → SQLite (WAL mode, rusqlite + sqlx)
```

## Tech Stack

- **Backend:** Rust (multi-crate workspace)
- **Desktop shell:** Tauri 2
- **Frontend:** SvelteKit
- **Database:** SQLite (WAL mode, rusqlite runtime + sqlx custodian subsystem)
- **Audio:** Symphonia (decode/validate), Lofty (tags), cpal (output)
- **Hashing:** blake3

## Modules

| Module | Role |
|--------|------|
| **Librarian** | Scan, import, normalize, and classify library state |
| **Custodian** | Organize, validate, quarantine, and stage files safely |
| **Orchestrator** | Reconcile desired and local state, then plan work |
| **Director** | Acquire media through providers and sources (7 active) |
| **Gatekeeper** | Validate and admit staged arrivals |
| **Library** | Operational management, locking, recovery, and observability |
| **Metadata** | Supporting metadata lookups, tag-fix flows, enrichment |
| **Validation** | Sandbox validation, logging checks, and operational audit |

## Providers

| Provider | Trust | Format |
|----------|-------|--------|
| Local Archive | 0 | Direct copy |
| Deezer | 5 | FLAC/320/128 via Blowfish CBC |
| Qobuz | 10 | Lossless via MD5-signed session |
| slskd/Soulseek | 10 | P2P with queue recovery |
| Usenet | 30 | NZBgeek + SABnzbd |
| yt-dlp | 50 | Subprocess fallback |
| Real-Debrid | 80 | Torrent resolution + extraction |

## Default Paths

| Purpose | Path |
|---------|------|
| Source library | `A:\music` |
| Canonical library | `A:\music_sorted` |
| Staging | `A:\music_staging` |
| Quarantine | `A:\music_quarantine` |
| Admin/DB | `A:\music_admin` |

## Building

```bash
# Rust workspace
cargo check --workspace
cargo test

# Frontend
cd ui && npm install && npm run build

# Desktop app
cargo tauri dev
```

## Documentation

Canonical project docs live in [`docs/`](docs/):

- `PROJECT_INDEX.md` — project map, status, architecture
- `PROJECT_STATE.md` — current runtime capabilities
- `TODO.md` — living task list
- `DECISIONS.md` — architectural rationale
- `AGENT_CODEX.md` — agent operating manual
- `AGENT_BRIEFING.md` — quick onboarding
