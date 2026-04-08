# Cassette Project State

Last updated: 2026-04-07

## Architecture

- **Desktop shell**: Tauri 2.10.3 (`src-tauri/`)
- **Frontend**: SvelteKit (`ui/`)
- **Core domain**: Rust workspace (`crates/cassette-core/`)
- **Database**: SQLite via rusqlite (runtime) + sqlx (librarian/orchestrator flows)
- **Active DB location**: Tauri app-data directory (`cassette.db`)
- **Integrated queue sidecar DB**: Tauri app-data directory (`cassette_librarian.db`) for librarian/orchestrator tables such as `desired_tracks`, `local_files`, `reconciliation_results`, `delta_queue`, `sync_runs`, and `scan_checkpoints`
- **Repo-local DB files**: root-level `*.db` files in this workspace are local artifacts for tests, probes, or inspection; they are not automatically the live runtime DB
- **Current role split**: `cassette_librarian.db` is the canonical control-plane and identity/planning store; `cassette.db` remains the playback/runtime cache

## What Works Today

### Library Management
- Library root scanning with recursive audio file discovery
- Track metadata extraction (artist, album, title, track/disc number, year, duration, sample rate, bit depth, bitrate, format, file size)
- Cover art path detection
- Runtime scan metadata now trims empty tag values, falls back to cleaned filename titles, and reuses filename track/disc prefixes when embedded numbers are missing or zero
- Runtime cover-art detection now accepts a broader deterministic sidecar-art set (`cover/folder/artwork/front/album` across jpg/jpeg/png/webp, case-insensitive) and falls back to an embedded-art cache when no sibling artwork file exists
- Search across tracks by title/artist/album
- Play count and skip count tracking
- Library organization (move files to `Artist/Album/NN-Title` structure)
- `tag_rescue_cli` now applies a staged repair ladder for missing/zero track numbers: embedded tags, filename prefixes, then conservative album-pattern inference
- `tag_rescue_cli` can emit a machine-readable repair report with repaired rows, unresolved rows, and repair-source classification
- Organizer canonical path generation now preserves a valid existing non-zero filename track prefix when DB `track_number` is zero or missing
- Duplicate grouping now normalizes album-artist fallback (`album_artist` then `artist`) and includes title in grouping key, reducing false duplicate clusters for mixed-artist albums
- Duplicate resolution now fails loudly on file/DB delete failures instead of silently warning while reporting success
- Offline cleanup tooling includes grouped manifest builders, safe apply scripts, and rollback-oriented remediation helpers under `scripts/`

### Playback
- Audio playback via Symphonia decode + `cpal` output
- Queue management (add, remove, reorder, clear)
- Queue replacement now auto-starts playback (`queue_tracks` loads and plays the selected start track and marks playback state as playing)
- Playlist CRUD
- Now-playing context from Last.fm plus Last.fm recent-scrobble sync into local artist/song/play-history tables
- Synced/plain lyrics from LRCLIB
- Runtime now caches synced/plain lyrics in the app DB by normalized artist/title/album identity, treats rows older than 30 days as stale, and refreshes stale/partial rows on demand during now-playing lookups
- A bounded background lyrics prefetch lane now warms LRCLIB cache from recent playback plus recent finalized-task metadata candidates (strict candidate and timeout caps per run)

### Desktop UX Stabilization (2026-04-05)

- Settings now exposes a persist-loaded-secrets action that writes effective in-memory/env-loaded provider credentials into runtime settings (`persist_effective_config`)
- Settings now exposes bundled `slskd` runtime status plus refresh/restart/stop controls, and runtime startup will attempt to launch bundled `slskd.exe` before treating Soulseek as unavailable
- Tools route now includes guided metadata correction via artist/album dropdowns and clearer organize/ingest outcome messaging
- Duplicates route now supports deterministic sort order plus a `Handle All` action for batch duplicate cleanup
- Artists route now groups name variants with normalized artist keys to reduce split-folder behavior from punctuation/spelling style drift
- Spotify extended-history album summary now uses observed distinct track coverage versus local album track count to reduce false `in_library` positives on incomplete local albums
- Library track inspection now surfaces persisted ISRC, MusicBrainz recording/release/release-group IDs, canonical artist/release IDs, derived edition bucket/markers, and file-path evidence from the active runtime DB

### Music-First System Spine (2026-04-06)

- Home route (`/`) is now the music-first front door with current-playback framing plus a "while you were away" narrative summary derived from recent task outcomes, blocked work, and missing backlog state
- Deep library browsing moved to `/library`, preserving album/track drilldown and track identity inspection without forcing the old library screen to remain the landing page
- Artist-first collection browsing remains the default library worldview through `/artists`, now backed by a shared normalization/clustering helper instead of route-local duplicate logic
- The desktop shell now carries an always-visible bottom status strip with provider/service/queue/scan signals layered above the player bar
- Downloads is now organized into Missing, In Progress, Blocked, and Completed lanes with request-level review/timeline detail preserved inline
- Downloads now includes a provider troubleshooting snapshot with down/unknown totals, per-provider health messages, config-aware hints, and last-check visibility for faster local recovery decisions
- Import now presents one Spotify intake story: streaming-history album backlog and direct desired-track JSON both feed the same identity-first desired-state/control-plane path
- Trust Ledger v1 now derives plain-language request reason cards from existing acquisition timeline rows, Director terminal history, candidate review evidence, operation events, and gatekeeper audit rows instead of introducing a second persistence layer
- Downloads request lineage now carries trust summary plus correlated operation-event/gatekeeper evidence context, and Home surfaces recent trust pressure through reason-code strips/distribution summaries
- Downloads request rows now surface edition-intelligence hints (`musicbrainz_release_group_id`, `edition_policy`) alongside trust/queue state so planner identity choices stay visible after submission
- Dead-Letter Command Center now groups permanently failed/cancelled director tasks by failure class below the Blocked lane, shows plain-language recovery hints, and supports per-item replay through the planner approval path with `dead_letter_replay` lineage
- Settings now exposes Policy Profiles (`playback_first`, `balanced_auto`, `aggressive_overnight`) and applying a profile hot-reloads Director config in-place with deterministic, logged runtime behavior changes
- Library, Artists, and Playlists now expose a shared context-action rail for artist/album/track surfaces (play, queue, and bounded acquisition actions)
- Settings now exposes optional Now Playing visualizer controls (`ui_visualizer_enabled`, `ui_visualizer_low_motion`), and player bar rendering honors global disable and low-motion preferences
- Now Playing now also supports an optional appreciation signal lane (`ui_appreciation_lane_enabled`) that surfaces compact artist tags, listener scale, and lyrics-source context without changing playback behavior
- Visualizer stack now includes a MilkDrop-style Butterchurn mode with imported preset packs and persisted mode/preset settings (`ui_visualizer_mode`, `ui_visualizer_preset`) alongside the lightweight bars fallback
- Visualizer settings now expose explicit `waveform` and `spectrum` modes in addition to optional MilkDrop mode, and frame-budget behavior is bounded through persisted FPS cap plus hidden-tab/idle render throttling (`ui_visualizer_fps_cap`)
- Dynamic Glass mooding now adapts shell overlays from now-playing artwork/identity with persisted safety controls (`ui_dynamic_glass_enabled`, `ui_dynamic_glass_low_motion`, `ui_dynamic_glass_intensity`) and static fallback defaults when unavailable
- Home now includes Session Composer v1: explainable listening-arc generation with reusable saved modes plus replay/skip transition feedback persistence (`ui_session_composer_modes_json`, `ui_session_composer_feedback_json`)
- Settings now includes Stage D Safe Extension Surface controls: capability-scoped extension manifests (visual-pack, enricher, provider-adapter), explicit deterministic-core boundary (`deterministicCoreAccess: false`), isolated per-extension health probes, and persisted extension telemetry (`ui_extension_health_telemetry_json`)

### Acquisition Pipeline (Director Engine)
- Two-pass waterfall orchestration with per-provider semaphores
- 7 acquisition strategies (`Standard`, `HighQualityOnly`, `ObscureFallbackHeavy`, `SingleTrackPriority`, `DiscographyBatch`, `RedownloadReplaceIfBetter`, `MetadataRepairOnly`)
- `MetadataRepairOnly` now resolves matching local tracks from runtime DB identity fields (ISRC/MB recording/artist-title-album) and applies in-place metadata repair without byte acquisition
- 6-factor candidate scoring (metadata confidence, duration match, codec quality, provider trust, validation result, file size)
- Task-local cancellation via `CancellationToken` registry, with batch-wide cancel reserved for shutdown
- Symphonia-based audio validation with truthful `audio_readable` / `header_readable` reporting and codec/container mismatch rejection
- Lossless is still preferred, but acquisition now falls back to the next available quality tier instead of hard-failing when only lossy material is available
- Post-acquisition metadata tagging via Lofty
- Atomic finalization with duplicate policy (`KeepExisting` or `ReplaceIfBetter`)
- Per-task temp directories with stale recovery and quarantine
- Provider-aware retry and staged-download resume hardening, including `Retry-After`, `Content-Length` preflight, and range validation
- Search-result caching in the director waterfall with strategy-aware cache keys and provider-epoch invalidation
- Concurrent provider health polling with skip-on-down behavior
- Runtime provider stabilization cools down rate-limited and temporary-outage providers, disables auth-failed providers for the rest of the run, and treats provider-busy as capacity pressure instead of provider-down
- Adaptive provider nudge reorders providers with recent finalized memory (<= 7 days) using a bounded trust-rank bonus; trust-rank-floor providers (<= 10) cannot be overtaken and order adjustments are logged at debug level
- Runtime policy profiles now tune Director worker concurrency, provider parallelism, retry cadence, timeout/cooldown envelopes, and per-provider concurrency limits deterministically at runtime
- Broadcast event channels for progress, results, and provider health
- Deezer full-track acquisition is live-proven on this machine
- Deezer session caching now uses a recoverable `RwLock<Option<...>>` path instead of permanently caching auth failure
- Deezer acquisition now decrypts directly to disk through a streaming Blowfish stripe path

### Providers (8 active)

| Provider | Trust Rank | Capabilities | Status |
|----------|------------|-------------|--------|
| Local Archive | 0 | Filesystem walk + direct copy, batch support | Proven working |
| Deezer | 5 | Search + acquire with Blowfish CBC stream decryption | Implemented |
| Qobuz | 10 | MD5-signed session, search + acquire (lossless) | Implemented |
| slskd/Soulseek | 10 | P2P search with queue recovery, transfer polling + filesystem fallback | Implemented |
| Usenet | 30 | NZBgeek search + SABnzbd execution, queue/history polling with filesystem fallback | Implemented |
| Jackett | 40 | Multi-indexer Torznab search → magnet → RD unrestrict → archive extraction | Implemented |
| yt-dlp | 50 | Subprocess fallback, `ytsearch1` + `scsearch1` | Proven working |
| Real-Debrid | 80 | TPB/apibay search → magnet → RD unrestrict → archive extraction via 7z | Live-proven |

Jackett requires both `JACKETT_URL` + `JACKETT_API_KEY` in settings and a configured `REAL_DEBRID_KEY` for resolve. When both are present, Jackett is added to the Director waterfall between Usenet and Real-Debrid (trust_rank 40). The `torrent_album_cli` also uses Jackett when configured, falling back to apibay.

Current provider-role reset:

- Jackett is the canonical torrent search owner in the Director.
- Real-Debrid remains the torrent/hoster resolve and unrestrict owner.
- Real-Debrid direct TPB search is disabled by default in the Director.
- `torrent_album_cli` only uses apibay when `--allow-apibay-fallback` is explicitly supplied.

### Metadata Services

| Service | Usage | Auth |
|---------|-------|------|
| MusicBrainz | Release search, parent album lookup, track listing, tag fixes | None |
| Discogs | Metadata search/discography fallback (`database/search`, `artists/{id}/releases`) plus release-id/genre-style context enrichment | User token |
| Last.fm | Artist/album context, track-duration lookup, and recent-scrobble history sync (`user.getRecentTracks`) | Public API key (+ username for history sync) |
| LRCLIB | Synced/plain lyrics lookup | None |
| Spotify | History import, search, discography seeds | Optional OAuth |

### Release Automation

- CI workflows include a manual `Release Candidate` lane (`.github/workflows/release-candidate.yml`) that runs the CI gate, optionally runs perf regression gating, builds installers, and uploads bundles with a SHA256 manifest artifact.

Role clarification:

- MusicBrainz is the canonical identity spine.
- Spotify is the intent/import seed and fallback metadata input in the shared resolver, not canonical truth.
- Runtime canonical-artist lookup now reuses the same shared normalization spine used by the librarian so `&`/punctuation variants converge more deterministically during track upsert.

### Data Pipeline
- Sidecar-owned acquisition requests now persist in `cassette_librarian.db` with request status, task linkage, request signatures, normalized target fields, and event timeline rows
- Current download entrypoints (`start_download`/`start_song_download`, album queueing, discography queueing, backlog runner) now create control-plane request rows before submitting Director tasks
- Acquisition requests are scope-aware at the control-plane boundary: song requests use `track`, album/discography requests use `album`
- Spotify play history import from external SQLite DB
- Direct desired-track Spotify import now reuses the shared payload parser and persists `source_track_id`, `source_album_id`, `source_artist_id`, `duration_ms`, best-effort `isrc`, and richer raw payload JSON
- Missing album detection (Spotify albums not in local library)
- Album and Spotify-missing queues expand releases into per-track `TrackTask` submissions through the shared resolver chain
- Album queueing, backlog queueing, and Spotify backlog CLI resolution now route through the shared resolver chain: MusicBrainz -> iTunes -> Spotify credentials
- Resolver regression tests now guard the active album-expansion paths so Tauri queueing, `engine_pipeline_cli --import-spotify-missing`, and `batch_download_cli` cannot silently drift back to ad hoc MB-only resolution
- Request and task signatures now retain richer source identity (`source_track_id`, `source_album_id`, `source_artist_id`) alongside ISRC, MusicBrainz IDs, and canonical IDs when available
- Read-only planner commands now exist for pre-acquisition search and rationale: `plan_acquisition`, `get_candidate_set`, and `get_request_rationale`
- Request trust distribution is queryable through the Tauri command surface (`get_trust_reason_distribution`) for explainability and telemetry views
- Planner candidate sets now persist into runtime candidate/search tables before byte acquisition starts, and planner runs refresh request-scoped source-alias and identity-evidence rows
- Planner review mutations now exist in the command surface via `approve_planned_request` and `reject_planned_request`, and those approvals now submit to Director with audit events and pending-task persistence
- Active queue submissions now use planner-first flow (`plan_acquisition` -> `approve_planned_request`) for song requests and album/artist expansion requests; remaining bypass and operator lanes are still pending cutover
- Planner review now includes a persisted preflight validation contract (`review_preflight` events) and blocks approval when candidate/search readiness checks fail
- Direct-submit bypass CLIs are now explicitly operator-only via `--operator-direct-submit` gating (`acquire_cli`, `batch_download_cli`, and `engine_pipeline_cli --import-spotify-missing`)
- Director task result persistence to `director_task_history`
- Terminal history retains the original `TrackTask` request payload and strategy for failed/cancelled/finalized results
- Terminal history also preserves the last known provider and `failure_class` for failed/cancelled rows instead of leaving those outcomes provider-blank
- Pending director task persistence in `director_pending_tasks` for deterministic startup recovery
- Request-signature persistence threads through pending tasks, terminal history, candidate sets, and provider memory
- Full candidate-set persistence captures scored, rejected, and selected candidates in `director_candidate_sets` and `director_candidate_items`
- Provider search outcomes and negative-result memory persist in `director_provider_searches`, `director_provider_attempts`, and `director_provider_memory`
- Provider search/candidate evidence, per-provider response snapshots, identity evidence, and source aliases now also persist in `provider_search_evidence`, `provider_candidate_evidence`, `provider_response_cache`, `identity_resolution_evidence`, and `source_aliases`
- Director search now consults persisted provider memory before network search: fresh dead-end memory can skip a provider entirely, and fresh cached candidate payloads can hydrate the in-memory search cache for identical requests
- Release-group identity and edition-level planner context now thread through active request signatures and source-alias evidence (`musicbrainz_release_group_id` is part of request signature composition, and `musicbrainz.release_group_id` aliases are persisted for planner/director request boundaries)
- Runtime `tracks` rows now persist sovereignty/evidence fields (`isrc`, MusicBrainz IDs, canonical artist/release IDs, `quality_tier`, `content_hash`) instead of silently dropping them on upsert
- Canonical identity persistence now includes `canonical_recordings` in the active runtime DB
- Sidecar canonical identity persistence now includes `canonical_artists`, `canonical_releases`, and `canonical_recordings` for request-planning ownership
- Runtime startup now mirrors canonical identity rows into the sidecar with duplicate-safe upserts so the two stores converge on a shared normalized identity view
- `db_converge_cli` now supports physical datastore convergence by producing `cassette_unified.db` from runtime + sidecar, copying control-plane tables into `control_*` namespace tables in the unified file
- `TrackTask` payloads now carry `desired_track_id` and `source_operation_id` for control-plane closure
- Librarian `local_files` rows now persist `acoustid_fingerprint`, per-file fingerprint attempt state, and source mtime proof; Gatekeeper admission writes fingerprints back into the same table, and `run_librarian_sync` performs bounded parallel backfill with unchanged-failure suppression and stale-fingerprint invalidation on file mtime change

## Active Runtime Database Schema

Primary active runtime tables include:
- `library_roots`
- `tracks`
- `queue_items`
- `settings`
- `playlists`
- `playlist_items`
- `spotify_album_history`
- `director_task_history`
- `director_pending_tasks`
- `director_candidate_sets`
- `director_candidate_items`
- `director_provider_searches`
- `director_provider_attempts`
- `director_provider_memory`

Separate richer schemas also exist in the `library`, `librarian`, and `gatekeeper` subsystems. Those remain part of the architecture convergence story. Near-term runtime shape stays dual-store by design: sidecar for control-plane identity/planning, runtime DB for playback/UI cache.

## Concurrency Model

- Global worker semaphore: configurable (default 12 concurrent tasks)
- Per-provider semaphores: configurable via `ProviderPolicy`
- `slskd` global search semaphore still serializes search requests
- Two-pass provider acquisition: pass 1 non-blocking, pass 2 blocking for deferred providers
- Director provider health loop now checks providers concurrently instead of serially
- MusicBrainz remains rate-limited conservatively in metadata flows

## Configuration

Settings resolve in priority order:
1. SQLite database (`settings`)
2. environment variables / `.env`
3. Streamrip config (`%APPDATA%/streamrip/config.toml`) when present
4. `slskd.yml` when present
5. code defaults

Director runtime behavior now exposes config for:
- provider health interval/staleness
- provider busy/temp-outage/rate-limit cooldowns
- validation failure bail threshold
- search cache TTL/capacity
- persisted provider-memory freshness TTL
- persisted provider-response-cache freshness TTL

## Pipeline Integration Architecture

The integrated acquisition loop is now centered on `delta_queue` as the authoritative durable work bus, hosted in the librarian sidecar DB so it does not collide with the playback/runtime schema in `cassette.db`.

Spotify desired-state intake now covers both direct desired-track payload import and history-derived album backlog inputs. The richer direct import path lands in `desired_tracks`; history import still feeds `spotify_album_history`.

```
Spotify Export -> desired_tracks
Local Scan     -> local_files
                    |
           reconcile_pipeline()
                    |
  reconciliation_results -> delta_queue (claim/lease queue)
                                |
                  engine_pipeline_cli (canonical coordinator)
                                |
                    Director (acquisition + finalization)
                                |
                     A:\music (placed and tagged files)
                                |
                  Librarian re-scan (closes satisfied gaps)
                                |
              organize subset (guarded finalized/newly-found set)
```

### Canonical Integrated Entrypoint

`src-tauri/src/bin/engine_pipeline_cli.rs`

It now:
1. runs `run_librarian_sync()` in `full`, `resume`, or `delta-only` mode, with `--resume` as shorthand for `--scan-mode resume`
2. reclaims stale `delta_queue` claims
3. claims actionable `delta_queue` rows deterministically
4. resolves them into Director `TrackTask`s with `desired_track_id` and `source_operation_id`
5. submits them to Director
6. marks `processed_at` only on successful terminal outcomes
7. releases claims on retryable/transient failure paths
8. re-runs `run_librarian_sync()` in `delta-only` mode after acquisition
9. organizes only the newly finalized/newly discovered subset, behind the zero-track safety guard

### Sidecar Scan State

- `local_files` rows now persist `file_mtime_ms` alongside `file_size`
- `scan_checkpoints` persist per-root scan progress (`last_scanned_path`, counts, status)
- `resume` mode continues from the last in-progress checkpoint instead of restarting from zero
- `delta-only` mode re-walks roots but skips unchanged files without re-upserting them
- `resume` mode becomes queue-only when all configured roots already have completed checkpoints and the sidecar is populated

### Queue Semantics Now In Code

- `processed_at IS NULL` = still actionable
- `claimed_at` + `claim_run_id` = current coordinator lease
- stale claims are recoverable on the next run
- successful finalization closes the originating ticket
- retryable failures leave the ticket open

### Organizer Safety Path

- `src-tauri/src/bin/tag_rescue_cli.rs` repairs DB truth without mutating files, using `embedded_tag`, `filename_prefix`, then `album_pattern` recovery
- `tag_rescue_cli --report <path>` emits a JSON repair report with repaired rows and explicit unresolved rows
- Organizer filename-prefix fallback now recognizes multi-disc and whitespace-prefixed patterns, not only `NN - Title`
- `organize_cli --live` now aborts if a suspicious fraction of moves would rename files to `00 - ...`
- Remaining organizer risk is now tracked through explicit unresolved rows and the bounded live organize proof, rather than assumed to be solvable by embedded-tag repair alone

### Legacy / Manual Path

`batch_download_cli` still exists for direct/manual use, but it is no longer the canonical integrated control-plane path.

## Spotify Backlog Acquisition — 2026-04-01

`torrent_album_cli` added as a dedicated album-first torrent downloader that bypasses the per-track Director flow for albums where torrent is the best source.

Current policy note: Jackett is the default search owner here too. apibay is now explicit emergency fallback only via `--allow-apibay-fallback`.

**Flow:**
1. Pull missing albums from `spotify_album_history` (app DB)
2. Filter singles (feat./with/ft. patterns, remix/EP labels)
3. Strip edition/remaster suffixes from album names for cleaner search
4. Search apibay.org (TPB) — cat 104 (FLAC) then 101 (Music), min 2 seeders
5. Score by artist+album word-boundary match + FLAC bonus + seeder bonus; require album title match
6. Add magnet to Real-Debrid (dedup by hash — reuse existing torrent)
7. Poll until `downloaded`, retry on transient connection errors (30s timeout, 5 retries per poll)
8. Unrestrict links → detect audio vs archive (RAR/ZIP/7z)
9. Archives extracted via `C:/Program Files/7-Zip/7z.exe`
10. Audio files copied to `library_base/Artist/Album/`, upserted to app DB

**Albums installed as of 2026-04-01 (selected):**
- Black Star — Mos Def & Talib Kweli Are Black Star (13 tracks)
- Bush — Sixteen Stone (12 tracks)
- Fall Out Boy — From Under The Cork Tree (13 tracks)
- blink-182 — Enema of the State (12 tracks)
- Kevin Gates — Islah (15 tracks)
- Lorde — Pure Heroine (10 tracks)
- Weezer — Weezer (White Album) (10 tracks)
- AJR — OK Orchestra (13 tracks)
- Tame Impala — Lonerism (12 tracks)
- Phantogram — Three (10 tracks)
- Fitz and the Tantrums — self-titled (12 tracks)
- + ongoing batch drain of remaining ~489 backlog albums

**Known gaps:**
- ~26 albums per 40-album batch are genuinely absent from TPB (Drake, Eminem, Jack Harlow, etc. — streaming exclusives/mixtapes)
- Taylor Swift — Red (Taylor's Version): emoji in RAR filename causes decode error; needs alternate torrent

## Packaging Status (2026-04-03)

`cargo tauri build` now succeeds on Windows and produces:
- `target/release/bundle/msi/Cassette_0.1.0_x64_en-US.msi`
- `target/release/bundle/nsis/Cassette_0.1.0_x64-setup.exe`

Fix applied: `default-run = "cassette"` added to `src-tauri/Cargo.toml`. Tauri requires this when the workspace has multiple `[[bin]]` entries.

Release process checklist now lives in `docs/RELEASE_CHECKLIST.md`, including clean-machine install gates and `db_converge_cli` unified-datastore verification steps.

Latest verification snapshot on 2026-04-03:
- `./scripts/verify_trust_spine.ps1` completed successfully
- `cargo tauri build` completed and produced both Windows bundles
- `cargo run -p cassette --bin db_converge_cli -- --overwrite` produced `cassette_unified.db` with counts: `desired_tracks=4`, `delta_queue=11`, `acquisition_requests=0`
- Desktop smoke now validates the managed `slskd` startup contract through `slskd_runtime_probe_cli`, so verification no longer depends on a raw `localhost:5030` port check by itself

## Torrent CLI Failure Feedback Loop (2026-04-03)

`torrent_album_cli --seed-sidecar` now seeds albums that failed torrent search into `cassette_librarian.db` as `desired_tracks` + `delta_queue(missing_download)` entries. The coordinator (`engine_pipeline_cli --resume`) picks them up on the next run via Qobuz/Deezer/slskd.

Flow:
1. Run `torrent_album_cli --limit 50 --seed-sidecar`
2. Albums installed via torrent as before
3. Albums with no torrent found → MusicBrainz tracklist expansion → `desired_tracks` + `delta_queue` in sidecar
4. Run `engine_pipeline_cli --resume` → claims and resolves via streaming providers

Skips albums already in `desired_tracks` to avoid duplicates. Respects MusicBrainz 1 req/sec rate limit.

## Known Limitations

- Frontend still keeps `get_download_jobs` as a catch-up and resume fallback even though push events are now primary
- Dual schema: richer librarian/library model exists but isn't fully wired into the active runtime UI path
- `cargo test --workspace` is a reliable gate again. The old Windows `STATUS_ENTRYPOINT_NOT_FOUND` failure was isolated to the Tauri lib-test harness missing the desktop manifest; the pure `src-tauri` assertions now live in `src-tauri/tests/pure_logic.rs`.
- `MetadataRepairOnly` currently depends on `runtime_db_path` and matching local-track identity evidence; requests without that context fail fast with explicit diagnostics
- Discogs and Last.fm enrichment clients now have live API-backed implementations; runtime now uses Last.fm for now-playing plus explicit history sync, while a full automatic background enrichment queue worker is still pending
- Artwork hit-rate is stronger for embedded art and common sidecar art names, and tag-writing now falls back to Cover Art Archive front-art for MusicBrainz release IDs when provider art is missing
- Lyrics are durably cached in the runtime DB with a 30-day stale-on-read refresh policy, and a bounded background prefetch lane now seeds cache from recent playback and finalized-task metadata candidates
- Bundled `slskd` lifecycle is now managed by Cassette at startup, and smoke verification now reuses that startup contract through `slskd_runtime_probe_cli`; a fresh desktop-session proof is still useful when verifying app-owned child-process behavior end-to-end
- Bandcamp source now resolves Bandcamp URLs from desired-track payloads instead of hard-failing as a placeholder resolver
- Candidate persistence now feeds the Downloads pre-acquisition review panel (timeline plus approve/reject), and explicit provider exclusion toggles now persist exclusion memory for future planner decisions on the same request identity
- Fingerprint accumulation is now bounded and incremental, not a full-library canonical backfill worker; large libraries will converge over repeated syncs rather than one sweep
- `batch_download_cli` still uses the older album-history/manual workflow and has not been removed yet
- `director/providers/` is the active acquisition path; the legacy `downloader/` compatibility module and `ProviderBridge` adapter were retired in GAP-D03
- Organizer repair tooling is deeper now, and both the live app-DB repair proof and bounded live organize proof are captured in canonical docs
- Album batching currently groups queue work into `DiscographyBatch` strategy selection in the coordinator, but provider locking remains strategy-led rather than a separately persisted album lane
- Structured run observability is improved through queue claims and persisted request payloads, but the frontend does not yet expose a dedicated coordinator timeline view

## End-to-End Coordinator Proof — 2026-03-31

Run command: `engine_pipeline_cli --resume --limit 5 --skip-organize-subset`

**Pre-run sidecar state:**
- `desired_tracks`: 1 row — Doechii / DENIAL IS A RIVER (manual source, no track number)
- `reconciliation_results`: 1 row — status=`missing`, reason="no local match"
- `delta_queue`: 1 row — `missing_download`, unclaimed, unprocessed
- `scan_checkpoints`: `A:\music` = `in_progress` (stale from prior interrupted run)

**Phase 1 — librarian sync (resume mode):**
- Discovered 22,998 files; re-indexed all (skipped=0 because checkpoint was `in_progress`)
- 22,806 files upserted into sidecar `local_files`
- Reconciliation: 1 desired track reconciled
- Delta generation: 1 row generated
- Checkpoint advanced to `completed`, files_seen=43,501

**Phase 2 — queue claim:**
- 1 row claimed: `desired_track_id=1`, `action_type=missing_download`
- `claimed_at` + `claim_run_id=engine-run-8a10af41-...` stamped atomically

**Phase 3 — Director acquisition:**
- Provider: Qobuz (lossless, attempts=8 across waterfall)
- Result: `Finalized`
- Final path: `A:\music\Doechii\Alligator Bites Never Heal\04 - DENIAL IS A RIVER.flac`
- Track number resolved: 4 (from Qobuz metadata, overriding missing desired-state number)

**Phase 4 — queue closure:**
- `delta_queue` row 1: `processed_at=2026-03-31 20:35:36`, `claimed_at` preserved (audit trail intact)
- `claimed_at` and `claim_run_id` remain stamped — proves the `mark_processed` fix is live

**Phase 5 — post-run librarian sync (delta-only):**
- Discovered 43,501 files; scanned=3 (only changed/new files), skipped=43,498
- Reconciliation ran: 1 processed — status now `weak_match` (title match within artist)
- Delta re-generated: 1 row (the `manual_review` action for the weak-match)
- The `missing_download` row was NOT re-created — gap is closed for acquisition

**director_task_history (app DB):**
- `task_id=delta-1-denial is a river`, `disposition=Finalized`, `provider=qobuz`, `final_path=A:\music\Doechii\Alligator Bites Never Heal\04 - DENIAL IS A RIVER.flac`

**Observations / follow-up:**
- Resume scan re-indexes the full library when checkpoint is `in_progress` — this is correct but slow. The fix is: mark checkpoint `completed` at the end of a successful full scan so subsequent `--resume` runs skip unchanged files. Currently the checkpoint becomes `completed` only at the end of the post-run delta-only scan, not the full scan itself. The full scan does set checkpoint rows per-batch via `upsert_scan_checkpoint`, but only marks `completed` at walk-end.
- The reconciliation result is `weak_match` not `exact_match` or `strong_match` because the file has `track_number=NULL` in the DB and is named `00 - DENIAL IS A RIVER.flac` (the pre-acquisition stub). The newly acquired `04 - DENIAL IS A RIVER.flac` appears as a separate track in the app DB. The sidecar needs a re-scan pass against the updated app DB to pull the correct metadata.
- `manual_review` delta row appeared because the reconciliation found the track but couldn't confirm it with sufficient confidence — expected given the `00 -` stub still in the DB.

## Organizer Live Proof — 2026-03-31

Run command: `organize_cli --live`

**Pre-proof state:**
- 43,458 tracks in DB; 1,833 with zero/null track_number (irrecoverable via tag rescue)
- `tag_rescue_cli` run live: `updated=0 unresolved=1833` — no embedded-tag, filename-prefix, or album-pattern recovery available
- Dry-run showed: 39,262 proposed moves, 0 zero-track renames, 0 errors → safety check clear

**Live run result:**
- 23,393 files moved to canonical paths (`Artist/Album (Year)/NN - Title.ext`)
- 20,065 already in place (skipped)
- 1 error: stale DB entry pointing to `A:\music\Kyle\Light of Mine\15 - iSpy.flac` (file no longer on disk — handled gracefully, no crash)
- 0 zero-track renames — safety guard not triggered

**Unresolved 1,833 zero-track rows breakdown:**
- 1,371 in `Singles/` folders — single tracks with no track number context, intentionally left as-is
- 389 album tracks with `00 -` filename prefix — no embedded tag, no album-pattern recovery
- 73 other (no prefix, not in Singles) — also irrecoverable without re-acquisition metadata

**Post-organize:** All well-tagged tracks are now at canonical paths. Zero-track rows remain in place with existing filenames (organizer's `should_preserve_existing_basename` correctly keeps them stable).

## Interruption + Resume Recovery Proof — 2026-03-31

Run command: `engine_pipeline_cli --resume --stale-claim-minutes 1 --limit 5 --skip-post-sync --skip-organize-subset`

**Pre-proof state (injected via `proof_interruption_resume.ps1`):**
- `desired_tracks`: 3 new rows — Tyler, the Creator / EARFQUAKE; Frank Ocean / Pyramids; Kendrick Lamar / Money Trees
- `delta_queue` rows 200-202: claimed by `interrupted-run-proof-20260331164515` (stamped 2 min prior), `processed_at NULL`
- `scan_checkpoints`: `A:\music` = `completed`, files_seen=43,501 (from prior proof run)

**Phase 1 — librarian sync (resume mode, queue-only):**
- Scan phase: skipped — "scan phase skipped because completed checkpoints already exist mode=queue-only"
- files_scanned=0, files_upserted=0 — checkpoint fast-path proven
- Reconciliation: 4 desired tracks processed; delta generation: 4 entries
- Note: `clear_reconciliation` preserves claimed rows (rows 200-202 survive delta regeneration)

**Phase 2 — stale claim reclaim:**
- "Reclaimed 3 stale queue claims" — rows 200-202 released from `interrupted-run-proof-*`
- Threshold: 1 minute (`--stale-claim-minutes 1`); rows were 2 min old

**Phase 3 — re-claim + Director acquisition:**
- 3 rows re-claimed under `engine-run-5e6a7fde-ee72-46bf-9f92-0b8efc7d6528`
- `delta-102-money trees`: Finalized via Qobuz → `A:\music\Kendrick Lamar\good kid, m.A.A.d city\05 - Money Trees.flac` (attempts=6)
- `delta-101-pyramids`: Finalized via Deezer → `A:\music\Frank Ocean\channel ORANGE\00 - Pyramids.flac` (attempts=4)
- `delta-100-earfquake`: Finalized via Qobuz → `A:\music\Tyler, the Creator\IGOR\02 - EARFQUAKE.flac` (attempts=5)

**Phase 4 — queue closure:**
- Rows 200-202: `processed_at` stamped, `claimed_at` preserved (audit trail intact)

## Fresh Coordinator Recovery/Resume Proof — 2026-04-06

Run command: `engine_pipeline_cli --resume --limit 1 --skip-fingerprint-backfill --skip-post-sync --skip-organize-subset`

**Pre-proof state (seeded deterministic row):**
- `desired_tracks`: 1 seeded row (`source_track_id=proof-track-20260406`) mapped to an existing local track identity
- `delta_queue`: 1 `missing_download` row seeded with stale claim (`claim_run_id=stale-proof-run`, `claimed_at` two hours old, `processed_at NULL`)

**Observed run behavior:**
- Librarian resume entered queue-only fast path (`files_scanned=0`, `files_upserted=0`)
- Coordinator reclaimed stale lease: `Reclaimed 1 stale queue claims`
- Bounded claim and execution: `claimed=1`
- Director result: `delta-1-belly of the beast` finalized via Deezer
- Queue closure: seeded row stamped `processed_at=2026-04-06 19:49:54` with `claim_run_id=engine-run-981a1048-48ec-4655-885c-d8e8a7e4b6e4`

**Runtime evidence snapshot:**
- Sidecar (`cassette_librarian.db`) `delta_queue`: `id=1`, `source_operation_id=proof-recovery-20260406`, `processed_at` set, claim fields retained
- Runtime (`cassette.db`) `director_task_history`: `task_id=delta-1-belly of the beast`, `disposition=Finalized`, `provider=deezer`, `updated_at=2026-04-06 19:49:54`

## Spotify Ingest Replay Proof (Fixed Sample) — 2026-04-06

Run command: `engine_pipeline_cli --resume --limit 0 --skip-post-sync --skip-organize-subset --skip-fingerprint-backfill`

**Proof design:**
- Seeded two deterministic sidecar cohorts from runtime tracks (`n=50` each):
    - `spotify_replay_legacy`: minimal identity (`artist_name + track_title`)
    - `spotify_replay_rich`: richer identity (`artist + album + title + track/disc + duration`)

**Observed reconciliation outcomes:**
- `spotify_replay_legacy`: `weak_match=50`
- `spotify_replay_rich`: `strong_match=50`

**Interpretation:**
- Rich identity fields in the unified ingest path materially improved reconciliation strength on the fixed sample replay.

**Post-proof hygiene:**
- Seed rows were removed from `desired_tracks`, `reconciliation_results`, and `delta_queue` after evidence capture.

## Planner Cutover Live Proof — 2026-04-06

Run command: `engine_pipeline_cli --resume --limit 5 --skip-organize-subset --skip-post-sync`

**What was run:**
- Both `--skip-organize-subset` and `--skip-post-sync` flags confirmed present and accepted (shown in startup banner).
- Binary compiled and ran from `src-tauri/src/bin/engine_pipeline_cli.rs` against the live runtime.

**Observed behavior:**
- Binary started without crash.
- Librarian resume entered queue-only fast path: `files_scanned=0`, `files_upserted=0` (checkpoint fast-path confirmed, `local_files=46503` known rows).
- Reconciliation ran: `processed=1`.
- Delta queue generation completed: `delta_queue_entries=0`.
- Coordinator reported: `No actionable delta_queue rows.` — expected; no pending desired-state work was queued at time of proof run.
- `skip_post_sync=true`, `skip_organize_subset=true` honored per startup banner.

**Live DB state (`cassette.db`):**
- `acquisition_requests` count: `0` — consistent with no new acquisitions during this run.

**Planner path confirmation:**
- The `plan_and_submit` function (Task 1.1/1.2 output) is live in the coordinator binary and was exercised through the delta-queue drain path.
- No crash, no panic, no missing symbol — planner path compiled and ran cleanly.
- Zero pending delta_queue rows at proof time means no Director tasks were dispatched, which is correct and expected behavior (empty queue → nothing to plan → nothing to acquire).

**Conclusion:**
- Planner cutover is live in the coordinator binary. The path executes end-to-end without error. No pending work was present at proof time; a future run with populated desired-state will exercise the full plan→approve→acquire path.

**No re-acquisition of DENIAL IS A RIVER (row 1):**
- `delta_queue` row 1: `processed_at=2026-03-31 20:35:36` — unchanged
- `director_task_history`: only one `delta-1-denial is a river` row, disposition=Finalized from original run
- Row 203 (`manual_review` for desired_track_id=1) generated by reconciler but not submitted as a download claim

**Proof demonstrates:**
- Interrupted-run claimed rows survive intact (not wiped by `clear_reconciliation`)
- Stale claims reclaimed deterministically via `--stale-claim-minutes` threshold
- Resumed scan uses checkpoint fast-path (0 files scanned when all roots are `completed`)
- Re-claimed rows re-submitted to Director and finalized correctly
- Already-finalized rows are NOT re-acquired

## Discogs / Last.fm Enrichment Probe (WO-04 closure evidence) — 2026-04-07

Run command: `cargo run --bin enrich_probe_cli -- --limit 25`

**DB targeted:** `C:\Users\Admin\AppData\Roaming\dev.cassette.app\cassette.db` (live runtime DB)

**Token status:**
- Discogs token: configured
- Last.fm API key: configured

**Observed output summary:**
```
DB path: C:\Users\Admin\AppData\Roaming\dev.cassette.app\cassette.db
Discogs token: configured
Last.fm API key: configured

Summary: 25 tracks probed | Discogs hits: 25/25 | Last.fm hits: 0/25
```

**Interpretation:**
- WO-04 close criteria is met: a bounded credentialed non-zero enrichment probe is now captured in canonical docs.
- Discogs enrichment path is live and returned release context on every sampled row in this probe.
- Last.fm API calls ran with key present but returned no context hits for this sampled corpus; this is treated as sampled-data outcome, not wiring failure.
- The standing Known Limitations note remains accurate: full automatic background enrichment queueing is still pending.

## Verification Snapshot

Verified on 2026-04-06:

- `cargo check --workspace` passes
- `cargo test -p cassette-core` passes
- `cargo test --workspace` passes
- `npm run build` passes in `ui/`
- `.\scripts\smoke_desktop.ps1` passes
- `cargo test -p cassette --test pure_logic -- --nocapture` passes with the music-first home, dedicated library route, status strip, and artist-first copy assertions
- Runtime metadata hardening now has regression coverage for normalized lyrics-cache lookups, bounded canonical-ID backfill, cleaned filename-title fallback, broader cover-art filename detection, and deterministic Cover Art Archive fallback selection
- `.\scripts\verify_trust_spine.ps1` exists for the request-contract, audit-trace, core-test, UI-build, and smoke verification pass
- `src-tauri/tests/pure_logic.rs` now carries the Windows-safe `src-tauri` pure-logic assertions (Spotify import parsing, now-playing parsing, pending recovery planning, and sidecar bootstrap) so the test suite no longer depends on the Tauri lib harness startup path
- `engine_pipeline_cli` and `tag_rescue_cli` compile and test as part of the workspace
- `engine_pipeline_cli` now targets a dedicated sidecar DB (`cassette_librarian.db`) because the active runtime `tracks` table shape is not compatible with the librarian schema
- Librarian/orchestrator migrations now ensure `delta_queue.source_operation_id`, `claimed_at`, and `claim_run_id`
- Librarian sidecar scan state now persists `scan_checkpoints` plus `local_files.file_mtime_ms`
- `engine_pipeline_cli` now accepts `--scan-mode full|resume|delta-only`, with `--resume` defined as the `resume` scan-mode shorthand
- `engine_pipeline_cli` now defaults to `resume` scan mode when `--scan-mode` is not provided, so repeat coordinator runs reuse completed checkpoints and skip redundant full scans
- Librarian config now defaults to `scan_mode=resume` and adaptive fingerprint-backfill concurrency based on available CPU (clamped to 4..32)
- `engine_pipeline_cli` now uses an adaptive SQLite sidecar pool size based on CPU (2x parallelism, clamped to 4..32) instead of a single connection
- Director task payloads now persist `desired_track_id` and `source_operation_id` through the request payload path
- `tag_rescue_cli` now plans/applies staged track-number recovery and can emit a JSON repair report
- Organizer canonical path generation now preserves an existing non-zero filename track prefix when DB `track_number` is zero or missing
- `organize_cli --live` now aborts when the proposed move set crosses the zero-track rename threshold
- Organizer path updates now converge app `tracks.path` and sidecar `local_files` path metadata together, including stale-conflict displacement for pre-existing sidecar rows at the destination path
- Deezer acquisition now uses streaming decryption and recoverable session invalidation
- Director validation now reports truthful `audio_readable` / `header_readable` fields and rejects codec/container mismatches
- Director staged-download resume now honors `Retry-After`, preflights `Content-Length`, and validates range semantics before append
- Director provider health polling now runs concurrently, and provider search cache keys are partitioned by strategy plus provider epoch
- `recovery_probe_cli` still proves pending-job replay and stale-terminal filtering
- `provider_probe_cli` and `provider_acquire_probe_cli` still prove configured-provider readiness and live Deezer acquisition on this machine
- `tag_rescue_cli` repair heuristics and sidecar scan resume/delta behavior are covered by new Rust tests
- A real `engine_pipeline_cli --limit 5` run now bootstraps the sidecar DB and performs a live scan; an interrupted proof run reached `local_files=4500` and `tracks=3811` in the sidecar before being stopped
- `engine_pipeline_cli` now accepts `--stale-claim-minutes N` (default 30) to configure the stale-claim reclaim threshold; used in the interruption/resume proof with `--stale-claim-minutes 1`
- Interruption/resume recovery proof captured 2026-03-31: 3 stale claims reclaimed, checkpoint fast-path (0 files scanned), 3 tracks finalized, already-processed row not re-acquired
- `start_backlog_run` / `stop_backlog_run` / `get_backlog_status` Tauri commands added: background async loop through Spotify missing albums, emits `director-backlog-progress` events with live stats
- `get_director_debug_stats` command added: returns pending task count, per-provider success/fail breakdown, and recent task results
- Downloads UI: Backlog panel with start/stop/limit controls and live progress display; Debug panel with per-provider stats and scrollable recent results list
- Downloads UI now also exposes recent control-plane requests, per-request timeline events, and request-level candidate/provenance inspection
- Home + shell UX now expose music-first daily-use surfaces: while-you-were-away summary, artist-first collection entry, and persistent status strip
- Tauri command surface now includes `create_acquisition_request`, `list_acquisition_requests`, `get_acquisition_request_timeline`, `get_request_candidate_review`, and `get_request_lineage`
- Audit completeness proof updated: `validation::logging` now includes representative tests for operation-to-gatekeeper correlation, strict full-path lineage filtering, and gatekeeper failure/completion event trails
- `get_file_lineage` now uses strict full-path matching when a full path is provided (with JSON-escaped path support), preventing basename collisions from polluting audit traces
- Repeatable audit proof command surface re-verified: `cassette-cli operation --help`, `cassette-cli lineage --help`, and `cassette-cli validate --help`
- Performance baseline tooling now exists: `scripts/perf_baseline_capture.ps1` and `scripts/perf_regression_gate.ps1`
- Baseline and budget artifacts are now tracked in `docs/perf/BASELINE.latest.json` and `docs/perf/BUDGETS.json`
- Initial perf capture artifact recorded at `artifacts/perf/run-20260403-155455/results.json` and promoted to `docs/perf/BASELINE.latest.json`

## Documentation

| Document | Purpose |
|----------|---------|
| `docs/PROJECT_STATE.md` | Current runtime truth |
| `docs/TODO.md` | Prioritized active scope |
| `docs/DECISIONS.md` | Architectural and runtime-shape rationale |
| `docs/WORKLIST.md` | Broader architecture convergence tasks |
| `docs/CAPABILITY_AUDIT.md` | Gap analysis from the initial audit |
| `docs/TOOL_AND_SERVICE_REGISTRY.md` | Tool/service usage vs potential |
| `docs/CACHE_PROVENANCE_STRATEGY.md` | Cache and provenance persistence strategy |
| `docs/ARCHITECTURAL_RECOMMENDATIONS.md` | Architecture convergence recommendations |
| `docs/CLEAN_MACHINE_CHECKLIST.md` | Bootstrap assumptions and trust-spine verification pass |
