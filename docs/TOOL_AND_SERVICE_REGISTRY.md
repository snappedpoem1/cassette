# Tool And Service Registry

Last audited: 2026-04-03

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
| SvelteKit UI | Frontend | Library/download/settings/tools pages | Candidate review, explainability, diff review, manual approval UX | Downloads now exposes request timeline plus approve/reject review, but richer explainability and policy controls are still limited | Renderer and interaction layer | Proven Working |
| SQLite (`cassette-core::db`) | Persistence | Active app runtime store in Tauri app-data `cassette.db` | Lightweight local runtime state | Current schema is not the reconciliation/control-plane store | Active playback/runtime persistence | Proven Working |
| SQLite (`librarian`/`library` sidecar schema) | Persistence | Durable sidecar store in Tauri app-data `cassette_librarian.db` | Desired-track memory, reconciliation, deltas, scan checkpoints, operations, invariants | Still not fully surfaced in the UI path | Canonical reconciliation/control-plane store | Partially Wired |
| Director | Orchestrator | Active acquisition waterfall | End-to-end acquisition planning, scoring, fallback, finalization | Planner-stage review is now live via request timeline and approve/reject, but deeper policy lanes are still pending | Canonical acquisition orchestrator | Proven Working |
| `downloader/` module | Internal legacy subsystem | Compatibility re-export for provider settings types | Could be removed once settings callers no longer depend on it | Must not regain runtime acquisition ownership | Compatibility-only until final removal | Legacy/Compatibility Only |
| MusicBrainz | External metadata API | Tag-fix search, parent-album resolution, release tracks | Canonical artist/release/release-group/recording identity spine | 1 req/sec etiquette; no audio delivery | Canonical metadata and identity owner | Proven Working |
| Cover Art Archive | External artwork API | Mentioned only | Artwork retrieval by MBID after canonical release chosen | Not currently wired | Canonical artwork fetch after release selection | Doc-Only Idea |
| Discogs | External metadata API | Metadata search/discography fallback plus enricher release context | Label, format, country, catalog no., secondary release enrich | API/auth and rate limits; docs access rough in this audit environment | Secondary enrich, not canonical identity | Implemented but Unverified |
| Last.fm | External metadata API | Player context info, duration enrichment, and recent-track history sync | Tags, bios, related-artists context, user-facing enrichment | Not canonical release authority | Contextual enrich for artist/album display and history sync | Implemented but Unverified |
| LRCLIB | External lyrics API | Player lyrics fetch | Synced/static lyrics lookup and caching | Official docs not clearly surfaced in this pass | Lyrics source | Implemented but Unverified |
| Spotify Web API | External metadata API | Search/discography/history import plus direct desired-track import seed | Artist catalog seed, exact track search via filters, library intent expansion | No audio acquisition; OAuth/rate limits | Intent seed and source-alias owner, not download owner | Proven Working |
| Qobuz | External provider | Search and acquisition provider | High-trust lossless acquisition and artwork metadata | Depends on non-public credentials/secrets and fragile auth handling | Preferred premium lossless acquisition adapter when available | Implemented but Unverified |
| Deezer | External provider | Search plus private media flow | Search, catalog lookup, and proven full-track acquisition on this machine | Acquisition uses private endpoints and decryption; brittle/legal/durability risk | Secondary premium acquisition adapter | Proven Working |
| slskd / Soulseek | External daemon/API | Search, download handoff, transfer polling | Broad long-tail acquisition, browsing, queueing, transfer management | Search concurrency and daemon state issues | Canonical P2P acquisition route | Implemented but Unverified |
| NZBGeek | External index/search API | Usenet search | NZB search/broker | External index availability and API key | NZB search owner if Usenet retained | Implemented but Unverified |
| SABnzbd | External download client | Optional NZB handoff plus queue/history polling work in progress | Queue, history, category handling, completion polling | Completion proof is still being hardened | NZB execution owner | Implemented but Unverified |
| Jackett | External indexer broker | Active Director provider and `torrent_album_cli` search path | Multi-index torrent search via Torznab | Requires configured Jackett plus Real-Debrid for resolve | Canonical torrent search owner | Implemented but Unverified |
| Real-Debrid | External resolver/hoster API | Torrent cache check, add/select/poll/unrestrict; dedup by hash before addMagnet | Cached torrent acquisition, hoster unrestriction, file selection | Key read from DB `real_debrid_key` or `REAL_DEBRID_KEY` env var | Canonical torrent/hoster resolver, not search owner | Proven Working |
| TPB `apibay` | External search endpoint | Explicit `torrent_album_cli --allow-apibay-fallback` fallback only | Album-level emergency torrent discovery when Jackett is unavailable | Low trust, unstructured results, no auth | Debug/emergency fallback only; not default Director search | Proven Working |
| 7-Zip | Local CLI | `torrent_album_cli` archive extraction (`C:/Program Files/7-Zip/7z.exe`) | Extract RARs/ZIPs delivered by RD unrestrict links | Must be present at that path | Archive extractor for RD-sourced album downloads | Proven Working |
| yt-dlp | External CLI | Fallback candidate and acquisition | Wide extractor coverage, audio extraction, low-trust fallback | Weak metadata provenance and edition certainty | Last-resort fallback acquisition | Proven Working |
| Symphonia | Rust crate | Validation/playback support | Decode/probe/audio property validation | Not a metadata writer | Canonical decode/validation layer | Proven Working |
| Lofty | Rust crate | Tag reads/writes, tag fixes | Robust metadata normalization and artwork embedding | Needs canonical metadata source upstream | Canonical tag write/read owner | Proven Working |
| `ProviderBridge` | Internal compatibility layer | Used in validation path | Adapter for old source-provider model | Keeps old abstractions alive | Remove after migration to canonical director path | Legacy/Compatibility Only |
| `BandcampSource` | Internal source | Payload URL resolver | Resolves Bandcamp URL fields from desired payload into a download candidate | No catalog/search integration; relies on upstream payload quality | Source resolver helper only | Implemented but Unverified |

## 2026-04-03 Corrections

These notes override older research/reference text where registry language previously lagged runtime truth:

- Jackett is an active Director provider and the canonical torrent search owner.
- Real-Debrid is the canonical torrent/hoster resolve and unrestrict owner. Its direct TPB search is debug-only and disabled by default in the Director.
- `torrent_album_cli` treats apibay as explicit fallback only via `--allow-apibay-fallback`; it is no longer the default torrent search path when Jackett is available.
- Spotify is not just search/discography seed. The direct desired-track import path now persists source IDs, best-effort ISRC, and richer raw payload JSON.
- `cassette_librarian.db` is the canonical control-plane and identity/planning store; `cassette.db` remains the playback/runtime cache.
- MusicBrainz is the canonical identity spine. Qobuz, Deezer, slskd, Usenet, Jackett, Real-Debrid, and yt-dlp are acquisition adapters or evidence sources, not identity owners.
