# Cassette Reference Documentation

Per-component reference docs for developers. Each describes what a component does **today in the codebase** — not what it could do.

Every API endpoint, data flow, and capability listed here was traced from the actual Rust source code.

## Providers

Download acquisition providers, ordered by trust rank (lower = more trusted).

| Doc | Component | Provider ID | Trust Rank | Status |
|-----|-----------|-------------|------------|--------|
| [local-archive.md](providers/local-archive.md) | Local Archive | `local_archive` | 0 | Proven Working |
| [deezer.md](providers/deezer.md) | Deezer | `deezer` | 5 | Implemented, Unverified |
| [qobuz.md](providers/qobuz.md) | Qobuz | `qobuz` | 10 | Implemented, Unverified |
| [slskd.md](providers/slskd.md) | Soulseek / slskd | `slskd` | 10 | Implemented, Unverified |
| [usenet.md](providers/usenet.md) | Usenet (NZBgeek + SABnzbd) | `usenet` | 30 | Implemented, Unverified |
| [ytdlp.md](providers/ytdlp.md) | yt-dlp | `yt_dlp` | 50 | Proven Working |
| [real-debrid.md](providers/real-debrid.md) | Real-Debrid | `real_debrid` | 80 | Implemented, Unverified |

## Orchestration

The Director engine and its internal pipeline stages.

| Doc | Component | Status |
|-----|-----------|--------|
| [director-engine.md](orchestration/director-engine.md) | Two-pass waterfall orchestration loop | Proven Working |
| [strategy-planner.md](orchestration/strategy-planner.md) | 7 acquisition strategies and provider ordering | Proven Working |
| [scoring.md](orchestration/scoring.md) | 6-factor candidate scoring | Proven Working |
| [validation.md](orchestration/validation.md) | Symphonia-based audio validation | Proven Working |
| [finalization.md](orchestration/finalization.md) | Path building, dedup, atomic moves | Proven Working |
| [temp-manager.md](orchestration/temp-manager.md) | Per-task temp dirs and stale recovery | Proven Working |

## Metadata Services

External metadata APIs and import paths.

| Doc | Component | Auth Required | Status |
|-----|-----------|---------------|--------|
| [musicbrainz.md](metadata/musicbrainz.md) | MusicBrainz (identity, tag fixes, discography) | No | Proven Working |
| [lastfm.md](metadata/lastfm.md) | Last.fm (artist/album context) | No (public key) | Proven Working |
| [lrclib.md](metadata/lrclib.md) | LRCLIB (lyrics) | No | Proven Working |
| [spotify.md](metadata/spotify.md) | Spotify (history import + search) | Optional | Proven Working |

## Audio Processing

Rust crates for audio validation and metadata.

| Doc | Component | Status |
|-----|-----------|--------|
| [symphonia.md](audio/symphonia.md) | Format probing, codec detection, playback | Proven Working |
| [lofty.md](audio/lofty.md) | Tag read/write, cover art embedding | Proven Working |

## Not Yet Documented (Coverage Gaps)

These components either have working code without dedicated reference pages, or are intentionally partial integrations:

| Component | Status | Notes |
|-----------|--------|-------|
| Jackett | Active provider | `director/providers/jackett.rs` exists; add a dedicated provider reference page |
| Genius | Config field only | `genius_token` env var defined in DownloadConfig, never used |
| Discogs | Implemented enrichment/search usage | `librarian/enrich/discogs.rs` and `sources.rs` are active; add dedicated Discogs reference doc |
| Last.fm history sync | Implemented | `sync_lastfm_history` and play-history persistence are active; expand `metadata/lastfm.md` coverage |
| Bandcamp | Partial resolver helper | `director/sources/bandcamp.rs` resolves payload URLs only; no catalog/search integration |
| Cover Art Archive | Not wired | No code references to coverartarchive.org API |

## Status Legend

- **Proven Working** — Code compiles, has tests, and/or has been observed working at runtime
- **Implemented, Unverified** — Full search + acquire implementation exists but has not been end-to-end verified in the shipped app
- **Stub/Placeholder** — Code exists but does nothing useful

## Configuration Priority

Settings are resolved in this order (highest wins):

1. **SQLite database** (user-saved via settings UI)
2. **Environment variables** (`.env` file)
3. **Streamrip config** (`%APPDATA%/streamrip/config.toml`) — auto-imports Qobuz/Deezer credentials
4. **slskd config** (`%LOCALAPPDATA%/slskd/slskd.yml`)
5. **Hardcoded defaults**

## Conventions

- Every external URL in these docs is traced from actual `reqwest` calls in the source
- Trust rank determines scoring bonus: `(20 - trust_rank).max(0)` points
- Provider concurrency is controlled by per-provider semaphores configured in `DirectorConfig.provider_policies`
- All providers implement the `Provider` trait: `descriptor()`, `search()`, `acquire()`
