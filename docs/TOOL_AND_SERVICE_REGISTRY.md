# Tool And Service Registry

Last audited: 2026-04-03

Provider evidence ledger: `PROVIDER_EVIDENCE_LEDGER.md`

## Status Legend

- `local-proven`
- `bounded-probe`
- `unverified`

## Registry

| Tool / Service / Library | Type | Current repo usage | Full practical potential for this app | Constraints / risks | Recommended canonical responsibility | Status |
| --- | --- | --- | --- | --- | --- | --- |
| Tauri 2 | App platform | Desktop shell, Rust commands, plugins | Local desktop orchestration, events, background tasks, native file access | Needs stable command contracts and permission discipline | Desktop shell and command boundary owner | local-proven |
| SvelteKit UI | Frontend | Library/download/settings/tools pages | Candidate review, explainability, diff review, manual approval UX | Downloads now exposes request timeline plus approve/reject review, but richer explainability and policy controls are still limited | Renderer and interaction layer | local-proven |
| SQLite (`cassette-core::db`) | Persistence | Active app runtime store in Tauri app-data `cassette.db` | Lightweight local runtime state | Current schema is not the reconciliation/control-plane store | Active playback/runtime persistence | local-proven |
| SQLite (`librarian`/`library` sidecar schema) | Persistence | Durable sidecar store in Tauri app-data `cassette_librarian.db` | Desired-track memory, reconciliation, deltas, scan checkpoints, operations, invariants | Still not fully surfaced in the UI path | Canonical reconciliation/control-plane store | bounded-probe |
| Director | Orchestrator | Active acquisition waterfall | End-to-end acquisition planning, scoring, fallback, finalization | Planner-stage review is now live via request timeline and approve/reject, but deeper policy lanes are still pending | Canonical acquisition orchestrator | local-proven |
| `downloader/` module | Internal legacy subsystem | Retired (removed from `cassette-core` exports in GAP-D03) | None (historical compatibility surface only) | Reintroduction would reopen parallel-lane risk | Keep removed; do not reintroduce runtime ownership outside Director | local-proven |
| MusicBrainz | External metadata API | Tag-fix search, parent-album resolution, release tracks | Canonical artist/release/release-group/recording identity spine | 1 req/sec etiquette; no audio delivery | Canonical metadata and identity owner | local-proven |
| Cover Art Archive | External artwork API | Runtime metadata tagging fallback when provider artwork is missing and MusicBrainz release ID is present | Artwork retrieval by MBID after canonical release chosen | MusicBrainz release ID required; fallback executes only when provider art is absent | Canonical artwork fallback source after release selection | local-proven |
| Discogs | External metadata API | Metadata search/discography fallback plus enricher release context | Label, format, country, catalog no., secondary release enrich | API/auth and rate limits; credentialed bounded probe captured 2026-04-07 | Secondary enrich, not canonical identity | bounded-probe |
| Last.fm | External metadata API | Player context info, duration enrichment, and recent-track history sync | Tags, bios, related-artists context, user-facing enrichment | Not canonical release authority; 2026-04-07 credentialed probe returned 0/25 context hits on sampled corpus | Contextual enrich for artist/album display and history sync | bounded-probe |
| LRCLIB | External lyrics API | Player lyrics fetch | Synced/static lyrics lookup and caching | Direct endpoint probe captured 2026-04-07 | Lyrics source | bounded-probe |
| Spotify Web API | External metadata API | Search/discography/history import plus direct desired-track import seed | Artist catalog seed, exact track search via filters, library intent expansion | No audio acquisition; OAuth/rate limits | Intent seed and source-alias owner, not download owner | local-proven |
| Qobuz | External provider | Search and acquisition provider | High-trust lossless acquisition and artwork metadata | Depends on non-public credentials/secrets and fragile auth handling | Preferred premium lossless acquisition adapter when available | unverified |
| Deezer | External provider | Search plus private media flow | Search, catalog lookup, and proven full-track acquisition on this machine | Acquisition uses private endpoints and decryption; brittle/legal/durability risk | Secondary premium acquisition adapter | local-proven |
| slskd / Soulseek | External daemon/API | Search, download handoff, transfer polling | Broad long-tail acquisition, browsing, queueing, transfer management | Search concurrency and daemon state issues | Canonical P2P acquisition route | unverified |
| NZBGeek | External index/search API | Usenet search | NZB search/broker | External index availability and API key | NZB search owner if Usenet retained | unverified |
| SABnzbd | External download client | Optional NZB handoff plus queue/history polling work in progress | Queue, history, category handling, completion polling | Completion polling runbook exists; current machine classification depends on configured credentials | NZB execution owner | unverified |
| Jackett | External indexer broker | Active Director provider and `torrent_album_cli` search path | Multi-index torrent search via Torznab | Requires configured Jackett plus Real-Debrid for resolve | Canonical torrent search owner | unverified |
| Real-Debrid | External resolver/hoster API | Torrent cache check, add/select/poll/unrestrict; dedup by hash before addMagnet | Cached torrent acquisition, hoster unrestriction, file selection | Key read from DB `real_debrid_key` or `REAL_DEBRID_KEY` env var | Canonical torrent/hoster resolver, not search owner | local-proven |
| TPB `apibay` | External search endpoint | Explicit `torrent_album_cli --allow-apibay-fallback` fallback only | Album-level emergency torrent discovery when Jackett is unavailable | Low trust, unstructured results, no auth | Debug/emergency fallback only; not default Director search | local-proven |
| 7-Zip | Local CLI | `torrent_album_cli` archive extraction (`C:/Program Files/7-Zip/7z.exe`) | Extract RARs/ZIPs delivered by RD unrestrict links | Must be present at that path | Archive extractor for RD-sourced album downloads | local-proven |
| yt-dlp | External CLI | Fallback candidate and acquisition | Wide extractor coverage, audio extraction, low-trust fallback | Weak metadata provenance and edition certainty | Last-resort fallback acquisition | local-proven |
| Symphonia | Rust crate | Validation/playback support | Decode/probe/audio property validation | Not a metadata writer | Canonical decode/validation layer | local-proven |
| Lofty | Rust crate | Tag reads/writes, tag fixes | Robust metadata normalization and artwork embedding | Needs canonical metadata source upstream | Canonical tag write/read owner | local-proven |
| `ProviderBridge` | Internal compatibility layer | Retired (deleted in GAP-D03; validation now uses local adapter) | None (historical adapter only) | Reintroduction would re-couple old source-provider abstraction | Keep removed; use explicit adapters only where needed | local-proven |
| `BandcampSource` | Internal source | Payload URL resolver | Resolves Bandcamp URL fields from desired payload into a download candidate | No catalog/search integration; relies on upstream payload quality | Source resolver helper only | unverified |

## 2026-04-03 Corrections

These notes override older research/reference text where registry language previously lagged runtime truth:

- Jackett is an active Director provider and the canonical torrent search owner.
- Real-Debrid is the canonical torrent/hoster resolve and unrestrict owner. Its direct TPB search is debug-only and disabled by default in the Director.
- `torrent_album_cli` treats apibay as explicit fallback only via `--allow-apibay-fallback`; it is no longer the default torrent search path when Jackett is available.
- Spotify is not just search/discography seed. The direct desired-track import path now persists source IDs, best-effort ISRC, and richer raw payload JSON.
- `cassette_librarian.db` is the canonical control-plane and identity/planning store; `cassette.db` remains the playback/runtime cache.
- MusicBrainz is the canonical identity spine. Qobuz, Deezer, slskd, Usenet, Jackett, Real-Debrid, and yt-dlp are acquisition adapters or evidence sources, not identity owners.
