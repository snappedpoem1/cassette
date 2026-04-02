# Cassette Service Inventory

Generated: 2026-04-02

This inventory was built from workspace manifests, config surfaces, provider modules, source adapters, enrichment stubs, shell commands, and live database inspection.

## Active In Code

| Surface | Role | Workspace Signal |
|---|---|---|
| MusicBrainz | Canonical metadata and tracklists | `crates/cassette-core/src/metadata.rs` |
| Spotify Web API | Search, album expansion, import seeds | `crates/cassette-core/src/sources.rs`, `validation/spotify_import.rs` |
| Deezer | Search plus acquire | `director/providers/deezer.rs`, `sources.rs` |
| Qobuz | Search plus acquire | `director/providers/qobuz.rs`, `sources.rs` |
| slskd / Soulseek | Search, queue, transfer polling | `director/providers/slskd.rs`, `sources.rs` |
| NZBGeek | Usenet indexer search | `director/providers/usenet.rs`, settings/config |
| SABnzbd | Usenet execution queue | `director/providers/usenet.rs`, settings/config |
| Real-Debrid | Torrent resolver / unrestrict | `director/providers/real_debrid.rs`, `torrent_album_cli.rs` |
| apibay / TPB | Torrent search upstream | `director/providers/real_debrid.rs`, `torrent_album_cli.rs` |
| Jackett | Optional torrent search broker | `torrent_album_cli.rs`, settings/config |
| yt-dlp | Fallback extraction | `director/providers/ytdlp.rs` |
| YouTube | Desired-source adapter and yt-dlp upstream | `director/sources/youtube.rs` |
| SoundCloud | yt-dlp search lane | `director/providers/ytdlp.rs` search tokens |
| Local archive | Filesystem source | `director/providers/local_archive.rs` |
| Last.fm | Now-playing context | `src-tauri/src/commands/player.rs` |
| LRCLIB | Synced/plain lyrics | `src-tauri/src/commands/player.rs` |
| Discogs | Enrichment stub and schema fields | `librarian/enrich/discogs.rs`, sidecar schema |
| Bandcamp | Placeholder source adapter | `director/sources/bandcamp.rs` |
| AcoustID / fingerprinting | Collision and identity fields referenced | gatekeeper/orchestrator schema and queries |
| Genius | Configured token surface only | `provider_settings.rs`, settings commands |
| iTunes Search API | Metadata fallback | `crates/cassette-core/src/metadata.rs` |
| Tidal | Doc-only idea / not wired | older project docs only |
| Cover Art Archive | Referenced architectural fit, not wired | project docs only |
| ListenBrainz | Not wired, but strategically relevant | not present in runtime code |

## Local Libraries And Plugins

| Surface | Role | Workspace Signal |
|---|---|---|
| Symphonia | Decode, probe, validation | `cassette-core/Cargo.toml`, `player/`, validation modules |
| Lofty | Tag read/write | `cassette-core/Cargo.toml`, `metadata.rs`, gatekeeper |
| CPAL | Playback output | `player/mod.rs` |
| Tauri plugin log | Desktop logging | `src-tauri/Cargo.toml` |
| Tauri plugin dialog | File/folder selection | `src-tauri/Cargo.toml`, UI |
| Tauri plugin fs | Desktop FS bridge | `src-tauri/Cargo.toml` |
| Tauri plugin shell | Shell launching | `src-tauri/Cargo.toml`, UI |
| Tauri plugin global-shortcut | App hotkeys | `src-tauri/Cargo.toml` |

## Tying Agents And Reference Systems

| Surface | Why It Matters |
|---|---|
| Beets | Mature pluginized import / MBID / fingerprint bridge |
| MusicBrainz Picard | Strong file-to-MBID recovery and tag-writing model |
| Lidarr | Wanted-list to downloader to import pipeline reference |

## Immediate Architectural Read

- Cassette already has multiple acquisition backends, but not a durable cross-provider identity spine.
- The live `tracks` table carries columns for MBIDs and canonical IDs, but they are empty in practice.
- The sidecar `local_files` table contains better quality and hash truth than the active runtime `tracks` table.
- The codebase is one refactor away from a true sovereign graph, but right now it still re-discovers too much every run.
