# Cassette Project State

Last updated: 2026-03-30

## Architecture

- **Desktop shell**: Tauri 2.10.3 (`src-tauri/`)
- **Frontend**: SvelteKit (`ui/`)
- **Core domain**: Rust workspace (`crates/cassette-core/`)
- **Database**: SQLite via rusqlite (runtime) + sqlx (librarian/orchestrator flows)
- **Active DB location**: Tauri app-data directory (`cassette.db`)
- **Integrated queue sidecar DB**: Tauri app-data directory (`cassette_librarian.db`) for librarian/orchestrator tables such as `desired_tracks`, `local_files`, `reconciliation_results`, `delta_queue`, `sync_runs`, and `scan_checkpoints`
- **Repo-local DB files**: root-level `*.db` files in this workspace are local artifacts for tests, probes, or inspection; they are not automatically the live runtime DB

## What Works Today

### Library Management
- Library root scanning with recursive audio file discovery
- Track metadata extraction (artist, album, title, track/disc number, year, duration, sample rate, bit depth, bitrate, format, file size)
- Cover art path detection
- Search across tracks by title/artist/album
- Play count and skip count tracking
- Library organization (move files to `Artist/Album/NN-Title` structure)
- `tag_rescue_cli` now applies a staged repair ladder for missing/zero track numbers: embedded tags, filename prefixes, then conservative album-pattern inference
- `tag_rescue_cli` can emit a machine-readable repair report with repaired rows, unresolved rows, and repair-source classification
- Organizer canonical path generation now preserves a valid existing non-zero filename track prefix when DB `track_number` is zero or missing
- Offline cleanup tooling includes grouped manifest builders, safe apply scripts, and rollback-oriented remediation helpers under `scripts/`

### Playback
- Audio playback via Symphonia decode + `cpal` output
- Queue management (add, remove, reorder, clear)
- Playlist CRUD
- Now-playing context from Last.fm
- Synced/plain lyrics from LRCLIB

### Acquisition Pipeline (Director Engine)
- Two-pass waterfall orchestration with per-provider semaphores
- 7 acquisition strategies (`Standard`, `HighQualityOnly`, `ObscureFallbackHeavy`, `SingleTrackPriority`, `DiscographyBatch`, `RedownloadReplaceIfBetter`, `MetadataRepairOnly`)
- 6-factor candidate scoring (metadata confidence, duration match, codec quality, provider trust, validation result, file size)
- Task-local cancellation via `CancellationToken` registry, with batch-wide cancel reserved for shutdown
- Symphonia-based audio validation with truthful `audio_readable` / `header_readable` reporting and codec/container mismatch rejection
- Post-acquisition metadata tagging via Lofty
- Atomic finalization with duplicate policy (`KeepExisting` or `ReplaceIfBetter`)
- Per-task temp directories with stale recovery and quarantine
- Provider-aware retry and staged-download resume hardening, including `Retry-After`, `Content-Length` preflight, and range validation
- Search-result caching in the director waterfall with strategy-aware cache keys and provider-epoch invalidation
- Concurrent provider health polling with skip-on-down behavior
- Runtime provider stabilization cools down rate-limited and temporary-outage providers, disables auth-failed providers for the rest of the run, and treats provider-busy as capacity pressure instead of provider-down
- Broadcast event channels for progress, results, and provider health
- Deezer full-track acquisition is live-proven on this machine
- Deezer session caching now uses a recoverable `RwLock<Option<...>>` path instead of permanently caching auth failure
- Deezer acquisition now decrypts directly to disk through a streaming Blowfish stripe path

### Providers (7 active)

| Provider | Trust Rank | Capabilities | Status |
|----------|------------|-------------|--------|
| Local Archive | 0 | Filesystem walk + direct copy, batch support | Proven working |
| Deezer | 5 | Search + acquire with Blowfish CBC stream decryption | Implemented |
| Qobuz | 10 | MD5-signed session, search + acquire (lossless) | Implemented |
| slskd/Soulseek | 10 | P2P search with queue recovery, transfer polling + filesystem fallback | Implemented |
| Usenet | 30 | NZBgeek search + SABnzbd execution, filesystem polling | Implemented |
| yt-dlp | 50 | Subprocess fallback, `ytsearch1` + `scsearch1` | Proven working |
| Real-Debrid | 80 | TPB search + torrent resolution + 7z extraction | Implemented |

### Metadata Services

| Service | Usage | Auth |
|---------|-------|------|
| MusicBrainz | Release search, parent album lookup, track listing, tag fixes | None |
| Last.fm | Artist/album context for now-playing | Public API key |
| LRCLIB | Synced/plain lyrics lookup | None |
| Spotify | History import, search, discography seeds | Optional OAuth |

### Data Pipeline
- Spotify play history import from external SQLite DB
- Missing album detection (Spotify albums not in local library)
- Album and Spotify-missing queues expand releases into per-track `TrackTask` submissions via MusicBrainz release tracklists
- Director task result persistence to `director_task_history`
- Terminal history retains the original `TrackTask` request payload and strategy for failed/cancelled/finalized results
- Pending director task persistence in `director_pending_tasks` for deterministic startup recovery
- Request-signature persistence threads through pending tasks, terminal history, candidate sets, and provider memory
- Full candidate-set persistence captures scored, rejected, and selected candidates in `director_candidate_sets` and `director_candidate_items`
- Provider search outcomes and negative-result memory persist in `director_provider_searches`, `director_provider_attempts`, and `director_provider_memory`
- `TrackTask` payloads now carry `desired_track_id` and `source_operation_id` for control-plane closure

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

Separate richer schemas also exist in the `library`, `librarian`, and `gatekeeper` subsystems. Those remain part of the architecture convergence story, not a replacement for the active Tauri runtime database.

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

## Pipeline Integration Architecture

The integrated acquisition loop is now centered on `delta_queue` as the authoritative durable work bus, hosted in the librarian sidecar DB so it does not collide with the playback/runtime schema in `cassette.db`.

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
- A live tag-rescue pass on 2026-03-30 updated `0` rows, so the remaining organizer issue is not a simple “DB blank, embedded tags present” case

### Legacy / Manual Path

`batch_download_cli` still exists for direct/manual use, but it is no longer the canonical integrated control-plane path.

## Known Limitations

- Frontend still keeps `get_download_jobs` as a catch-up and resume fallback even though push events are now primary
- Dual schema: richer librarian/library model exists but isn't fully wired into the active runtime UI path
- `MetadataRepairOnly` strategy is still a stub
- Discogs/Last.fm enrichers outside now-playing remain stubbed/no-op
- Bandcamp source remains placeholder-only
- Candidate persistence exists, but the app still does not reuse that memory for pre-acquisition review, exclusion decisions, or explicit user override lanes
- `batch_download_cli` still uses the older album-history/manual workflow and has not been removed yet
- `director/providers/` is the active acquisition path; `downloader/` is now only a legacy compatibility re-export for provider settings types
- Organizer repair tooling is deeper now, but the live app-DB repair proof and bounded live organize proof are still pending
- Album batching currently groups queue work into `DiscographyBatch` strategy selection in the coordinator, but provider locking remains strategy-led rather than a separately persisted album lane
- Structured run observability is improved through queue claims and persisted request payloads, but the frontend does not yet expose a dedicated coordinator timeline view

## Verification Snapshot

Verified on 2026-03-30:

- `cargo check --workspace` passes
- `cargo test --workspace` passes
- `npm run build` passes in `ui/` (with existing Svelte accessibility warnings on `src/routes/downloads/+page.svelte`)
- `.\scripts\smoke_desktop.ps1` passes
- `engine_pipeline_cli` and `tag_rescue_cli` compile and test as part of the workspace
- `engine_pipeline_cli` now targets a dedicated sidecar DB (`cassette_librarian.db`) because the active runtime `tracks` table shape is not compatible with the librarian schema
- Librarian/orchestrator migrations now ensure `delta_queue.source_operation_id`, `claimed_at`, and `claim_run_id`
- Librarian sidecar scan state now persists `scan_checkpoints` plus `local_files.file_mtime_ms`
- `engine_pipeline_cli` now accepts `--scan-mode full|resume|delta-only`, with `--resume` defined as the `resume` scan-mode shorthand
- Director task payloads now persist `desired_track_id` and `source_operation_id` through the request payload path
- `tag_rescue_cli` now plans/applies staged track-number recovery and can emit a JSON repair report
- Organizer canonical path generation now preserves an existing non-zero filename track prefix when DB `track_number` is zero or missing
- `organize_cli --live` now aborts when the proposed move set crosses the zero-track rename threshold
- Deezer acquisition now uses streaming decryption and recoverable session invalidation
- Director validation now reports truthful `audio_readable` / `header_readable` fields and rejects codec/container mismatches
- Director staged-download resume now honors `Retry-After`, preflights `Content-Length`, and validates range semantics before append
- Director provider health polling now runs concurrently, and provider search cache keys are partitioned by strategy plus provider epoch
- `recovery_probe_cli` still proves pending-job replay and stale-terminal filtering
- `provider_probe_cli` and `provider_acquire_probe_cli` still prove configured-provider readiness and live Deezer acquisition on this machine
- `tag_rescue_cli` repair heuristics and sidecar scan resume/delta behavior are covered by new Rust tests
- A real `engine_pipeline_cli --limit 5` run now bootstraps the sidecar DB and performs a live scan; an interrupted proof run reached `local_files=4500` and `tracks=3811` in the sidecar before being stopped

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
