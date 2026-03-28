# Changelog

All notable changes to the Cassette project.

## [0.1.0] - 2026-03-27

### Foundation
- Tauri 2 desktop shell with SvelteKit frontend and Rust workspace backend
- SQLite database with 9 tables (tracks, albums, queue, playlists, settings, spotify history, director task history)
- Library scanning, search, playback, queue, and playlist management

### Director Acquisition Engine
- Two-pass waterfall orchestration with configurable worker concurrency (default 12)
- 7 acquisition strategies: Standard, HighQualityOnly, ObscureFallbackHeavy, SingleTrackPriority, DiscographyBatch, RedownloadReplaceIfBetter, MetadataRepairOnly
- 6-factor candidate scoring: metadata confidence, duration match, codec quality, provider trust rank, validation result, file size
- Per-provider semaphore-based concurrency control with configurable limits
- Retry with linear backoff (configurable attempts and base delay)
- Symphonia-based audio validation with magic byte detection and duration extraction
- Post-acquisition Lofty tag writing with cover art download and embedding
- Atomic finalization with dedup policy (KeepExisting or ReplaceIfBetter)
- Per-task temp directory management with stale recovery and quarantine
- Broadcast event channels for progress tracking

### Providers
- **Local Archive** (trust 0): Filesystem walk with normalized substring matching, batch support
- **Deezer** (trust 5): Search via api.deezer.com, acquire with Blowfish CBC stream decryption (FLAC/320/128 quality cascade)
- **Qobuz** (trust 10): MD5-signed session auth, search + acquire with RwLock session caching
- **slskd/Soulseek** (trust 10): P2P search with global semaphore serialization, queue recovery (>500 or stale >10min), multi-query search with weighted ranking, dual transfer detection (API + filesystem), connection health/reconnect
- **Usenet** (trust 30): NZBgeek search (cat 3000) + SABnzbd multipart POST execution, filesystem completion polling
- **yt-dlp** (trust 50): Subprocess fallback with ytsearch1 + scsearch1, extract-audio mode
- **Real-Debrid** (trust 80): TPB search via apibay.org, seeding qualifiers, instant availability batch check, torrent resolution with file selection, link unrestriction, 7z extraction

### Metadata Services
- **MusicBrainz**: Release search, recording-to-parent-album lookup, track listing fetch, tag fix proposals with lofty application. 1 req/sec rate limiting.
- **Last.fm**: Artist and album context for now-playing display (artist.getinfo, album.getinfo)
- **LRCLIB**: Synced and plain lyrics lookup by artist + track name
- **Spotify**: Play history import from external SQLite, search, discography seeding

### Audio Processing
- **Symphonia**: Format probing, codec detection, duration extraction, audio playback decode
- **Lofty**: Tag read/write for all major formats, cover art embedding (PNG/JPEG detection, 15MB max)

### Data Pipeline
- Spotify history import with missing album detection
- Album-level batch download submission
- Director task result persistence with full provenance records
- Library organization (atomic moves to Artist/Album/NN-Title.ext structure)

### Code Quality
- Fixed 6 operational correctness issues found in audit
- Real-Debrid provider implementation added
- Batch download CLI tool
- Library organize CLI tool

### Documentation
- 19 code-traced per-component reference docs in `docs/reference/`
- Capability audit, tool registry, request matrix, integration gaps analysis
- Architecture recommendations and cache/provenance strategy docs
- Accurate PROJECT_STATE.md reflecting actual codebase state
