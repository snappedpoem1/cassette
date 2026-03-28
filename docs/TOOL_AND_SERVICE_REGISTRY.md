# Tool And Service Registry

Last audited: 2026-03-27

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
| SQLite (`cassette-core::db`) | Persistence | Active app runtime store | Lightweight local runtime state | Current schema too thin for identity/provenance memory | Short-term runtime persistence unless richer schema adopted | Proven Working |
| SQLite (`librarian`/`library` schema) | Persistence | Rich sidecar schema exists in repo/root DB | Desired-track memory, reconciliation, deltas, operations, invariants | Not the active Tauri runtime spine | Canonical acquisition-memory/provenance store | Partially Wired |
| Director | Orchestrator | Active acquisition waterfall | End-to-end acquisition planning, scoring, fallback, finalization | Request contract too shallow; candidate persistence weak | Canonical acquisition orchestrator | Proven Working |
| `downloader/` module | Internal legacy subsystem | Config/status overlap remains | Could be removed or folded | Duplicates active provider ownership | None; retire or merge | Dead/Conflicting |
| MusicBrainz | External metadata API | Tag-fix search, parent-album resolution, release tracks | Canonical artist/release/release-group/recording identity spine | 1 req/sec etiquette; no audio delivery | Canonical metadata and identity owner | Proven Working |
| Cover Art Archive | External artwork API | Mentioned only | Artwork retrieval by MBID after canonical release chosen | Not currently wired | Canonical artwork fetch after release selection | Doc-Only Idea |
| Discogs | External metadata API | Stub enricher only | Label, format, country, catalog no., secondary release enrich | API/auth and rate limits; docs access rough in this audit environment | Secondary enrich, not canonical identity | Stub/Placeholder |
| Last.fm | External metadata API | Player context info; enricher stub | Tags, bios, related-artists context, user-facing enrichment | Not canonical release authority | Contextual enrich for artist/album display | Partially Wired |
| LRCLIB | External lyrics API | Player lyrics fetch | Synced/static lyrics lookup and caching | Official docs not clearly surfaced in this pass | Lyrics source | Implemented but Unverified |
| Spotify Web API | External metadata API | Search/discography/history import | Artist catalog seed, exact track search via filters, library intent expansion | No audio acquisition; OAuth/rate limits | Discovery and intent seed, not download owner | Proven Working |
| Qobuz | External provider | Search and acquisition provider | High-trust lossless acquisition and artwork metadata | Depends on non-public credentials/secrets and fragile auth handling | Preferred premium lossless acquisition route when available | Implemented but Unverified |
| Deezer | External provider | Search plus private media flow | Search, catalog lookup, potentially good FLAC acquisition path | Acquisition uses private endpoints and decryption; brittle/legal/durability risk | Secondary premium acquisition only if private path remains acceptable | Implemented but Unverified |
| slskd / Soulseek | External daemon/API | Search, download handoff, transfer polling | Broad long-tail acquisition, browsing, queueing, transfer management | Search concurrency and daemon state issues | Canonical P2P acquisition route | Implemented but Unverified |
| NZBGeek | External index/search API | Usenet search | NZB search/broker | External index availability and API key | NZB search owner if Usenet retained | Implemented but Unverified |
| SABnzbd | External download client | Optional NZB handoff | Queue, history, category handling, completion polling | Repo only uses submit, not full queue/history APIs | NZB execution owner | Implemented but Unverified |
| Jackett | External indexer broker | Config-only | Multi-index torrent search via Torznab | Not wired today | Canonical torrent search owner if torrent path retained | Doc-Only Idea / Underused |
| Real-Debrid | External resolver/hoster API | Torrent cache check, add/select/poll, link unrestrict | Cached torrent acquisition, hoster unrestriction, file selection | Repo mixes it with TPB search; search should live elsewhere | Canonical torrent/hoster resolver, not search owner | Implemented but Unverified |
| TPB `apibay` | External search endpoint | Real-Debrid provider uses it for torrent search | Very limited torrent discovery | Low trust, not configurable, not broad or durable enough | None; replace with Jackett if torrent path stays | Dead/Conflicting |
| yt-dlp | External CLI | Fallback candidate and acquisition | Wide extractor coverage, audio extraction, low-trust fallback | Weak metadata provenance and edition certainty | Last-resort fallback acquisition | Proven Working |
| Symphonia | Rust crate | Validation/playback support | Decode/probe/audio property validation | Not a metadata writer | Canonical decode/validation layer | Proven Working |
| Lofty | Rust crate | Tag reads/writes, tag fixes | Robust metadata normalization and artwork embedding | Needs canonical metadata source upstream | Canonical tag write/read owner | Proven Working |
| `ProviderBridge` | Internal compatibility layer | Used in validation path | Adapter for old source-provider model | Keeps old abstractions alive | Remove after migration to canonical director path | Legacy/Compatibility Only |
| `BandcampSource` | Internal source | Placeholder error | Could be metadata search or source resolver later | Not configured at all | None until real provider exists | Stub/Placeholder |
| Tidal | Planned provider | Docs mention only | Premium catalog/discography/acquisition if legally and technically implemented | No code today | None yet | Doc-Only Idea |
