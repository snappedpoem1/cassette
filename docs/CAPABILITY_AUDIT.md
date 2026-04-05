# Cassette Capability Audit

Last audited: 2026-04-03

Historical snapshot: parts of this audit were superseded by the 2026-03-30 documentation and runtime hardening pass. For current truth, prefer `PROJECT_STATE.md`, `TODO.md`, and `DECISIONS.md`.

## Audit Standard

Every major claim below is labeled:

- `Proven`: directly supported by current repo code, current repo data, or a successful local build/test command.
- `Strong Inference`: not explicitly wired end-to-end, but strongly implied by existing code shape and neighboring behavior.
- `Weak Inference`: plausible, but the repo does not prove it and external documentation was incomplete or inaccessible.
- `Not Supported`: the repo does not currently prove the behavior.

This document is intentionally not a feature wish list. It is a system truth map.

## Executive Truth

| Claim | Status | Evidence |
| --- | --- | --- |
| Cassette is currently a Tauri desktop music app with a Rust core, SQLite local store, Svelte UI, local library scan/playback, and a provider-backed acquisition workflow. | `Proven` | `src-tauri/src/lib.rs`, `src-tauri/src/state.rs:23-61`, `ui/package.json`, `crates/cassette-core/src/db/mod.rs:23-108` |
| The shipped runtime is not built on the repo's richest schema. It uses a smaller app DB centered on `tracks`, `settings`, `spotify_album_history`, and `director_task_history`. | `Proven` | `crates/cassette-core/src/db/mod.rs:23-108`; local `%APPDATA%\\dev.cassette.app\\cassette.db` inspection showed only 8 runtime tables |
| The repo also contains a richer library/reconciliation/provenance stack (`desired_tracks`, `reconciliation_results`, `delta_queue`, `operation_log`, invariant tracking), but it is not the active Tauri runtime data spine. | `Proven` | `crates/cassette-core/src/librarian/db/migrations.rs:1-146`, `crates/cassette-core/src/library/schema/migrations.rs:1-103`; local root `cassette.db` inspection |
| Acquisition is real, but product-level explainability, release identity, and durable candidate memory are incomplete. | `Proven` | `src-tauri/src/commands/downloads.rs:12-595`, `crates/cassette-core/src/director/engine.rs:167-460`, `crates/cassette-core/src/db/mod.rs:92-108` |
| The codebase contains both current and overlapping legacy/parallel abstractions for downloading and provider integration. | `Proven` | `crates/cassette-core/src/director/providers/*`, `crates/cassette-core/src/downloader/mod.rs`, `crates/cassette-core/src/director/sources/*` |

## Phase 1: Repo Truth Inventory

### Governance Inputs

| Artifact | Finding | Status |
| --- | --- | --- |
| `AGENTS.md` / user instructions | Windows + PowerShell environment; no `os.path`; type hints/logging rules if Python is touched. | `Proven` |
| `docs/PROJECT_STATE.md` | Current runtime-truth document as of 2026-03-30. | `Proven` |
| `docs/WORKLIST.md` | Forward-looking planning document; must stay aligned with the deliberate sidecar control-plane architecture. | `Proven` |
| `docs/ROADMAP_ENGINE_TO_ENTITY.md` | Missing. | `Proven` |
| `docs/MISSING_FEATURES_REGISTRY.md` | Missing. | `Proven` |
| `docs/SESSION_INDEX.md` | Missing. | `Proven` |
| `specs/` | Missing. | `Proven` |

### Current Build/Test Reality

| Check | Result | Status |
| --- | --- | --- |
| `cargo check` | Passes at the workspace root. | `Proven` |
| `cargo test` | Passes at the workspace root. | `Proven` |
| `npm run build` in `ui` | Succeeds. | `Proven` |
| `scripts/smoke_desktop.ps1` | Exists, but it is an environment/readiness check rather than a behavioral desktop smoke suite. | `Proven` |

### Internal System Inventory

| Component | Role | Classification | Evidence |
| --- | --- | --- | --- |
| `src-tauri` | Desktop shell, command registration, app-state composition | `Proven Working` | `src-tauri/src/lib.rs`, `src-tauri/src/state.rs` |
| `ui` | Renderer, pages, Tauri invoke client | `Proven Working` | `ui/src/lib/api/tauri.ts`, `ui/src/routes/*` |
| `cassette-core::db` | Active app runtime DB | `Proven Working` | `crates/cassette-core/src/db/mod.rs:23-108` |
| `cassette-core::director` | Current acquisition engine and provider waterfall | `Proven Working` | `crates/cassette-core/src/director/engine.rs` |
| `director/providers/qobuz` | Search + direct file acquisition | `Implemented but Unverified` | `crates/cassette-core/src/director/providers/qobuz.rs` |
| `director/providers/deezer` | Search + private media acquisition/decryption | `Proven Working` on this machine | `crates/cassette-core/src/director/providers/deezer.rs` |
| `director/providers/slskd` | Search + transfer handoff + local file discovery | `Implemented but Unverified` | `crates/cassette-core/src/director/providers/slskd.rs` |
| `director/providers/usenet` | NZBGeek search + SABnzbd handoff + local file discovery | `Implemented but Unverified` | `crates/cassette-core/src/director/providers/usenet.rs` |
| `director/providers/real_debrid` | Torrent resolver path via TPB search + RD torrent APIs | `Implemented but Unverified` | `crates/cassette-core/src/director/providers/real_debrid.rs` |
| `director/providers/ytdlp` | yt-dlp search-token generation + extract-audio acquisition | `Proven Working` at code level; provider proof partial | `crates/cassette-core/src/director/providers/ytdlp.rs` |
| `director/providers/local_archive` | Local filesystem candidate search/copy | `Proven Working` | `crates/cassette-core/src/director/providers/local_archive.rs` |
| `cassette-core::sources` | Metadata search/discography aggregation, slskd helpers, provider auth helpers | `Proven Working` | `crates/cassette-core/src/sources.rs` |
| `cassette-core::metadata` | MusicBrainz-based tag-fix proposal and Lofty tag writing | `Proven Working` | `crates/cassette-core/src/metadata.rs:46-335` |
| `cassette-core::librarian` + `orchestrator` + `gatekeeper` + `custodian` | Richer reconciliation, delta, validation, staging, invariant tracking | `Partially Wired` | modules exist; not active Tauri data spine |
| `librarian/enrich/discogs.rs` | Discogs enricher | `Implemented but Unverified` | `crates/cassette-core/src/librarian/enrich/discogs.rs` |
| `librarian/enrich/lastfm.rs` | Last.fm enricher | `Implemented but Unverified` | `crates/cassette-core/src/librarian/enrich/lastfm.rs` |
| `director/sources/provider_bridge.rs` and `director/sources/*` | Compatibility bridge from older source-provider abstraction to current providers | `Legacy/Compatibility Only` | `crates/cassette-core/src/director/sources/*`, validation path references |
| `downloader/mod.rs` | Provider-settings compatibility surface | `Legacy/Compatibility Only` | no longer an active acquisition owner |
| `BandcampSource` | Bandcamp resolver | `Implemented but Unverified` | resolves Bandcamp URLs from desired payload metadata in `crates/cassette-core/src/director/sources/bandcamp.rs` |

## Phase 2: External Capability Research

### Metadata and Identity Sources

| Service | Relevant external capability | Repo use today | Notes |
| --- | --- | --- | --- |
| MusicBrainz | Search entities including `recording`, `release`, `release-group`; search supports Lucene query and `limit` 1-100. Source: [MusicBrainz Search API](https://musicbrainz.org/doc/MusicBrainz_API/Search). | `metadata.rs` searches releases and recordings, fetches release tracklists, and proposes tag fixes. | Repo uses only a subset of MB's strongest value. No persistent MBID spine in active app DB. |
| Cover Art Archive | Artwork service keyed from MusicBrainz entities. Source: [Cover Art Archive API](https://musicbrainz.org/doc/Cover_Art_Archive/API). | Mentioned in docs/session logs, not actively integrated. | Best fit is downstream enrichment after canonical release selection. |
| Discogs | Releases, masters, labels, formats, country, catalog numbers. Primary portal: [Discogs Developers](https://www.discogs.com/developers). | Used in shared metadata search/discography fallback and in librarian enrichment context. | Good secondary enrich source; should not outrank MusicBrainz for canonical identity. |
| Last.fm | Artist and album info endpoints expose tags, bios/wiki, stats, similar artists. Sources: [artist.getInfo](https://www.last.fm/api/show/artist.getInfo), [album.getInfo](https://www.last.fm/api/show/album.getInfo). | Used for player context, track-duration enrichment, and recent-track history sync. | Good for user-facing context and listening-history recovery, not canonical release identity. |
| LRCLIB | Free lyrics service used through `https://lrclib.net/api/get`. Public root: [LRCLIB](https://lrclib.net/). | Player fetches lyrics. | Official API documentation was not directly discoverable in this pass, so broader capability claims remain `Weak Inference`. |

### Search and Library Metadata APIs

| Service | Relevant external capability | Repo use today | Notes |
| --- | --- | --- | --- |
| Spotify Web API | Search supports albums/artists/tracks with filters including `album`, `artist`, `track`, `year`, `isrc`, `genre`; artist albums endpoint can page artist catalogs; rate limits are app-wide. Sources: [Search](https://developer.spotify.com/documentation/web-api/reference/search), [Get Artist's Albums](https://developer.spotify.com/documentation/web-api/reference/get-an-artists-albums), [Rate Limits](https://developer.spotify.com/documentation/web-api/concepts/rate-limits). | Metadata search/discography in `sources.rs`; Spotify history import in app DB. No direct audio acquisition. | Strong metadata source for intent expansion and discography building. |
| Deezer API | Public search API supports track search at `api.deezer.com`; repo uses that path for candidate retrieval. | Search uses public `search/track`; acquisition uses private session/media endpoints and decryption. | Public metadata/search use is durable; direct media acquisition path is unofficial and brittle. |
| Qobuz | Official primary sources surfaced here were terms/guidelines, not a modern public developer reference: [API Terms](https://static.qobuz.com/apps/api/QobuzAPI-TermsofUse.pdf), [Apps Guidelines](https://static.qobuz.com/apps/api/Qobuz-AppsGuidelines-V1.0.pdf). | Repo uses Qobuz search/auth/file-url flows directly. | Search/acquire path exists, but long-term durability depends on non-public app credentials and secret handling. |

### Acquisition and Transfer Systems

| Service | Relevant external capability | Repo use today | Notes |
| --- | --- | --- | --- |
| slskd | Soulseek daemon with web/API surface for searches, browse/downloads, queueing, transfers, and automation. Primary source: [slskd README](https://github.com/slskd/slskd/blob/master/README.md). | Repo uses `/api/v0/searches`, `/responses`, `/server`, transfer download APIs, and transfer polling. | Good for candidate retrieval and transfer execution, but current app does not persist search result history in a reusable canonical way. |
| SABnzbd | API-driven NZB queueing/management. Primary source: [SABnzbd API docs](https://sabnzbd.org/wiki/api). | Repo submits NZBs to `/api` with `mode=addfile`. | Current code does not use SAB queue/history APIs to prove completion; it just polls watched roots. |
| Jackett | Torznab/API broker over many torrent indexers. Primary source: [Jackett README](https://github.com/Jackett/Jackett). | Active Director provider plus `torrent_album_cli` search owner. | Canonical torrent search owner while torrent acquisition remains in scope. |
| Real-Debrid | Torrent and link resolver APIs include `torrents/addMagnet`, `torrents/selectFiles/{id}`, `torrents/info/{id}`, `torrents/instantAvailability/{hashes}`, and `unrestrict/link`. Source: [Real-Debrid API docs](https://api.real-debrid.com/). | Repo uses instant availability, torrent add/select/poll, and link unrestrict. | Repo uses RD as resolver/unrestrict owner, not as the canonical torrent search owner. |
| yt-dlp | Search extractors and audio extraction/postprocessing. Primary source: [yt-dlp README](https://github.com/yt-dlp/yt-dlp). | Repo creates `ytsearch1:` and `scsearch1:` candidates and runs `--extract-audio --audio-format best`. | Good as low-trust fallback; not suitable for canonical metadata identity. |

### Local Media and App Platform

| Tool/Library | Relevant external capability | Repo use today | Notes |
| --- | --- | --- | --- |
| Symphonia | Rust decode/probe framework for multiple audio codecs/containers. Source: [symphonia docs.rs](https://docs.rs/symphonia/latest/symphonia/). | Used for playback/validation surfaces. | Good validator/decoder; not a metadata write tool. |
| Lofty | Reads/writes metadata across many formats, exposes generic tag handling and picture support. Source: [lofty docs.rs](https://docs.rs/lofty/latest/lofty/). | Used for tag reads/writes and tag-fix application. | Strong tag-normalization owner. |
| Tauri 2 | Rust command surface, frontend invocation, plugin-based capability model. Sources: [Calling Rust](https://v2.tauri.app/develop/calling-rust/), [Calling Frontend](https://v2.tauri.app/develop/calling-frontend/), [Shell plugin](https://v2.tauri.app/plugin/shell/). | Core app shell uses commands/invoke and several plugins. | Suitable for local orchestration UI, but the repo has not defined enough stable request/response contracts for candidate review/explainability. |

## Phase 3: System Function Decomposition

| Domain | What exists now | What is missing | Canonical owner |
| --- | --- | --- | --- |
| Acquisition planning | `Director` builds a strategy plan and runs a provider waterfall. | Request object is too thin: no edition rules, exclusions, manual review policy, or cache-aware planning. | `director` plus a richer request model |
| Provider search | Qobuz, Deezer, slskd, Usenet, Real-Debrid/TPB, Local Archive, yt-dlp. Metadata search/discography separately uses `sources.rs`. | Search is split into metadata search and acquisition search with no canonical identity cache between them. | metadata resolver first, acquisition providers second |
| Candidate retrieval/scoring | Providers return candidates; engine validates and scores them. | Candidate sets are not durably stored for later reuse or UI review. | `director` |
| User validation/review | Practically none in the UI beyond choosing an album/discography to queue. | No "show candidates", "approve release", "exclude edition", or "approve low-trust fallback" contract. | UI + Tauri command surface |
| Metadata resolution | MusicBrainz tag-fix and parent-album resolution; Spotify/Qobuz/Deezer metadata search. | No durable canonical release-group/release spine in the active runtime DB. | MusicBrainz-backed resolver |
| Download/transfer | Multiple provider-specific acquisition paths are real. | Post-transfer reporting and provenance are not canonical across providers. | `director` orchestration |
| File validation | Candidate validation in `director::validation`; deeper validation in `custodian`. | Validation results are not reused as quality memory for future decisions. | `director::validation` + `custodian` |
| Quarantine/airlock/staging | Temp staging, stale temp recovery, optional quarantine on failure; richer `custodian` also exists. | The active app flow does not fully converge on the richer gatekeeper/custodian audit trail. | `director` handing off to `custodian` |
| Tagging/normalization | Lofty-based tag repair and organizer tools exist. | Canonical pathing/tagging are not driven by durable release identity. | metadata resolver + organizer/custodian |
| Duplicate handling/import | Duplicate policy exists; organizer/import tools exist; richer reconciliation DB exists in parallel. | "Already owned" checks are shallow because local files are not mapped to canonical source identities. | unified library identity store |
| Cache reuse/explainability | `director_task_history` stores final result JSON; slskd can reuse search history opportunistically. | No durable query cache, candidate cache, negative-result memory, user-exclusion memory, or rationale UI. | cache/provenance subsystem |

## Phase 4: User-Request Capability Summary

See `REQUEST_CAPABILITY_MATRIX.md` for the full matrix. The short version:

| Request family | Current truth |
| --- | --- |
| One song by artist/title | `Proven`, but exact-edition guarantees are weak |
| One album | `Proven`, but album identity is mostly text-based |
| One artist discography | `Proven` at coarse metadata level, not at precise edition policy level |
| Selected albums only | `Could Support with Existing Building Blocks`, but lacks first-class request grammar |
| Exclude live/remaster/deluxe | `Blocked by Missing UX Contract` and `Blocked by Missing Data Model` |
| Validate before download | `Partially Wired`; pre-acquisition review plus approve/reject is now user-visible, but deeper validation rationale and policy controls are still limited |
| Show candidates before acquisition | `Partially Wired`; request candidate/timeline review is exposed in Downloads, but there is no full query-level candidate explorer yet |
| Reuse prior metadata/search results | `Blocked by Missing Data Model` in the active app path |
| Prove why a result was chosen | `Partially Wired` internally, not surfaced or persisted richly enough |

## Phase 5: Cache, Memory, and Provenance Truth

| Finding | Status | Evidence |
| --- | --- | --- |
| The active app runtime now persists request signatures, candidate sets, provider searches, provider attempts, and provider-negative memory, but it still lacks a canonical release-identity spine. | `Proven` | `docs/PROJECT_STATE.md`, `crates/cassette-core/src/db/mod.rs` |
| The sidecar DB now models desired-track reconciliation, `delta_queue`, and scan checkpoints in a durable control-plane store. | `Proven` | `crates/cassette-core/src/librarian/db/migrations.rs`, `docs/PROJECT_STATE.md` |
| The architecture now has enough durable primitives to support cache reuse and explainability, but the UI and planning layers do not yet use them coherently. | `Strong Inference` | split between active runtime persistence, sidecar control plane, and current UI surface |

## Phase 6: Best-Shape Inference

1. Use MusicBrainz as the canonical release identity spine.
2. Persist source mappings from MB release/release-group/recording IDs to Spotify/Qobuz/Deezer/Discogs/source-specific IDs.
3. Split acquisition into explicit stages:
   - intent normalization
   - metadata resolution
   - candidate retrieval
   - candidate scoring
   - optional user review
   - acquisition
   - validation
   - normalization/tagging
   - library import
   - provenance finalization
4. Make `director` the orchestration owner, not `downloader/` and not ad hoc command logic.
5. Use the richer sidecar reconciliation/provenance schema coherently instead of leaving it half-integrated.

## Honest Bottom Line

### What the system really is today

Cassette is a working local-library desktop app with a real multi-provider acquisition engine, plus a second richer but only partially integrated reconciliation/provenance subsystem. It is already more than a player, but less than a fully explainable, canonical, edition-aware acquisition platform.

### What it could become with the current stack

With the parts already referenced, this repo could grow into an edition-aware music acquisition and library-management system that:

- plans acquisitions from normalized intent
- searches multiple provider classes
- reviews/scoring candidates
- validates files before admission
- normalizes tags and artwork against canonical metadata
- remembers prior searches, failures, and user exclusions
- explains why a release/provider was chosen

### What blocks that today

Not provider count. Not lack of crates. The main blockers are:

- split architecture
- missing canonical request grammar
- missing active provenance/cache data model
- lack of candidate-review UX contract
- reliance on some unofficial/private provider behaviors
