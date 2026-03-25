# Cassette — Session Log & Quick Reference

**Last updated:** 2026-03-20
**Agent continuity file:** Use this when context resets. Contains everything needed to resume work.

---

## Project Identity

- **App name:** Cassette
- **AI assistant name:** Cass (full: Cassondra) — **deferred**, not current priority
- **Stack:** Tauri 2 + SvelteKit + Rust + SQLite
- **Repo:** `c:/Cassette Music/`
- **Docs:** `c:/chatgpt/docs/`
- **Music library:** `A:/music/` (~2291 album folders)
- **Spotify history DB:** `c:/chatgpt/cassette_spotify.db`
- **App DB:** `%APPDATA%/Roaming/dev.cassette.app/cassette.db`
- **App identifier:** `dev.cassette.app` v0.1.0

---

## Current State (What's Done)

### Downloads — COMPLETE
- **2575 albums** downloaded from Spotify library (2569 original + extras)
- **122 remixes** downloaded (POP.EXE 50 + 72 top-artist remixes)
- Sources used: Qobuz (primary), Deezer (fallback), Soulseek, YouTube (remixes)
- **5 stubborn albums** grabbed via yt-dlp: Right Away Great Captain, Dead Poet Society, Fenech-Soler, GY!BE, Denzel Curry - Nostalgic 64
- All music consolidated to `A:/music/`
- Old scattered locations cleaned up: `C:/Cassette`, `C:/Users/Admin/StreamripDownloads`, `A:/StreamripDownloads`, `A:/Cassette`

### Config Paths — All Set to A:/music
- `c:/chatgpt/cassette_downloader.py` → `LIBRARY = Path('A:/Music')`, `SLSKD_DL`, `QOBUZ_DL` all `A:/Music`
- `c:/Cassette Music/.env` → `LIBRARY_BASE=A:\Music`, `STAGING_FOLDER=A:\Staging`
- `%APPDATA%/streamrip/config.toml` → `folder = "A:/Music"`
- `%LOCALAPPDATA%/slskd/slskd.yml` → `downloads: A:\Music`

### App Codebase — What's Already Built
- **Player:** Symphonia (decode) + CPAL (output) + RB (ring buffer). Play/pause/stop/seek/volume work. Media keys registered (Play/Pause/Next/Prev via tauri-plugin-global-shortcut).
- **Library:** Lofty tag reader, WalkDir scanner, SQLite with WAL. Scans FLAC/MP3/M4A/OGG/OPUS/WAV/AIFF/WV/APE. Emits scan-progress events.
- **UI:** SvelteKit routes: `/` (library), `/artists`, `/downloads`, `/playlists`, `/settings`. Three-column layout with NowPlaying bar.
- **Downloads:** Rust waterfall: Slskd → Usenet → Real-Debrid → Torrent. 3-concurrent semaphore. Progress events.
- **Queue:** Full CRUD, auto-play on queue_tracks.
- **Playlists:** Create/delete/reorder/play.
- **No console windows:** `windows_subsystem = "windows"` in main.rs. **EXCEPT** archive extraction calls PowerShell — needs fixing.

### Key Files
| File | Purpose |
|---|---|
| `c:/Cassette Music/src-tauri/src/lib.rs` | App setup, global shortcuts, state init |
| `c:/Cassette Music/src-tauri/src/state.rs` | AppState (player, db, download jobs) |
| `c:/Cassette Music/src-tauri/src/commands/downloads.rs` | Download commands + archive extraction (PowerShell bug here) |
| `c:/Cassette Music/src-tauri/src/commands/library.rs` | Scan, search, get tracks/albums/artists |
| `c:/Cassette Music/src-tauri/src/commands/player.rs` | Playback commands |
| `c:/Cassette Music/crates/cassette-core/src/player/mod.rs` | Player thread, decode loop, ring buffer |
| `c:/Cassette Music/crates/cassette-core/src/library/mod.rs` | Scanner, tag reading, cover art detection |
| `c:/Cassette Music/crates/cassette-core/src/db/mod.rs` | SQLite schema, migrations |
| `c:/Cassette Music/ui/src/routes/+layout.svelte` | Main layout, event listeners |
| `c:/Cassette Music/ui/src/lib/stores/player.ts` | Player state store |
| `c:/chatgpt/cassette_downloader.py` | Python batch downloader (not part of app — standalone tool) |
| `c:/chatgpt/qobuz_dl.py` | Standalone Qobuz+Deezer subprocess downloader |
| `c:/chatgpt/remix_downloader.py` | yt-dlp batch remix downloader |
| `c:/chatgpt/cassette_run_loop.py` | Multi-instance download loop runner |

### Credentials (configured, not all tested live in Rust app)
- **Soulseek:** snappedpoem / pierre2409 (in slskd.yml)
- **Deezer ARL:** in streamrip config.toml
- **Usenet:** EasyUsenet + NZBgeek (in .env)
- **Real-Debrid:** key in .env
- **Qobuz:** email/password in .env (login was failing)
- **slskd:** `c:/Cassette Music/binaries/slskd/slskd.exe` on port 5030 — must be started manually

---

## TODO List — Ordered by Priority

### 🔧 Immediate Fix
- [ ] **Fix PowerShell console flash in archive extraction** — `downloads.rs:130-166` calls `Command::new("powershell")` for ZIP extraction. Replace with Rust `zip` crate to eliminate console window flash. Critical for "no black screens" requirement.

### 🎵 PLAYER Features (Tier 1)
- [ ] **Gapless/crossfade playback** — Current player creates a new buffer per track. Need pre-buffering of next track and crossfade mixing in the playback callback. Key file: `crates/cassette-core/src/player/mod.rs`
- [ ] **ReplayGain / volume normalization** — Library has mixed sources (Qobuz FLAC, Deezer, YouTube MP3) with wildly different volumes. Read ReplayGain tags (Lofty supports this), apply gain in playback callback.
- [ ] **Windows media overlay integration** — Media keys already work via global-shortcut. Need SMTC (System Media Transport Controls) integration so Windows shows track info in the volume popup. Crate: `windows` or `souvlaki`.
- [ ] **System tray mini-player** — Tauri supports system tray natively. Add tray icon with play/pause/next/prev context menu + tooltip showing current track.
- [ ] **Audio EQ / DSP** — Apply biquad filters in the playback callback. 10-band EQ with presets (flat, bass boost, vocal, etc.). Store presets in settings table.
- [ ] **Visualizer** — Send FFT data from playback callback to frontend via Tauri events. Render waveform/spectrum in canvas/WebGL in NowPlaying bar.

### 📚 LIBRARY Features (Tier 2)
- [ ] **Folder watcher** — Use `notify` crate to watch `A:/music` for new files. Auto-scan on change. Emit `library-updated` event.
- [ ] **Tag editor** — Lofty already reads tags. Add write support: `lofty::Tag::save_to_path()`. Expose as Tauri command. UI: inline edit in track detail view.
- [ ] **Album art fetcher** — Query MusicBrainz Cover Art Archive, Discogs, Fanart.tv APIs. Download and save as `cover.jpg` in album folder. Lofty can embed art in tags too.
- [ ] **Library statistics / Wrapped** — Query play_count, skip_count, total_ms from Spotify DB + app DB. Show top artists, genres, decades, total listening time. Generate a "Wrapped" style summary page.
- [ ] **Connect enricher to library files** — The app's library scanner needs `A:/music` as a library root. Then enrichment commands (MusicBrainz ingestion, audio feature extraction) can run against real files.

### 📊 DATA Pipeline (Tier 3)
- [ ] **Integrate Spotify play counts** — Merge `cassette_spotify.db` tracks table (play_count, skip_count, first_played, last_played) into the app's `cassette.db` tracks table. Match by artist+title+album.
- [ ] **Import Last.fm scrobble history** — Use Last.fm API (`user.getRecentTracks`) to pull full scrobble history. Store in a `scrobbles` table. Feed into play counts and taste memory.
- [ ] **Scrape genres/subgenres** — For each artist: query MusicBrainz tags, Last.fm tags, Discogs styles. Store in an `artist_genres` table. Use for smart playlists, browse-by-genre, and taste profiling.
- [ ] **Run MusicBrainz artist ingestion** — Commands already exist: `ingest_artist_relationships`, `pending_artist_ingestion_count`. Need to run across full library.
- [ ] **Run audio feature extraction** — Commands exist: `extract_audio_features_batch`. Need to run across full library. Feeds into mood/energy matching.
- [ ] **MBID identity spine** — Canonical artist/track IDs from MusicBrainz. Partially implemented. Needed for cross-provider dedup and accurate enrichment.

### 🎶 PLAYLISTS (Tier 4)
- [ ] **Smart playlists** — Auto-generated by rules: genre=post-hardcore AND decade=2000s AND BPM>120. Stored as saved queries, re-evaluated on library changes.
- [ ] **Import/export** — M3U, XSPF write from playlist. Spotify playlist import via API (track matching by ISRC or artist+title).
- [ ] **Drag-reorder queue** — Frontend: Svelte drag-and-drop on QueuePanel. Backend: `reorder_queue(track_id, new_position)` command.

### 🔌 INTEGRATIONS (Tier 5)
- [ ] **Last.fm scrobbling (live)** — On track play >50% or >4min, scrobble via Last.fm API. Need user API key + session auth.
- [ ] **Discord Rich Presence** — Use `discord-rich-presence` crate. Show current track, album art, elapsed time. Toggle in settings.
- [ ] **Lyrics display** — Query LRCLIB API for synced lyrics (`.lrc` format), Genius fallback for static. Display in RightSidebar "Lyrics" tab. Highlight current line during playback.
- [ ] **Chromecast / DLNA** — Lower priority. Would need `mdns` discovery + DLNA/Chromecast protocol. Consider `rust-dlna` or `gcast` crates.

### 🚀 RELEASE (Tier 6)
- [ ] **GitHub Actions CI/CD** — Workflow: build Tauri app on push, run tests, create draft release with .msi/.exe artifacts.
- [ ] **Auto-updater** — Tauri has built-in updater plugin. Point at GitHub Releases JSON endpoint.
- [ ] **Windows installer + clean-machine proof** — NSIS or WiX via Tauri bundler. Test on fresh Windows install.

---

## Architecture Notes for New Agents

### Player thread model
The player runs on its own OS thread with a command channel (`mpsc::Receiver<PlayerCommand>`). It decodes packets from Symphonia into a lock-free ring buffer (RB crate, ~2s capacity). A separate CPAL audio callback reads from the ring buffer and writes to the sound card. Position is tracked via atomic f64. This means:
- **Don't block the player thread** — it must keep decoding to prevent buffer underrun
- **Gapless needs pre-buffering** — open and start decoding the next track before the current one ends
- **EQ/effects go in the CPAL callback** — that's where samples become audio output

### State model
`AppState` holds `Arc<Mutex<>>` wrapped fields. Player is `Arc<Player>` (internally uses atomics + mpsc). DB is `Arc<Mutex<Db>>`. Download jobs are `Arc<Mutex<HashMap>>`. For high-frequency operations (position polling at 100ms), the atomic approach in Player is correct. For DB access, WAL mode handles concurrent reads.

### Frontend-backend bridge
All communication is via Tauri `invoke` (request/response) and `emit`/`listen` (events). The frontend polls playback state every 100ms for the seek bar. Download progress and scan progress are pushed via events.

### Self-contained requirement
**Critical:** The user requires NO console windows, NO PowerShell pop-ups, NO black screens. Everything must work inside the Tauri window like a normal desktop app. The only current violation is `downloads.rs` archive extraction calling PowerShell for ZIP files. Fix: use the `zip` Rust crate instead.

### Hardware
- Ryzen 7 (8 cores / 16 threads), 32GB RAM, 16GB VRAM
- Always maximize parallelism — batch sizes of 8-16 workers, concurrent I/O, async pipelines
- C: drive ~350GB free, A: drive 8.3TB free (music lives here)

---

## Spotify DB Schema (cassette_spotify.db)
```sql
tracks(spotify_uri PK, track, artist, album, play_count, full_play_count, total_ms, skip_count, first_played, last_played, downloaded, download_status, download_path)
album_queue(id, artist, album, play_count, track_count, status, attempts, error, ...)
```

## App DB Schema (cassette.db)
```sql
library_roots(id, path, enabled)
tracks(id, path, title, artist, album, album_artist, track_number, disc_number, year, duration_secs, sample_rate, bit_depth, bitrate_kbps, format, file_size, cover_art_path, added_at)
queue_items(id, track_id, position)
settings(key PK, value)
playlists(id, name, description, created_at)
playlist_items(id, playlist_id, track_id, position)
```

---

## Decisions Made
- All "Lyra" references are legacy → product is **Cassette**
- AI features (Cass/Cassondra) are deferred — player-first approach
- Python downloaders are standalone tools, NOT part of the app runtime
- Rust owns all canonical app state — no Python in the app
- Music lives on A:/music — all configs updated
- slskd binary bundled at `c:/Cassette Music/binaries/slskd/` — packaging strategy TBD
- yt-dlp for YouTube/SoundCloud remixes — not integrated into app, standalone script only

## Reference Docs
- `c:/chatgpt/docs/WHOLE_DREAM_CHECKLIST.md` — Master feature checklist with checked/unchecked items
- `c:/chatgpt/docs/MISSING_FEATURES_REGISTRY.md` — Active gap matrix (G-060 through G-067)
- `c:/chatgpt/docs/WORKLIST.md` — Execution priority order
- `c:/chatgpt/docs/BACKLOG_TAGS.md` — Acknowledged-but-dormant concepts
- `c:/chatgpt/docs/WORKFLOW_NEEDS.md` — Expected workflows from legacy parity
- `c:/chatgpt/docs/PROJECT_STATE.md` — Backend reality audit
- `A:/music/requested remixes/remix_list.md` — Full remix list with sources noted
- `C:/Users/Admin/.claude/projects/c--Cassette-Music/memory/` — Agent memory files
