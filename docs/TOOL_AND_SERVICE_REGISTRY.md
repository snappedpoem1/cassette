# Tool And Service Registry

Last audited: 2026-04-01

## Status Legend

- `Proven Working`
- `Implemented but Unverified`
- `Partially Wired`
- `Stub/Placeholder`
- `Legacy/Compatibility Only`
- `Doc-Only Idea`
- `Dead/Conflicting`

## Registry

| Tool / Service / Library | Type | Current repo usage | Full practical potential for this app | Constraints / risks | Recommended canonical responsibility | Status |
| --- | --- | --- | --- | --- | --- | --- |
| Tauri 2 | App platform | Desktop shell, Rust commands, plugins | Local desktop orchestration, events, background tasks, native file access | Needs stable command contracts and permission discipline | Desktop shell and command boundary owner | Proven Working |
| SvelteKit UI | Frontend | Library/download/settings/tools pages | Candidate review, explainability, diff review, manual approval UX | Current UX only exposes coarse flows | Renderer and interaction layer | Proven Working |
| SQLite (`cassette-core::db`) | Persistence | Active app runtime store in Tauri app-data `cassette.db` | Lightweight local runtime state | Current schema is not the reconciliation/control-plane store | Active playback/runtime persistence | Proven Working |
| SQLite (`librarian`/`library` sidecar schema) | Persistence | Durable sidecar store in Tauri app-data `cassette_librarian.db` | Desired-track memory, reconciliation, deltas, scan checkpoints, operations, invariants | Still not fully surfaced in the UI path | Canonical reconciliation/control-plane store | Partially Wired |
| Director | Orchestrator | Active acquisition waterfall | End-to-end acquisition planning, scoring, fallback, finalization | Request contract too shallow; candidate persistence weak | Canonical acquisition orchestrator | Proven Working |
| `downloader/` module | Internal legacy subsystem | Compatibility re-export for provider settings types | Could be removed once settings callers no longer depend on it | Must not regain runtime acquisition ownership | Compatibility-only until final removal | Legacy/Compatibility Only |
| MusicBrainz | External metadata API | Tag-fix search, parent-album resolution, release tracks | Canonical artist/release/release-group/recording identity spine | 1 req/sec etiquette; no audio delivery | Canonical metadata and identity owner | Proven Working |
| Cover Art Archive | External artwork API | Mentioned only | Artwork retrieval by MBID after canonical release chosen | Not currently wired | Canonical artwork fetch after release selection | Doc-Only Idea |
| Discogs | External metadata API | Stub enricher only | Label, format, country, catalog no., secondary release enrich | API/auth and rate limits; docs access rough in this audit environment | Secondary enrich, not canonical identity | Stub/Placeholder |
| Last.fm | External metadata API | Player context info; enricher stub | Tags, bios, related-artists context, user-facing enrichment | Not canonical release authority | Contextual enrich for artist/album display | Partially Wired |
| LRCLIB | External lyrics API | Player lyrics fetch | Synced/static lyrics lookup and caching | Official docs not clearly surfaced in this pass | Lyrics source | Implemented but Unverified |
| Spotify Web API | External metadata API | Search/discography/history import | Artist catalog seed, exact track search via filters, library intent expansion | No audio acquisition; OAuth/rate limits | Discovery and intent seed, not download owner | Proven Working |
| Qobuz | External provider | Search and acquisition provider | High-trust lossless acquisition and artwork metadata | Depends on non-public credentials/secrets and fragile auth handling | Preferred premium lossless acquisition route when available | Implemented but Unverified |
| Deezer | External provider | Search plus private media flow | Search, catalog lookup, and proven full-track acquisition on this machine | Acquisition uses private endpoints and decryption; brittle/legal/durability risk | Secondary premium acquisition only if private path remains acceptable | Proven Working |
| slskd / Soulseek | External daemon/API | Search, download handoff, transfer polling | Broad long-tail acquisition, browsing, queueing, transfer management | Search concurrency and daemon state issues | Canonical P2P acquisition route | Implemented but Unverified |
| NZBGeek | External index/search API | Usenet search | NZB search/broker | External index availability and API key | NZB search owner if Usenet retained | Implemented but Unverified |
| SABnzbd | External download client | Optional NZB handoff | Queue, history, category handling, completion polling | Repo only uses submit, not full queue/history APIs | NZB execution owner | Implemented but Unverified |
| Jackett | External indexer broker | Config-only | Multi-index torrent search via Torznab | Not wired today | Canonical torrent search owner if torrent path retained | Doc-Only Idea / Underused |
| Real-Debrid | External resolver/hoster API | Torrent cache check, add/select/poll/unrestrict; dedup by hash before addMagnet | Cached torrent acquisition, hoster unrestriction, file selection | Key read from DB `real_debrid_key` or `REAL_DEBRID_KEY` env var | Canonical torrent/hoster resolver, not search owner | Proven Working |
| TPB `apibay` | External search endpoint | `torrent_album_cli` album-first flow; cat 104 (FLAC) → 101 (Music); word-boundary album match scoring | Album-level torrent discovery for Spotify backlog | Low trust, unstructured results, no auth; replace with Jackett for production scale | Spotify-backlog torrent source; not Director path | Proven Working |
| 7-Zip | Local CLI | `torrent_album_cli` archive extraction (`C:/Program Files/7-Zip/7z.exe`) | Extract RARs/ZIPs delivered by RD unrestrict links | Must be present at that path | Archive extractor for RD-sourced album downloads | Proven Working |
| yt-dlp | External CLI | Fallback candidate and acquisition | Wide extractor coverage, audio extraction, low-trust fallback | Weak metadata provenance and edition certainty | Last-resort fallback acquisition | Proven Working |
| Symphonia | Rust crate | Validation/playback support | Decode/probe/audio property validation | Not a metadata writer | Canonical decode/validation layer | Proven Working |
| Lofty | Rust crate | Tag reads/writes, tag fixes | Robust metadata normalization and artwork embedding | Needs canonical metadata source upstream | Canonical tag write/read owner | Proven Working |
| `ProviderBridge` | Internal compatibility layer | Used in validation path | Adapter for old source-provider model | Keeps old abstractions alive | Remove after migration to canonical director path | Legacy/Compatibility Only |
| `BandcampSource` | Internal source | Placeholder error | Could be metadata search or source resolver later | Not configured at all | None until real provider exists | Stub/Placeholder |
| Tidal | Planned provider | Docs mention only | Premium catalog/discography/acquisition if legally and technically implemented | No code today | None yet | Doc-Only Idea |
