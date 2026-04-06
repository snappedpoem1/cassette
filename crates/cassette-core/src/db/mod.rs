use crate::acquisition::AcquisitionRequest as PlannerAcquisitionRequest;
use crate::director::models::{
    CandidateRecord, DirectorTaskResult, ProviderSearchRecord, TrackTask,
};
use crate::identity::{normalize_artist_identity, normalize_identity_text};
use crate::models::{
    Album, Artist, LibraryRoot, Playlist, PlaylistItem, QueueItem, SpotifyAlbumHistory, Track,
};
use crate::Result;
use chrono::{Duration, Utc};
use rusqlite::{params, Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Reads an image file from disk, downsamples it to 8×8, and returns the
/// average colour as a CSS hex string like `"#3d2810"`.
/// Returns `None` if the file cannot be read or decoded.
pub fn extract_dominant_color(path: &str) -> Option<String> {
    let img = image::open(path).ok()?;
    let small = img.resize_exact(8, 8, image::imageops::FilterType::Lanczos3);
    let rgb = small.to_rgb8();
    let pixels = rgb.pixels();
    let count = 64u64;
    let (r, g, b) = pixels.fold((0u64, 0u64, 0u64), |(ar, ag, ab), p| {
        (ar + p[0] as u64, ag + p[1] as u64, ab + p[2] as u64)
    });
    Some(format!(
        "#{:02x}{:02x}{:02x}",
        (r / count) as u8,
        (g / count) as u8,
        (b / count) as u8
    ))
}

pub struct Db {
    conn: Connection,
}

#[derive(Debug, Clone)]
pub struct TrackPathUpdate {
    pub track_id: i64,
    pub old_path: String,
    pub new_path: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrackIdentityContext {
    pub musicbrainz_release_group_id: Option<String>,
    pub canonical_release_type: Option<String>,
    pub edition_bucket: Option<String>,
    pub edition_markers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CachedLyrics {
    pub lyrics: Option<String>,
    pub synced_lyrics: Option<String>,
    pub source: String,
    pub fetched_at: String,
}

impl Db {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    pub fn open_read_only(path: &Path) -> Result<Self> {
        let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        conn.execute_batch("PRAGMA query_only=ON; PRAGMA foreign_keys=ON;")?;
        Ok(Self { conn })
    }

    fn migrate(&self) -> Result<()> {
        self.conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS library_roots (
                id      INTEGER PRIMARY KEY AUTOINCREMENT,
                path    TEXT NOT NULL UNIQUE,
                enabled INTEGER NOT NULL DEFAULT 1
            );
            CREATE TABLE IF NOT EXISTS tracks (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                path            TEXT NOT NULL UNIQUE,
                title           TEXT NOT NULL DEFAULT '',
                artist          TEXT NOT NULL DEFAULT '',
                album           TEXT NOT NULL DEFAULT '',
                album_artist    TEXT NOT NULL DEFAULT '',
                track_number    INTEGER,
                disc_number     INTEGER,
                year            INTEGER,
                duration_secs   REAL NOT NULL DEFAULT 0,
                sample_rate     INTEGER,
                bit_depth       INTEGER,
                bitrate_kbps    INTEGER,
                format          TEXT NOT NULL DEFAULT '',
                file_size       INTEGER NOT NULL DEFAULT 0,
                cover_art_path  TEXT,
                play_count      INTEGER NOT NULL DEFAULT 0,
                skip_count      INTEGER NOT NULL DEFAULT 0,
                last_played     TEXT,
                added_at        TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE INDEX IF NOT EXISTS idx_tracks_artist       ON tracks(artist);
            CREATE INDEX IF NOT EXISTS idx_tracks_album        ON tracks(album);
            CREATE INDEX IF NOT EXISTS idx_tracks_title        ON tracks(title);
            CREATE INDEX IF NOT EXISTS idx_tracks_album_artist ON tracks(album_artist);
            CREATE TABLE IF NOT EXISTS play_history_events (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                track_id    INTEGER REFERENCES tracks(id) ON DELETE SET NULL,
                source      TEXT NOT NULL,
                artist      TEXT NOT NULL,
                title       TEXT NOT NULL,
                album       TEXT,
                played_at   TEXT NOT NULL,
                created_at  TEXT NOT NULL DEFAULT (datetime('now')),
                UNIQUE (source, artist, title, album, played_at)
            );
            CREATE INDEX IF NOT EXISTS idx_play_history_events_played_at
                ON play_history_events(played_at DESC);
            CREATE INDEX IF NOT EXISTS idx_play_history_events_artist_title
                ON play_history_events(artist COLLATE NOCASE, title COLLATE NOCASE);
            CREATE TABLE IF NOT EXISTS artist_play_history (
                artist      TEXT PRIMARY KEY,
                play_count  INTEGER NOT NULL DEFAULT 0,
                last_played TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_artist_play_history_play_count
                ON artist_play_history(play_count DESC);
            CREATE TABLE IF NOT EXISTS song_play_history (
                artist      TEXT NOT NULL,
                title       TEXT NOT NULL,
                album       TEXT NOT NULL DEFAULT '',
                play_count  INTEGER NOT NULL DEFAULT 0,
                last_played TEXT,
                PRIMARY KEY (artist, title, album)
            );
            CREATE INDEX IF NOT EXISTS idx_song_play_history_play_count
                ON song_play_history(play_count DESC);
            CREATE TABLE IF NOT EXISTS queue_items (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                track_id    INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
                position    INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS settings (
                key     TEXT PRIMARY KEY,
                value   TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS playlists (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                name        TEXT NOT NULL UNIQUE,
                description TEXT,
                created_at  TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE TABLE IF NOT EXISTS playlist_items (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
                track_id    INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
                position    INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_playlist_items ON playlist_items(playlist_id, position);
            CREATE TABLE IF NOT EXISTS spotify_album_history (
                artist      TEXT NOT NULL,
                album       TEXT NOT NULL,
                total_ms    INTEGER NOT NULL DEFAULT 0,
                play_count  INTEGER NOT NULL DEFAULT 0,
                skip_count  INTEGER NOT NULL DEFAULT 0,
                in_library  INTEGER NOT NULL DEFAULT 0,
                imported_at TEXT NOT NULL DEFAULT (datetime('now')),
                PRIMARY KEY (artist, album)
            );
            CREATE INDEX IF NOT EXISTS idx_spotify_album_history_artist
                ON spotify_album_history(artist COLLATE NOCASE);
            CREATE INDEX IF NOT EXISTS idx_spotify_album_history_play_count
                ON spotify_album_history(play_count DESC);
            CREATE TABLE IF NOT EXISTS director_task_history (
                task_id              TEXT PRIMARY KEY,
                disposition          TEXT NOT NULL,
                provider             TEXT,
                failure_class        TEXT,
                final_path           TEXT,
                request_signature    TEXT,
                score_total          INTEGER,
                error                TEXT,
                source_metadata_json TEXT,
                validation_json      TEXT,
                score_reason_json    TEXT,
                attempts_json        TEXT NOT NULL,
                result_json          TEXT NOT NULL,
                updated_at           TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE INDEX IF NOT EXISTS idx_director_task_history_updated_at
                ON director_task_history(updated_at DESC);
            CREATE TABLE IF NOT EXISTS director_pending_tasks (
                task_id     TEXT PRIMARY KEY,
                track_json  TEXT NOT NULL,
                request_signature TEXT,
                strategy    TEXT NOT NULL,
                progress    TEXT NOT NULL,
                created_at  TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE INDEX IF NOT EXISTS idx_director_pending_tasks_updated_at
                ON director_pending_tasks(updated_at DESC);
            CREATE TABLE IF NOT EXISTS director_candidate_sets (
                task_id                         TEXT PRIMARY KEY,
                request_signature              TEXT,
                request_strategy               TEXT,
                disposition                    TEXT NOT NULL,
                selected_provider              TEXT,
                selected_provider_candidate_id TEXT,
                selected_score_total           INTEGER,
                candidate_count                INTEGER NOT NULL DEFAULT 0,
                provider_count                 INTEGER NOT NULL DEFAULT 0,
                result_json                    TEXT NOT NULL,
                created_at                     TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at                     TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE INDEX IF NOT EXISTS idx_director_candidate_sets_request_signature
                ON director_candidate_sets(request_signature, updated_at DESC);
            CREATE TABLE IF NOT EXISTS director_candidate_items (
                id                  INTEGER PRIMARY KEY AUTOINCREMENT,
                task_id             TEXT NOT NULL,
                request_signature   TEXT,
                provider_id         TEXT NOT NULL,
                provider_display_name TEXT NOT NULL,
                provider_trust_rank INTEGER NOT NULL,
                provider_order_index INTEGER NOT NULL,
                search_rank         INTEGER NOT NULL,
                provider_candidate_id TEXT NOT NULL,
                outcome             TEXT NOT NULL,
                rejection_reason    TEXT,
                is_selected         INTEGER NOT NULL DEFAULT 0,
                acquisition_temp_path TEXT,
                score_total         INTEGER,
                candidate_json      TEXT NOT NULL,
                validation_json     TEXT,
                score_json          TEXT,
                score_reason_json   TEXT,
                recorded_at         TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE INDEX IF NOT EXISTS idx_director_candidate_items_task
                ON director_candidate_items(task_id, provider_order_index, search_rank);
            CREATE INDEX IF NOT EXISTS idx_director_candidate_items_request_signature
                ON director_candidate_items(request_signature, provider_id, recorded_at DESC);
            CREATE TABLE IF NOT EXISTS director_provider_searches (
                id                  INTEGER PRIMARY KEY AUTOINCREMENT,
                task_id             TEXT NOT NULL,
                request_signature   TEXT,
                provider_id         TEXT NOT NULL,
                provider_display_name TEXT NOT NULL,
                provider_trust_rank INTEGER NOT NULL,
                provider_order_index INTEGER NOT NULL,
                outcome             TEXT NOT NULL,
                candidate_count     INTEGER NOT NULL DEFAULT 0,
                error               TEXT,
                retryable           INTEGER NOT NULL DEFAULT 0,
                recorded_at         TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE INDEX IF NOT EXISTS idx_director_provider_searches_task
                ON director_provider_searches(task_id, provider_order_index);
            CREATE INDEX IF NOT EXISTS idx_director_provider_searches_request_signature
                ON director_provider_searches(request_signature, provider_id, recorded_at DESC);
            CREATE TABLE IF NOT EXISTS director_provider_attempts (
                id                INTEGER PRIMARY KEY AUTOINCREMENT,
                task_id           TEXT NOT NULL,
                request_signature TEXT,
                provider_id       TEXT NOT NULL,
                attempt_index     INTEGER NOT NULL,
                attempt_number    INTEGER NOT NULL,
                outcome           TEXT NOT NULL,
                recorded_at       TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE INDEX IF NOT EXISTS idx_director_provider_attempts_task
                ON director_provider_attempts(task_id, attempt_index);
            CREATE INDEX IF NOT EXISTS idx_director_provider_attempts_request_signature
                ON director_provider_attempts(request_signature, provider_id, recorded_at DESC);
            CREATE TABLE IF NOT EXISTS director_provider_memory (
                request_signature TEXT NOT NULL,
                provider_id       TEXT NOT NULL,
                last_task_id      TEXT NOT NULL,
                last_outcome      TEXT NOT NULL,
                failure_class     TEXT NOT NULL,
                error             TEXT,
                retryable         INTEGER NOT NULL DEFAULT 0,
                candidate_count   INTEGER NOT NULL DEFAULT 0,
                backoff_until     TEXT,
                updated_at        TEXT NOT NULL DEFAULT (datetime('now')),
                PRIMARY KEY (request_signature, provider_id)
            );
            CREATE INDEX IF NOT EXISTS idx_director_provider_memory_updated_at
                ON director_provider_memory(updated_at DESC);
            CREATE TABLE IF NOT EXISTS track_lyrics (
                id                INTEGER PRIMARY KEY AUTOINCREMENT,
                track_id          INTEGER REFERENCES tracks(id) ON DELETE CASCADE,
                artist            TEXT NOT NULL,
                title             TEXT NOT NULL,
                album             TEXT,
                normalized_artist TEXT NOT NULL,
                normalized_title  TEXT NOT NULL,
                normalized_album  TEXT NOT NULL DEFAULT '',
                lyrics            TEXT,
                synced_lyrics     TEXT,
                source            TEXT NOT NULL,
                fetched_at        TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at        TEXT NOT NULL DEFAULT (datetime('now')),
                UNIQUE (normalized_artist, normalized_title, normalized_album)
            );
            CREATE INDEX IF NOT EXISTS idx_track_lyrics_track_id
                ON track_lyrics(track_id);
        ",
        )?;
        self.ensure_column_exists("director_task_history", "request_json", "TEXT")?;
        self.ensure_column_exists("director_task_history", "request_strategy", "TEXT")?;
        self.ensure_column_exists("director_task_history", "request_signature", "TEXT")?;
        self.ensure_column_exists("director_task_history", "failure_class", "TEXT")?;
        self.ensure_column_exists("director_pending_tasks", "request_signature", "TEXT")?;

        // ── Schema convergence: canonical identity tables ────────────────────
        self.conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS canonical_artists (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                name            TEXT NOT NULL,
                normalized_name TEXT NOT NULL,
                musicbrainz_id  TEXT UNIQUE,
                spotify_id      TEXT UNIQUE,
                discogs_id      TEXT UNIQUE,
                sort_name       TEXT,
                created_at      TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE INDEX IF NOT EXISTS idx_canonical_artists_normalized
                ON canonical_artists(normalized_name);

            CREATE TABLE IF NOT EXISTS canonical_releases (
                id                          INTEGER PRIMARY KEY AUTOINCREMENT,
                canonical_artist_id         INTEGER NOT NULL REFERENCES canonical_artists(id),
                title                       TEXT NOT NULL,
                normalized_title            TEXT NOT NULL,
                release_group_mbid          TEXT,
                release_mbid                TEXT UNIQUE,
                spotify_id                  TEXT UNIQUE,
                discogs_id                  TEXT UNIQUE,
                release_type                TEXT,
                year                        INTEGER,
                release_date                TEXT,
                track_count                 INTEGER,
                cover_art_url               TEXT,
                created_at                  TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at                  TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE INDEX IF NOT EXISTS idx_canonical_releases_artist
                ON canonical_releases(canonical_artist_id);
            CREATE INDEX IF NOT EXISTS idx_canonical_releases_normalized
                ON canonical_releases(normalized_title);
            CREATE INDEX IF NOT EXISTS idx_canonical_releases_release_group
                ON canonical_releases(release_group_mbid);

            CREATE TABLE IF NOT EXISTS acquisition_requests (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                scope           TEXT NOT NULL,
                artist          TEXT NOT NULL,
                album           TEXT,
                strategy        TEXT NOT NULL,
                quality_floor   TEXT,
                exclude_providers TEXT,
                edition_policy  TEXT,
                canonical_artist_id  INTEGER REFERENCES canonical_artists(id),
                canonical_release_id INTEGER REFERENCES canonical_releases(id),
                status          TEXT NOT NULL DEFAULT 'pending',
                created_at      TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE INDEX IF NOT EXISTS idx_acquisition_requests_status
                ON acquisition_requests(status, created_at DESC);

            CREATE TABLE IF NOT EXISTS canonical_recordings (
                id                          INTEGER PRIMARY KEY AUTOINCREMENT,
                canonical_artist_id         INTEGER REFERENCES canonical_artists(id),
                canonical_release_id        INTEGER REFERENCES canonical_releases(id),
                title                       TEXT NOT NULL,
                normalized_title            TEXT NOT NULL,
                musicbrainz_recording_id    TEXT UNIQUE,
                isrc                        TEXT,
                track_number                INTEGER,
                disc_number                 INTEGER,
                duration_secs               REAL,
                created_at                  TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at                  TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE INDEX IF NOT EXISTS idx_canonical_recordings_artist
                ON canonical_recordings(canonical_artist_id);
            CREATE INDEX IF NOT EXISTS idx_canonical_recordings_release
                ON canonical_recordings(canonical_release_id);
            CREATE INDEX IF NOT EXISTS idx_canonical_recordings_normalized
                ON canonical_recordings(normalized_title);
            CREATE INDEX IF NOT EXISTS idx_canonical_recordings_isrc
                ON canonical_recordings(isrc);

            CREATE TABLE IF NOT EXISTS source_aliases (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                entity_type     TEXT NOT NULL,
                entity_key      TEXT NOT NULL,
                source_name     TEXT NOT NULL,
                source_key      TEXT NOT NULL,
                source_value    TEXT NOT NULL,
                confidence      REAL,
                raw_json        TEXT,
                created_at      TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at      TEXT NOT NULL DEFAULT (datetime('now')),
                UNIQUE (entity_type, entity_key, source_name, source_key, source_value)
            );
            CREATE INDEX IF NOT EXISTS idx_source_aliases_entity
                ON source_aliases(entity_type, entity_key);
            CREATE INDEX IF NOT EXISTS idx_source_aliases_lookup
                ON source_aliases(source_name, source_key, source_value);

            CREATE TABLE IF NOT EXISTS provider_search_evidence (
                id                  INTEGER PRIMARY KEY AUTOINCREMENT,
                task_id             TEXT NOT NULL,
                request_signature   TEXT,
                provider_id         TEXT NOT NULL,
                outcome             TEXT NOT NULL,
                candidate_count     INTEGER NOT NULL DEFAULT 0,
                retryable           INTEGER NOT NULL DEFAULT 0,
                error               TEXT,
                raw_json            TEXT NOT NULL,
                retention_class     TEXT NOT NULL DEFAULT 'provenance',
                recorded_at         TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE INDEX IF NOT EXISTS idx_provider_search_evidence_task
                ON provider_search_evidence(task_id, recorded_at DESC);
            CREATE INDEX IF NOT EXISTS idx_provider_search_evidence_request
                ON provider_search_evidence(request_signature, provider_id, recorded_at DESC);

            CREATE TABLE IF NOT EXISTS provider_candidate_evidence (
                id                      INTEGER PRIMARY KEY AUTOINCREMENT,
                task_id                 TEXT NOT NULL,
                request_signature       TEXT,
                provider_id             TEXT NOT NULL,
                provider_candidate_id   TEXT NOT NULL,
                outcome                 TEXT NOT NULL,
                rejection_reason        TEXT,
                is_selected             INTEGER NOT NULL DEFAULT 0,
                metadata_confidence     REAL,
                raw_json                TEXT NOT NULL,
                retention_class         TEXT NOT NULL DEFAULT 'provenance',
                recorded_at             TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE INDEX IF NOT EXISTS idx_provider_candidate_evidence_task
                ON provider_candidate_evidence(task_id, provider_id, recorded_at DESC);
            CREATE INDEX IF NOT EXISTS idx_provider_candidate_evidence_request
                ON provider_candidate_evidence(request_signature, provider_id, recorded_at DESC);

            CREATE TABLE IF NOT EXISTS provider_response_cache (
                request_signature   TEXT NOT NULL,
                provider_id         TEXT NOT NULL,
                last_task_id        TEXT NOT NULL,
                outcome             TEXT NOT NULL,
                candidate_count     INTEGER NOT NULL DEFAULT 0,
                response_json       TEXT NOT NULL,
                failure_class       TEXT,
                backoff_until       TEXT,
                retention_class     TEXT NOT NULL DEFAULT 'ephemeral_cache',
                updated_at          TEXT NOT NULL DEFAULT (datetime('now')),
                PRIMARY KEY (request_signature, provider_id)
            );
            CREATE INDEX IF NOT EXISTS idx_provider_response_cache_updated
                ON provider_response_cache(updated_at DESC);

            CREATE TABLE IF NOT EXISTS identity_resolution_evidence (
                id                          INTEGER PRIMARY KEY AUTOINCREMENT,
                task_id                     TEXT,
                request_signature           TEXT,
                entity_type                 TEXT NOT NULL,
                entity_key                  TEXT NOT NULL,
                source_name                 TEXT NOT NULL,
                evidence_type               TEXT NOT NULL,
                canonical_artist_id         INTEGER,
                canonical_release_id        INTEGER,
                musicbrainz_recording_id    TEXT,
                musicbrainz_release_id      TEXT,
                isrc                        TEXT,
                confidence                  REAL,
                raw_json                    TEXT,
                retention_class             TEXT NOT NULL DEFAULT 'identity_fact',
                recorded_at                 TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE INDEX IF NOT EXISTS idx_identity_resolution_evidence_entity
                ON identity_resolution_evidence(entity_type, entity_key, recorded_at DESC);
            CREATE INDEX IF NOT EXISTS idx_identity_resolution_evidence_request
                ON identity_resolution_evidence(request_signature, recorded_at DESC);
        ",
        )?;

        // Add identity columns to tracks table (non-breaking migration)
        self.ensure_column_exists("tracks", "isrc", "TEXT")?;
        self.ensure_column_exists("tracks", "musicbrainz_recording_id", "TEXT")?;
        self.ensure_column_exists("tracks", "musicbrainz_release_id", "TEXT")?;
        self.ensure_column_exists("tracks", "canonical_artist_id", "INTEGER")?;
        self.ensure_column_exists("tracks", "canonical_release_id", "INTEGER")?;
        self.ensure_column_exists("track_lyrics", "track_id", "INTEGER")?;
        self.ensure_column_exists("tracks", "quality_tier", "TEXT")?;
        self.ensure_column_exists("tracks", "content_hash", "TEXT")?;
        self.ensure_column_exists("tracks", "dominant_color_hex", "TEXT")?;

        // Create indexes for new track columns (IF NOT EXISTS is safe to repeat)
        let _ = self.conn.execute_batch(
            "
            CREATE INDEX IF NOT EXISTS idx_tracks_isrc ON tracks(isrc);
            CREATE INDEX IF NOT EXISTS idx_tracks_mb_recording ON tracks(musicbrainz_recording_id);
            CREATE INDEX IF NOT EXISTS idx_tracks_canonical_artist ON tracks(canonical_artist_id);
            CREATE INDEX IF NOT EXISTS idx_tracks_canonical_release ON tracks(canonical_release_id);
            CREATE INDEX IF NOT EXISTS idx_tracks_content_hash ON tracks(content_hash);
        ",
        );

        Ok(())
    }

    fn ensure_column_exists(&self, table: &str, column: &str, column_type: &str) -> Result<()> {
        let mut stmt = self.conn.prepare(&format!("PRAGMA table_info({table})"))?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
        let existing = rows.collect::<rusqlite::Result<Vec<_>>>()?;
        if existing.iter().any(|name| name == column) {
            return Ok(());
        }

        self.conn.execute(
            &format!("ALTER TABLE {table} ADD COLUMN {column} {column_type}"),
            [],
        )?;
        Ok(())
    }

    // ── Library Roots ─────────────────────────────────────────────────────────

    pub fn add_library_root(&self, path: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO library_roots (path) VALUES (?1)",
            params![path],
        )?;
        Ok(())
    }

    pub fn remove_library_root(&self, path: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM library_roots WHERE path = ?1", params![path])?;
        Ok(())
    }

    pub fn get_library_roots(&self) -> Result<Vec<LibraryRoot>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, path, enabled FROM library_roots ORDER BY id")?;
        let rows = stmt.query_map([], |row| {
            Ok(LibraryRoot {
                id: row.get(0)?,
                path: row.get(1)?,
                enabled: row.get::<_, i64>(2)? != 0,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    // ── Tracks ────────────────────────────────────────────────────────────────

    pub fn upsert_track(&self, t: &Track) -> Result<()> {
        let resolved = self.resolve_track_canonical_ids(t)?;
        self.conn.execute("
            INSERT INTO tracks
                (path,title,artist,album,album_artist,track_number,disc_number,year,
                 duration_secs,sample_rate,bit_depth,bitrate_kbps,format,file_size,cover_art_path,
                 isrc,musicbrainz_recording_id,musicbrainz_release_id,canonical_artist_id,
                 canonical_release_id,quality_tier,content_hash)
            VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22)
            ON CONFLICT(path) DO UPDATE SET
                title=excluded.title, artist=excluded.artist, album=excluded.album,
                album_artist=excluded.album_artist, track_number=excluded.track_number,
                disc_number=excluded.disc_number, year=excluded.year,
                duration_secs=excluded.duration_secs, sample_rate=excluded.sample_rate,
                bit_depth=excluded.bit_depth, bitrate_kbps=excluded.bitrate_kbps,
                format=excluded.format, file_size=excluded.file_size,
                cover_art_path=excluded.cover_art_path,
                isrc=COALESCE(excluded.isrc, tracks.isrc),
                musicbrainz_recording_id=COALESCE(excluded.musicbrainz_recording_id, tracks.musicbrainz_recording_id),
                musicbrainz_release_id=COALESCE(excluded.musicbrainz_release_id, tracks.musicbrainz_release_id),
                canonical_artist_id=COALESCE(excluded.canonical_artist_id, tracks.canonical_artist_id),
                canonical_release_id=COALESCE(excluded.canonical_release_id, tracks.canonical_release_id),
                quality_tier=COALESCE(excluded.quality_tier, tracks.quality_tier),
                content_hash=COALESCE(excluded.content_hash, tracks.content_hash)
        ", params![
            resolved.path, resolved.title, resolved.artist, resolved.album, resolved.album_artist,
            resolved.track_number, resolved.disc_number, resolved.year, resolved.duration_secs,
            resolved.sample_rate, resolved.bit_depth, resolved.bitrate_kbps, resolved.format,
            resolved.file_size as i64, resolved.cover_art_path, resolved.isrc,
            resolved.musicbrainz_recording_id, resolved.musicbrainz_release_id, resolved.canonical_artist_id,
            resolved.canonical_release_id, resolved.quality_tier, resolved.content_hash,
        ])?;
        self.upsert_track_identity_evidence(&resolved)?;
        Ok(())
    }

    pub fn upsert_tracks_batch(&self, tracks: &[Track]) -> Result<()> {
        if tracks.is_empty() {
            return Ok(());
        }

        self.conn.execute_batch("BEGIN IMMEDIATE TRANSACTION;")?;
        let write_result: Result<()> = (|| {
            let mut stmt = self.conn.prepare(
                "
                INSERT INTO tracks
                    (path,title,artist,album,album_artist,track_number,disc_number,year,
                     duration_secs,sample_rate,bit_depth,bitrate_kbps,format,file_size,cover_art_path,
                     isrc,musicbrainz_recording_id,musicbrainz_release_id,canonical_artist_id,
                     canonical_release_id,quality_tier,content_hash)
                VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22)
                ON CONFLICT(path) DO UPDATE SET
                    title=excluded.title, artist=excluded.artist, album=excluded.album,
                    album_artist=excluded.album_artist, track_number=excluded.track_number,
                    disc_number=excluded.disc_number, year=excluded.year,
                    duration_secs=excluded.duration_secs, sample_rate=excluded.sample_rate,
                    bit_depth=excluded.bit_depth, bitrate_kbps=excluded.bitrate_kbps,
                    format=excluded.format, file_size=excluded.file_size,
                    cover_art_path=excluded.cover_art_path,
                    isrc=COALESCE(excluded.isrc, tracks.isrc),
                    musicbrainz_recording_id=COALESCE(excluded.musicbrainz_recording_id, tracks.musicbrainz_recording_id),
                    musicbrainz_release_id=COALESCE(excluded.musicbrainz_release_id, tracks.musicbrainz_release_id),
                    canonical_artist_id=COALESCE(excluded.canonical_artist_id, tracks.canonical_artist_id),
                    canonical_release_id=COALESCE(excluded.canonical_release_id, tracks.canonical_release_id),
                    quality_tier=COALESCE(excluded.quality_tier, tracks.quality_tier),
                    content_hash=COALESCE(excluded.content_hash, tracks.content_hash)
                "
            )?;

            for t in tracks {
                let resolved = self.resolve_track_canonical_ids(t)?;
                stmt.execute(params![
                    resolved.path,
                    resolved.title,
                    resolved.artist,
                    resolved.album,
                    resolved.album_artist,
                    resolved.track_number,
                    resolved.disc_number,
                    resolved.year,
                    resolved.duration_secs,
                    resolved.sample_rate,
                    resolved.bit_depth,
                    resolved.bitrate_kbps,
                    resolved.format,
                    resolved.file_size as i64,
                    resolved.cover_art_path,
                    resolved.isrc,
                    resolved.musicbrainz_recording_id,
                    resolved.musicbrainz_release_id,
                    resolved.canonical_artist_id,
                    resolved.canonical_release_id,
                    resolved.quality_tier,
                    resolved.content_hash,
                ])?;
            }
            Ok(())
        })();

        if let Err(error) = write_result {
            let _ = self.conn.execute_batch("ROLLBACK;");
            return Err(error);
        }

        self.conn.execute_batch("COMMIT;")?;
        for track in tracks {
            let resolved = self.resolve_track_canonical_ids(track)?;
            self.upsert_track_identity_evidence(&resolved)?;
        }
        Ok(())
    }

    fn resolve_track_canonical_ids(&self, track: &Track) -> Result<Track> {
        let mut resolved = track.clone();
        if resolved.canonical_artist_id.is_none() {
            let artist_name = if !resolved.album_artist.trim().is_empty() {
                resolved.album_artist.as_str()
            } else {
                resolved.artist.as_str()
            };
            if !artist_name.trim().is_empty() {
                if let Some(artist) = self.get_canonical_artist_by_name(artist_name)? {
                    resolved.canonical_artist_id = Some(artist.id);
                }
            }
        }

        if resolved.canonical_release_id.is_none()
            && !resolved.album.trim().is_empty()
            && resolved.canonical_artist_id.is_some()
        {
            resolved.canonical_release_id = self.find_canonical_release_id(
                resolved.canonical_artist_id.expect("checked is_some"),
                &resolved.album,
            )?;
        }

        Ok(resolved)
    }

    pub fn backfill_missing_track_canonical_ids(&self, limit: usize) -> Result<usize> {
        let bounded_limit = i64::try_from(limit.max(1)).unwrap_or(250);
        let mut stmt = self.conn.prepare(
            "SELECT id
             FROM tracks
             WHERE canonical_artist_id IS NULL
                OR (canonical_release_id IS NULL AND trim(ifnull(album, '')) <> '')
             ORDER BY datetime(added_at) ASC, id ASC
             LIMIT ?1",
        )?;
        let ids = stmt
            .query_map(params![bounded_limit], |row| row.get::<_, i64>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let mut updated = 0usize;
        for track_id in ids {
            let Some(track) = self.get_track_by_id(track_id)? else {
                continue;
            };
            let resolved = self.resolve_track_canonical_ids(&track)?;
            if resolved.canonical_artist_id == track.canonical_artist_id
                && resolved.canonical_release_id == track.canonical_release_id
            {
                continue;
            }

            self.conn.execute(
                "UPDATE tracks
                 SET canonical_artist_id = ?1,
                     canonical_release_id = ?2
                 WHERE id = ?3",
                params![
                    resolved.canonical_artist_id,
                    resolved.canonical_release_id,
                    track_id
                ],
            )?;
            self.upsert_track_identity_evidence(&resolved)?;
            updated += 1;
        }

        Ok(updated)
    }

    fn find_canonical_release_id(
        &self,
        canonical_artist_id: i64,
        album: &str,
    ) -> Result<Option<i64>> {
        let normalized_album = normalize_identity_text(album);
        if normalized_album.is_empty() {
            return Ok(None);
        }
        match self.conn.query_row(
            "SELECT id FROM canonical_releases
             WHERE canonical_artist_id = ?1 AND normalized_title = ?2
             LIMIT 1",
            params![canonical_artist_id, normalized_album],
            |row| row.get::<_, i64>(0),
        ) {
            Ok(id) => Ok(Some(id)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    fn upsert_track_identity_evidence(&self, track: &Track) -> Result<()> {
        if track.musicbrainz_recording_id.is_none()
            && track.isrc.is_none()
            && track.canonical_artist_id.is_none()
            && track.canonical_release_id.is_none()
        {
            return Ok(());
        }

        let canonical_recording_id = self.upsert_canonical_recording(
            &track.title,
            track.canonical_artist_id,
            track.canonical_release_id,
            track.musicbrainz_recording_id.as_deref(),
            track.isrc.as_deref(),
            track.track_number,
            track.disc_number,
            Some(track.duration_secs),
        )?;

        let entity_key = format!("canonical_recording:{canonical_recording_id}");
        if let Some(isrc) = track.isrc.as_deref() {
            self.upsert_source_alias(
                "canonical_recording",
                &entity_key,
                "isrc",
                "track",
                isrc,
                Some(1.0),
                None,
            )?;
        }
        if let Some(mbid) = track.musicbrainz_recording_id.as_deref() {
            self.upsert_source_alias(
                "canonical_recording",
                &entity_key,
                "musicbrainz",
                "recording_id",
                mbid,
                Some(1.0),
                None,
            )?;
        }
        Ok(())
    }

    pub fn get_track_size_index(&self) -> Result<HashMap<String, u64>> {
        let mut stmt = self.conn.prepare("SELECT path, file_size FROM tracks")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as u64))
        })?;

        let mut index = HashMap::new();
        for row in rows {
            let (path, file_size) = row?;
            index.insert(path, file_size);
        }
        Ok(index)
    }

    pub fn get_track_count(&self) -> Result<i64> {
        Ok(self
            .conn
            .query_row("SELECT COUNT(*) FROM tracks", [], |r| r.get(0))?)
    }

    pub fn get_tracks(&self, limit: i64, offset: i64) -> Result<Vec<Track>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT id,path,title,artist,album,album_artist,track_number,disc_number,year,
                   duration_secs,sample_rate,bit_depth,bitrate_kbps,format,file_size,
                   cover_art_path,isrc,musicbrainz_recording_id,musicbrainz_release_id,
                   canonical_artist_id,canonical_release_id,quality_tier,content_hash,added_at
            FROM tracks
            ORDER BY album_artist COLLATE NOCASE, album COLLATE NOCASE, disc_number, track_number
            LIMIT ?1 OFFSET ?2
        ",
        )?;
        let rows = stmt.query_map(params![limit, offset], Self::row_to_track)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn search_tracks(&self, query: &str) -> Result<Vec<Track>> {
        let pattern = format!("%{query}%");
        let mut stmt = self.conn.prepare(
            "
            SELECT id,path,title,artist,album,album_artist,track_number,disc_number,year,
                   duration_secs,sample_rate,bit_depth,bitrate_kbps,format,file_size,
                   cover_art_path,isrc,musicbrainz_recording_id,musicbrainz_release_id,
                   canonical_artist_id,canonical_release_id,quality_tier,content_hash,added_at
            FROM tracks
            WHERE title LIKE ?1 OR artist LIKE ?1 OR album LIKE ?1
            ORDER BY artist COLLATE NOCASE, album COLLATE NOCASE, track_number
            LIMIT 200
        ",
        )?;
        let rows = stmt.query_map(params![pattern], Self::row_to_track)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn get_track_by_id(&self, id: i64) -> Result<Option<Track>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT id,path,title,artist,album,album_artist,track_number,disc_number,year,
                   duration_secs,sample_rate,bit_depth,bitrate_kbps,format,file_size,
                   cover_art_path,isrc,musicbrainz_recording_id,musicbrainz_release_id,
                   canonical_artist_id,canonical_release_id,quality_tier,content_hash,added_at
            FROM tracks WHERE id = ?1
        ",
        )?;
        let mut rows = stmt.query_map(params![id], Self::row_to_track)?;
        Ok(rows.next().transpose()?)
    }

    pub fn get_track_by_path(&self, path: &str) -> Result<Option<Track>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT id,path,title,artist,album,album_artist,track_number,disc_number,year,
                   duration_secs,sample_rate,bit_depth,bitrate_kbps,format,file_size,
                   cover_art_path,isrc,musicbrainz_recording_id,musicbrainz_release_id,
                   canonical_artist_id,canonical_release_id,quality_tier,content_hash,added_at
            FROM tracks WHERE path = ?1
        ",
        )?;
        let mut rows = stmt.query_map(params![path], Self::row_to_track)?;
        Ok(rows.next().transpose()?)
    }

    pub fn get_track_identity_context(&self, track_id: i64) -> Result<Option<TrackIdentityContext>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT
                COALESCE(
                    (SELECT release_group_mbid FROM canonical_releases WHERE id = t.canonical_release_id LIMIT 1),
                    (SELECT release_group_mbid FROM canonical_releases WHERE lower(release_mbid) = lower(t.musicbrainz_release_id) LIMIT 1)
                ) AS release_group_mbid,
                COALESCE(
                    (SELECT release_type FROM canonical_releases WHERE id = t.canonical_release_id LIMIT 1),
                    (SELECT release_type FROM canonical_releases WHERE lower(release_mbid) = lower(t.musicbrainz_release_id) LIMIT 1)
                ) AS release_type,
                t.album
            FROM tracks t
            WHERE t.id = ?1
            LIMIT 1
            ",
        )?;

        let row = stmt.query_row(params![track_id], |row| {
            let release_group_mbid: Option<String> = row.get(0)?;
            let release_type: Option<String> = row.get(1)?;
            let album: String = row.get(2)?;
            Ok((release_group_mbid, release_type, album))
        });

        match row {
            Ok((musicbrainz_release_group_id, canonical_release_type, album)) => {
                let markers = detect_edition_markers(&album);
                let edition_bucket = classify_edition_bucket(&album, canonical_release_type.as_deref());
                Ok(Some(TrackIdentityContext {
                    musicbrainz_release_group_id,
                    canonical_release_type,
                    edition_bucket,
                    edition_markers: markers,
                }))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    pub fn find_tracks_for_metadata_repair(
        &self,
        target: &crate::director::models::NormalizedTrack,
        limit: usize,
    ) -> Result<Vec<Track>> {
        let mut stmt = self.conn.prepare(
            "SELECT id,path,title,artist,album,album_artist,track_number,disc_number,year,
                    duration_secs,sample_rate,bit_depth,bitrate_kbps,format,file_size,
                    cover_art_path,isrc,musicbrainz_recording_id,musicbrainz_release_id,
                    canonical_artist_id,canonical_release_id,quality_tier,content_hash,added_at
             FROM tracks
             WHERE (
                    (?1 IS NOT NULL AND lower(ifnull(isrc, '')) = lower(?1))
                 OR (?2 IS NOT NULL AND lower(ifnull(musicbrainz_recording_id, '')) = lower(?2))
                 OR (
                        lower(ifnull(album_artist, artist)) = lower(?3)
                    AND lower(title) = lower(?4)
                    AND (?5 IS NULL OR lower(album) = lower(?5))
                 )
             )
             ORDER BY
                CASE
                    WHEN (?1 IS NOT NULL AND lower(ifnull(isrc, '')) = lower(?1)) THEN 0
                    WHEN (?2 IS NOT NULL AND lower(ifnull(musicbrainz_recording_id, '')) = lower(?2)) THEN 1
                    ELSE 2
                END,
                added_at DESC
             LIMIT ?6",
        )?;

        let rows = stmt.query_map(
            params![
                target.isrc.as_deref(),
                target.musicbrainz_recording_id.as_deref(),
                target.artist.as_str(),
                target.title.as_str(),
                target.album.as_deref(),
                i64::try_from(limit.max(1)).unwrap_or(50),
            ],
            Self::row_to_track,
        )?;

        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn get_albums(&self) -> Result<Vec<Album>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT album_artist, album, MIN(year), MIN(cover_art_path), COUNT(*)
            FROM tracks GROUP BY album_artist, album
            ORDER BY album_artist COLLATE NOCASE, album COLLATE NOCASE
        ",
        )?;
        let rows = stmt.query_map([], |row| {
            let artist: String = row.get(0)?;
            let title: String = row.get(1)?;
            let id = stable_entity_id(&[artist.as_str(), title.as_str()]);
            let cover_art_path: Option<String> = row.get(3)?;
            Ok(Album {
                id,
                title,
                artist,
                year: row.get(2)?,
                dominant_color_hex: cover_art_path.as_deref().and_then(extract_dominant_color),
                cover_art_path,
                track_count: row.get::<_, i64>(4)? as usize,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn get_album_tracks(&self, artist: &str, album: &str) -> Result<Vec<Track>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT id,path,title,artist,album,album_artist,track_number,disc_number,year,
                   duration_secs,sample_rate,bit_depth,bitrate_kbps,format,file_size,
                   cover_art_path,isrc,musicbrainz_recording_id,musicbrainz_release_id,
                   canonical_artist_id,canonical_release_id,quality_tier,content_hash,added_at
            FROM tracks WHERE album_artist = ?1 AND album = ?2
            ORDER BY disc_number, track_number
        ",
        )?;
        let rows = stmt.query_map(params![artist, album], Self::row_to_track)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn get_artists(&self) -> Result<Vec<Artist>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT album_artist, COUNT(DISTINCT album), COUNT(*)
            FROM tracks GROUP BY album_artist
            ORDER BY album_artist COLLATE NOCASE
        ",
        )?;
        let rows = stmt.query_map([], |row| {
            let name: String = row.get(0)?;
            let id = stable_entity_id(&[name.as_str()]);
            Ok(Artist {
                id,
                name,
                album_count: row.get::<_, i64>(1)? as usize,
                track_count: row.get::<_, i64>(2)? as usize,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn get_cached_track_lyrics(
        &self,
        artist: &str,
        title: &str,
        album: Option<&str>,
    ) -> Result<Option<CachedLyrics>> {
        let normalized_artist = normalize_artist_identity(artist);
        let normalized_title = normalize_identity_text(title);
        let normalized_album = album.map(normalize_identity_text).unwrap_or_default();

        if normalized_artist.is_empty() || normalized_title.is_empty() {
            return Ok(None);
        }

        match self.conn.query_row(
            "SELECT lyrics, synced_lyrics, source, fetched_at
             FROM track_lyrics
             WHERE normalized_artist = ?1
               AND normalized_title = ?2
               AND normalized_album = ?3
             LIMIT 1",
            params![normalized_artist, normalized_title, normalized_album],
            |row| {
                Ok(CachedLyrics {
                    lyrics: row.get(0)?,
                    synced_lyrics: row.get(1)?,
                    source: row.get(2)?,
                    fetched_at: row.get(3)?,
                })
            },
        ) {
            Ok(row) => Ok(Some(row)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    pub fn upsert_track_lyrics(
        &self,
        track_id: Option<i64>,
        artist: &str,
        title: &str,
        album: Option<&str>,
        lyrics: Option<&str>,
        synced_lyrics: Option<&str>,
        source: &str,
    ) -> Result<()> {
        let normalized_artist = normalize_artist_identity(artist);
        let normalized_title = normalize_identity_text(title);
        let normalized_album = album.map(normalize_identity_text).unwrap_or_default();

        if normalized_artist.is_empty()
            || normalized_title.is_empty()
            || (lyrics.is_none() && synced_lyrics.is_none())
        {
            return Ok(());
        }

        self.conn.execute(
            "INSERT INTO track_lyrics
                (track_id, artist, title, album, normalized_artist, normalized_title, normalized_album,
                 lyrics, synced_lyrics, source)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(normalized_artist, normalized_title, normalized_album) DO UPDATE SET
                track_id = COALESCE(excluded.track_id, track_lyrics.track_id),
                artist = excluded.artist,
                title = excluded.title,
                album = excluded.album,
                lyrics = COALESCE(excluded.lyrics, track_lyrics.lyrics),
                synced_lyrics = COALESCE(excluded.synced_lyrics, track_lyrics.synced_lyrics),
                source = excluded.source,
                fetched_at = datetime('now'),
                updated_at = datetime('now')",
            params![
                track_id,
                artist,
                title,
                album,
                normalized_artist,
                normalized_title,
                normalized_album,
                lyrics,
                synced_lyrics,
                source,
            ],
        )?;
        Ok(())
    }

    /// Update a track's path after file move
    pub fn update_track_path(&self, track_id: i64, new_path: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE tracks SET path = ?1 WHERE id = ?2",
            params![new_path, track_id],
        )?;
        Ok(())
    }

    pub fn apply_track_path_updates(
        &self,
        sidecar_db_path: &Path,
        updates: &[TrackPathUpdate],
    ) -> Result<()> {
        if updates.is_empty() {
            return Ok(());
        }

        self.conn.execute_batch("BEGIN IMMEDIATE TRANSACTION;")?;
        let app_write: Result<()> = (|| {
            let mut stmt = self
                .conn
                .prepare("UPDATE tracks SET path = ?1 WHERE id = ?2")?;
            for update in updates {
                stmt.execute(params![update.new_path, update.track_id])?;
            }
            Ok(())
        })();
        if let Err(error) = app_write {
            let _ = self.conn.execute_batch("ROLLBACK;");
            return Err(error);
        }
        self.conn.execute_batch("COMMIT;")?;

        Self::sync_sidecar_local_file_paths(sidecar_db_path, updates)
    }

    pub fn sync_sidecar_local_file_paths(
        sidecar_db_path: &Path,
        updates: &[TrackPathUpdate],
    ) -> Result<()> {
        if updates.is_empty() || !sidecar_db_path.exists() {
            return Ok(());
        }

        let conn = Connection::open(sidecar_db_path)?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON; BEGIN IMMEDIATE TRANSACTION;",
        )?;

        let sync_result: Result<()> = (|| {
            for update in updates {
                let new_path = PathBuf::from(&update.new_path);
                let file_name = new_path
                    .file_name()
                    .map(|value| value.to_string_lossy().to_string())
                    .unwrap_or_default();
                let extension = new_path
                    .extension()
                    .map(|value| value.to_string_lossy().to_string())
                    .unwrap_or_default();
                let file_mtime_ms = std::fs::metadata(&new_path)
                    .ok()
                    .and_then(|metadata| metadata.modified().ok())
                    .and_then(|modified| modified.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|duration| duration.as_millis() as i64);

                conn.execute(
                    "
                    UPDATE local_files
                    SET file_path = file_path || '.stale-conflict-' || id,
                        integrity_status = 'missing_on_disk',
                        updated_at = CURRENT_TIMESTAMP
                    WHERE LOWER(file_path) = LOWER(?1)
                      AND LOWER(file_path) != LOWER(?2)
                    ",
                    params![update.new_path, update.old_path],
                )?;

                conn.execute(
                    "
                    UPDATE local_files
                    SET file_path = ?1,
                        file_name = ?2,
                        extension = ?3,
                        file_mtime_ms = ?4,
                        integrity_status = CASE
                            WHEN integrity_status = 'missing_on_disk' THEN 'readable'
                            ELSE integrity_status
                        END,
                        last_scanned_at = CURRENT_TIMESTAMP,
                        updated_at = CURRENT_TIMESTAMP
                    WHERE LOWER(file_path) = LOWER(?5)
                    ",
                    params![
                        update.new_path,
                        file_name,
                        extension,
                        file_mtime_ms,
                        update.old_path,
                    ],
                )?;
            }
            Ok(())
        })();

        if let Err(error) = sync_result {
            let _ = conn.execute_batch("ROLLBACK;");
            return Err(error);
        }
        conn.execute_batch("COMMIT;")?;
        Ok(())
    }

    pub fn update_track_embedded_metadata(
        &self,
        track_id: i64,
        title: Option<&str>,
        artist: Option<&str>,
        album: Option<&str>,
        track_number: Option<i32>,
        disc_number: Option<i32>,
        year: Option<i32>,
    ) -> Result<()> {
        self.conn.execute(
            "
            UPDATE tracks
            SET
                title = COALESCE(?1, CASE WHEN TRIM(title) = '' THEN title ELSE title END),
                artist = COALESCE(?2, CASE WHEN TRIM(artist) = '' THEN artist ELSE artist END),
                album = COALESCE(?3, CASE WHEN TRIM(album) = '' THEN album ELSE album END),
                track_number = COALESCE(?4, track_number),
                disc_number = COALESCE(?5, disc_number),
                year = COALESCE(?6, year)
            WHERE id = ?7
            ",
            params![
                title,
                artist,
                album,
                track_number,
                disc_number,
                year,
                track_id
            ],
        )?;
        Ok(())
    }

    /// Get all tracks for a specific album (by album_artist + album name)
    pub fn get_all_tracks_unfiltered(&self) -> Result<Vec<Track>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT id,path,title,artist,album,album_artist,track_number,disc_number,year,
                   duration_secs,sample_rate,bit_depth,bitrate_kbps,format,file_size,
                   cover_art_path,isrc,musicbrainz_recording_id,musicbrainz_release_id,
                   canonical_artist_id,canonical_release_id,quality_tier,content_hash,added_at
            FROM tracks
            ORDER BY album_artist COLLATE NOCASE, album COLLATE NOCASE, disc_number, track_number
        ",
        )?;
        let rows = stmt.query_map([], Self::row_to_track)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Delete a track by ID
    pub fn delete_track(&self, track_id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM tracks WHERE id = ?1", params![track_id])?;
        Ok(())
    }

    /// Delete tracks whose files no longer exist on disk
    pub fn prune_missing_tracks(&self) -> Result<usize> {
        let all = self.get_all_tracks_unfiltered()?;
        let missing: Vec<i64> = all
            .iter()
            .filter(|t| !std::path::Path::new(&t.path).exists())
            .map(|t| t.id)
            .collect();

        if missing.is_empty() {
            return Ok(0);
        }

        self.conn.execute_batch("BEGIN IMMEDIATE TRANSACTION;")?;
        let write_result: Result<()> = (|| {
            for id in &missing {
                self.conn
                    .execute("DELETE FROM tracks WHERE id = ?1", params![id])?;
            }
            Ok(())
        })();
        if let Err(e) = write_result {
            let _ = self.conn.execute_batch("ROLLBACK;");
            return Err(e);
        }
        self.conn.execute_batch("COMMIT;")?;
        Ok(missing.len())
    }

    pub fn increment_play_count(&self, track_id: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE tracks SET play_count = play_count + 1, last_played = datetime('now') WHERE id = ?1",
            params![track_id],
        )?;
        Ok(())
    }

    pub fn increment_play_count_by_identity(
        &self,
        artist: &str,
        title: &str,
        played_at: Option<&str>,
    ) -> Result<usize> {
        let affected = self.conn.execute(
            "UPDATE tracks
             SET play_count = play_count + 1,
                 last_played = COALESCE(?3, datetime('now'))
             WHERE lower(trim(artist)) = lower(trim(?1))
               AND lower(trim(title)) = lower(trim(?2))",
            params![artist.trim(), title.trim(), played_at],
        )?;
        Ok(affected)
    }

    pub fn record_play_history_event(
        &self,
        source: &str,
        artist: &str,
        title: &str,
        album: Option<&str>,
        played_at: Option<&str>,
        track_id: Option<i64>,
    ) -> Result<bool> {
        let source = source.trim();
        let artist = artist.trim();
        let title = title.trim();
        if source.is_empty() || artist.is_empty() || title.is_empty() {
            return Ok(false);
        }

        let album_value = album
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.to_string());
        let played_at_value = played_at
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.to_string())
            .unwrap_or_else(|| Utc::now().format("%Y-%m-%d %H:%M:%S").to_string());

        let rows = self.conn.execute(
            "INSERT OR IGNORE INTO play_history_events
             (track_id, source, artist, title, album, played_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                track_id,
                source,
                artist,
                title,
                album_value,
                played_at_value
            ],
        )?;

        if rows == 0 {
            return Ok(false);
        }

        self.conn.execute(
            "INSERT INTO artist_play_history (artist, play_count, last_played)
             VALUES (?1, 1, ?2)
             ON CONFLICT(artist)
             DO UPDATE SET
               play_count = artist_play_history.play_count + 1,
               last_played = MAX(artist_play_history.last_played, excluded.last_played)",
            params![artist, played_at_value],
        )?;

        self.conn.execute(
            "INSERT INTO song_play_history (artist, title, album, play_count, last_played)
             VALUES (?1, ?2, ?3, 1, ?4)
             ON CONFLICT(artist, title, album)
             DO UPDATE SET
               play_count = song_play_history.play_count + 1,
               last_played = MAX(song_play_history.last_played, excluded.last_played)",
            params![
                artist,
                title,
                album_value.unwrap_or_default(),
                played_at_value
            ],
        )?;

        Ok(true)
    }

    fn row_to_track(row: &rusqlite::Row) -> rusqlite::Result<Track> {
        Ok(Track {
            id: row.get(0)?,
            path: row.get(1)?,
            title: row.get(2)?,
            artist: row.get(3)?,
            album: row.get(4)?,
            album_artist: row.get(5)?,
            track_number: row.get(6)?,
            disc_number: row.get(7)?,
            year: row.get(8)?,
            duration_secs: row.get(9)?,
            sample_rate: row.get::<_, Option<i64>>(10)?.map(|v| v as u32),
            bit_depth: row.get::<_, Option<i64>>(11)?.map(|v| v as u32),
            bitrate_kbps: row.get::<_, Option<i64>>(12)?.map(|v| v as u32),
            format: row.get(13)?,
            file_size: row.get::<_, i64>(14)? as u64,
            cover_art_path: row.get(15)?,
            isrc: row.get(16)?,
            musicbrainz_recording_id: row.get(17)?,
            musicbrainz_release_id: row.get(18)?,
            canonical_artist_id: row.get(19)?,
            canonical_release_id: row.get(20)?,
            quality_tier: row.get(21)?,
            content_hash: row.get(22)?,
            added_at: row.get(23).unwrap_or_default(),
        })
    }

    // ── Queue ─────────────────────────────────────────────────────────────────

    pub fn get_queue(&self) -> Result<Vec<QueueItem>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT q.id, q.track_id, q.position,
                   t.id,t.path,t.title,t.artist,t.album,t.album_artist,t.track_number,
                   t.disc_number,t.year,t.duration_secs,t.sample_rate,t.bit_depth,
                   t.bitrate_kbps,t.format,t.file_size,t.cover_art_path,t.isrc,
                   t.musicbrainz_recording_id,t.musicbrainz_release_id,t.canonical_artist_id,
                   t.canonical_release_id,t.quality_tier,t.content_hash,t.added_at
            FROM queue_items q JOIN tracks t ON q.track_id = t.id
            ORDER BY q.position
        ",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(QueueItem {
                id: row.get(0)?,
                track_id: row.get(1)?,
                position: row.get(2)?,
                track: Some(Track {
                    id: row.get(3)?,
                    path: row.get(4)?,
                    title: row.get(5)?,
                    artist: row.get(6)?,
                    album: row.get(7)?,
                    album_artist: row.get(8)?,
                    track_number: row.get(9)?,
                    disc_number: row.get(10)?,
                    year: row.get(11)?,
                    duration_secs: row.get(12)?,
                    sample_rate: row.get::<_, Option<i64>>(13)?.map(|v| v as u32),
                    bit_depth: row.get::<_, Option<i64>>(14)?.map(|v| v as u32),
                    bitrate_kbps: row.get::<_, Option<i64>>(15)?.map(|v| v as u32),
                    format: row.get(16)?,
                    file_size: row.get::<_, i64>(17)? as u64,
                    cover_art_path: row.get(18)?,
                    isrc: row.get(19)?,
                    musicbrainz_recording_id: row.get(20)?,
                    musicbrainz_release_id: row.get(21)?,
                    canonical_artist_id: row.get(22)?,
                    canonical_release_id: row.get(23)?,
                    quality_tier: row.get(24)?,
                    content_hash: row.get(25)?,
                    added_at: row.get(26).unwrap_or_default(),
                }),
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn clear_queue(&self) -> Result<()> {
        self.conn.execute("DELETE FROM queue_items", [])?;
        Ok(())
    }

    pub fn add_to_queue(&self, track_id: i64, position: i64) -> Result<()> {
        self.conn.execute(
            "INSERT INTO queue_items (track_id, position) VALUES (?1, ?2)",
            params![track_id, position],
        )?;
        Ok(())
    }

    pub fn get_max_queue_position(&self) -> Result<i64> {
        Ok(self.conn.query_row(
            "SELECT COALESCE(MAX(position), -1) FROM queue_items",
            [],
            |r| r.get(0),
        )?)
    }

    // ── Settings ──────────────────────────────────────────────────────────────

    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT value FROM settings WHERE key = ?1")?;
        let mut rows = stmt.query_map(params![key], |r| r.get(0))?;
        Ok(rows.next().transpose()?)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO settings (key,value) VALUES (?1,?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn delete_setting(&self, key: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM settings WHERE key = ?1", params![key])?;
        Ok(())
    }

    // —— Spotify History ————————————————————————————————————————————————————————————————————————————————————————

    pub fn replace_spotify_album_history(&self, rows: &[SpotifyAlbumHistory]) -> Result<()> {
        self.conn.execute_batch("BEGIN IMMEDIATE TRANSACTION;")?;
        let write_result: Result<()> = (|| {
            self.conn.execute("DELETE FROM spotify_album_history", [])?;
            let mut stmt = self.conn.prepare(
                "INSERT INTO spotify_album_history
                    (artist, album, total_ms, play_count, skip_count, in_library, imported_at)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, datetime('now'))",
            )?;
            for row in rows {
                stmt.execute(params![
                    row.artist,
                    row.album,
                    row.total_ms as i64,
                    row.play_count as i64,
                    row.skip_count as i64,
                    if row.in_library { 1 } else { 0 },
                ])?;
            }
            Ok(())
        })();
        if let Err(e) = write_result {
            let _ = self.conn.execute_batch("ROLLBACK;");
            return Err(e);
        }
        self.conn.execute_batch("COMMIT;")?;
        Ok(())
    }

    pub fn get_spotify_album_history_count(&self) -> Result<i64> {
        Ok(self
            .conn
            .query_row("SELECT COUNT(*) FROM spotify_album_history", [], |row| {
                row.get(0)
            })?)
    }

    pub fn get_spotify_album_history_last_imported_at(&self) -> Result<Option<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT MAX(imported_at) FROM spotify_album_history")?;
        let mut rows = stmt.query_map([], |row| row.get::<_, Option<String>>(0))?;
        Ok(rows.next().transpose()?.flatten())
    }

    pub fn save_director_task_result(
        &self,
        result: &DirectorTaskResult,
        request: Option<&TrackTask>,
    ) -> Result<()> {
        let request_signature = request.map(director_request_signature);
        let provider = derive_result_provider(result);
        let failure_class = classify_failure(result);
        let selected_provider_candidate_id = result
            .finalized
            .as_ref()
            .and_then(|finalized| finalized.provenance.selected_provider_candidate_id.clone());
        let final_path = result
            .finalized
            .as_ref()
            .map(|finalized| finalized.path.to_string_lossy().to_string());
        let score_total = result
            .finalized
            .as_ref()
            .map(|finalized| finalized.provenance.score_reason.details.get("score_total"))
            .and_then(|value| value.and_then(|value| value.parse::<i64>().ok()));
        let source_metadata_json = result
            .finalized
            .as_ref()
            .map(|finalized| serde_json::to_string(&finalized.provenance.source_metadata))
            .transpose()?;
        let validation_json = result
            .finalized
            .as_ref()
            .map(|finalized| serde_json::to_string(&finalized.provenance.validation_summary))
            .transpose()?;
        let score_reason_json = result
            .finalized
            .as_ref()
            .map(|finalized| serde_json::to_string(&finalized.provenance.score_reason))
            .transpose()?;
        let attempts_json = serde_json::to_string(&result.attempts)?;
        let result_json = serde_json::to_string(result)?;
        let request_json = request.map(serde_json::to_string).transpose()?;
        let request_strategy = request.map(|task| format!("{:?}", task.strategy));

        self.conn.execute_batch("BEGIN IMMEDIATE TRANSACTION;")?;
        let write_result: Result<()> = (|| {
            self.conn.execute(
                "
                INSERT INTO director_task_history
                    (task_id, disposition, provider, failure_class, final_path, request_signature, score_total, error,
                     source_metadata_json, validation_json, score_reason_json,
                     attempts_json, result_json, request_json, request_strategy, updated_at)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, datetime('now'))
                ON CONFLICT(task_id) DO UPDATE SET
                    disposition=excluded.disposition,
                    provider=COALESCE(excluded.provider, director_task_history.provider),
                    failure_class=COALESCE(excluded.failure_class, director_task_history.failure_class),
                    final_path=COALESCE(excluded.final_path, director_task_history.final_path),
                    request_signature=COALESCE(excluded.request_signature, director_task_history.request_signature),
                    score_total=COALESCE(excluded.score_total, director_task_history.score_total),
                    error=COALESCE(excluded.error, director_task_history.error),
                    source_metadata_json=excluded.source_metadata_json,
                    validation_json=excluded.validation_json,
                    score_reason_json=excluded.score_reason_json,
                    attempts_json=excluded.attempts_json,
                    result_json=excluded.result_json,
                    request_json=COALESCE(excluded.request_json, director_task_history.request_json),
                    request_strategy=COALESCE(excluded.request_strategy, director_task_history.request_strategy),
                    updated_at=datetime('now')
                ",
                params![
                    result.task_id,
                    format!("{:?}", result.disposition),
                    provider,
                    failure_class.as_deref(),
                    final_path,
                    request_signature,
                    score_total,
                    result.error,
                    source_metadata_json,
                    validation_json,
                    score_reason_json,
                    attempts_json,
                    result_json,
                    request_json,
                    request_strategy,
                ],
            )?;

            self.persist_candidate_set(
                result,
                request_signature.as_deref(),
                request_strategy.as_deref(),
                score_total,
                selected_provider_candidate_id.as_deref(),
            )?;
            self.persist_provider_searches(result, request_signature.as_deref())?;
            self.persist_candidate_items(
                result,
                request_signature.as_deref(),
                selected_provider_candidate_id.as_deref(),
            )?;
            self.persist_provider_attempts(result, request_signature.as_deref())?;
            self.refresh_provider_memory(result, request_signature.as_deref())?;
            self.persist_provider_search_evidence(result, request_signature.as_deref())?;
            self.persist_provider_candidate_evidence(
                result,
                request_signature.as_deref(),
                selected_provider_candidate_id.as_deref(),
            )?;
            self.refresh_provider_response_cache(
                result,
                request_signature.as_deref(),
                failure_class.as_deref(),
            )?;
            self.persist_identity_resolution_evidence(
                result,
                request,
                request_signature.as_deref(),
            )?;
            self.persist_source_aliases_for_result(result, request, request_signature.as_deref())?;
            Ok(())
        })();

        if let Err(error) = write_result {
            let _ = self.conn.execute_batch("ROLLBACK;");
            return Err(error);
        }
        self.conn.execute_batch("COMMIT;")?;
        Ok(())
    }

    fn persist_candidate_set(
        &self,
        result: &DirectorTaskResult,
        request_signature: Option<&str>,
        request_strategy: Option<&str>,
        score_total: Option<i64>,
        selected_provider_candidate_id: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            "
            INSERT INTO director_candidate_sets
                (task_id, request_signature, request_strategy, disposition, selected_provider,
                 selected_provider_candidate_id, selected_score_total, candidate_count, provider_count,
                 result_json, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, datetime('now'))
            ON CONFLICT(task_id) DO UPDATE SET
                request_signature=COALESCE(excluded.request_signature, director_candidate_sets.request_signature),
                request_strategy=COALESCE(excluded.request_strategy, director_candidate_sets.request_strategy),
                disposition=excluded.disposition,
                selected_provider=excluded.selected_provider,
                selected_provider_candidate_id=excluded.selected_provider_candidate_id,
                selected_score_total=excluded.selected_score_total,
                candidate_count=excluded.candidate_count,
                provider_count=excluded.provider_count,
                result_json=excluded.result_json,
                updated_at=datetime('now')
            ",
            params![
                result.task_id,
                request_signature,
                request_strategy,
                format!("{:?}", result.disposition),
                result
                    .finalized
                    .as_ref()
                    .map(|finalized| finalized.provenance.selected_provider.as_str()),
                selected_provider_candidate_id,
                score_total,
                result.candidate_records.len() as i64,
                result.provider_searches.len() as i64,
                serde_json::to_string(result)?,
            ],
        )?;
        Ok(())
    }

    fn persist_candidate_items(
        &self,
        result: &DirectorTaskResult,
        request_signature: Option<&str>,
        selected_provider_candidate_id: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            "DELETE FROM director_candidate_items WHERE task_id = ?1",
            params![result.task_id],
        )?;

        let selected_provider_id = result
            .finalized
            .as_ref()
            .map(|finalized| finalized.provenance.selected_provider.as_str());
        let mut stmt = self.conn.prepare(
            "
            INSERT INTO director_candidate_items
                (task_id, request_signature, provider_id, provider_display_name, provider_trust_rank,
                 provider_order_index, search_rank, provider_candidate_id, outcome, rejection_reason,
                 is_selected, acquisition_temp_path, score_total, candidate_json, validation_json,
                 score_json, score_reason_json, recorded_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, datetime('now'))
            ",
        )?;

        for record in &result.candidate_records {
            let is_selected = selected_provider_id == Some(record.provider_id.as_str())
                && selected_provider_candidate_id
                    == Some(record.candidate.provider_candidate_id.as_str());
            let validation_json = record
                .validation
                .as_ref()
                .map(serde_json::to_string)
                .transpose()?;
            let score_json = record
                .score
                .as_ref()
                .map(serde_json::to_string)
                .transpose()?;
            let score_reason_json = record
                .score_reason
                .as_ref()
                .map(serde_json::to_string)
                .transpose()?;
            stmt.execute(params![
                result.task_id,
                request_signature,
                record.provider_id,
                record.provider_display_name,
                record.provider_trust_rank,
                record.provider_order_index as i64,
                record.search_rank as i64,
                record.candidate.provider_candidate_id,
                record.outcome,
                record.rejection_reason,
                if is_selected { 1_i64 } else { 0_i64 },
                record
                    .acquisition_temp_path
                    .as_ref()
                    .map(|path| path.to_string_lossy().to_string()),
                record.score.as_ref().map(|score| i64::from(score.total)),
                serde_json::to_string(&record.candidate)?,
                validation_json,
                score_json,
                score_reason_json,
            ])?;
        }

        Ok(())
    }

    fn persist_provider_searches(
        &self,
        result: &DirectorTaskResult,
        request_signature: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            "DELETE FROM director_provider_searches WHERE task_id = ?1",
            params![result.task_id],
        )?;
        let mut stmt = self.conn.prepare(
            "
            INSERT INTO director_provider_searches
                (task_id, request_signature, provider_id, provider_display_name, provider_trust_rank,
                 provider_order_index, outcome, candidate_count, error, retryable, recorded_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, datetime('now'))
            ",
        )?;

        for record in &result.provider_searches {
            stmt.execute(params![
                result.task_id,
                request_signature,
                record.provider_id,
                record.provider_display_name,
                record.provider_trust_rank,
                record.provider_order_index as i64,
                record.outcome,
                record.candidate_count as i64,
                record.error,
                if record.retryable { 1_i64 } else { 0_i64 },
            ])?;
        }
        Ok(())
    }

    fn persist_provider_attempts(
        &self,
        result: &DirectorTaskResult,
        request_signature: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            "DELETE FROM director_provider_attempts WHERE task_id = ?1",
            params![result.task_id],
        )?;
        let mut stmt = self.conn.prepare(
            "
            INSERT INTO director_provider_attempts
                (task_id, request_signature, provider_id, attempt_index, attempt_number, outcome, recorded_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, datetime('now'))
            ",
        )?;
        for (index, attempt) in result.attempts.iter().enumerate() {
            stmt.execute(params![
                result.task_id,
                request_signature,
                attempt.provider_id,
                index as i64,
                attempt.attempt as i64,
                attempt.outcome,
            ])?;
        }
        Ok(())
    }

    fn refresh_provider_memory(
        &self,
        result: &DirectorTaskResult,
        request_signature: Option<&str>,
    ) -> Result<()> {
        let Some(request_signature) = request_signature else {
            return Ok(());
        };

        let mut provider_ids = std::collections::BTreeSet::<String>::new();
        for record in &result.provider_searches {
            provider_ids.insert(record.provider_id.clone());
        }
        for record in &result.candidate_records {
            provider_ids.insert(record.provider_id.clone());
        }

        let selected_provider_id = result
            .finalized
            .as_ref()
            .map(|finalized| finalized.provenance.selected_provider.as_str());
        let selected_candidate_id = result.finalized.as_ref().and_then(|finalized| {
            finalized
                .provenance
                .selected_provider_candidate_id
                .as_deref()
        });

        for provider_id in provider_ids {
            let provider_candidates = result
                .candidate_records
                .iter()
                .filter(|record| record.provider_id == provider_id)
                .collect::<Vec<_>>();
            let provider_searches = result
                .provider_searches
                .iter()
                .filter(|record| record.provider_id == provider_id)
                .collect::<Vec<_>>();

            let selected_here = selected_provider_id == Some(provider_id.as_str())
                && provider_candidates.iter().any(|record| {
                    selected_candidate_id == Some(record.candidate.provider_candidate_id.as_str())
                });

            if selected_here {
                self.conn.execute(
                    "DELETE FROM director_provider_memory WHERE request_signature = ?1 AND provider_id = ?2",
                    params![request_signature, provider_id],
                )?;
                continue;
            }

            let summary =
                summarize_provider_memory(&provider_id, &provider_searches, &provider_candidates);
            let Some(summary) = summary else {
                continue;
            };
            let backoff_until = summary.retryable.then(|| {
                (Utc::now() + Duration::minutes(15))
                    .format("%Y-%m-%d %H:%M:%S")
                    .to_string()
            });

            self.conn.execute(
                "
                INSERT INTO director_provider_memory
                    (request_signature, provider_id, last_task_id, last_outcome, failure_class,
                     error, retryable, candidate_count, backoff_until, updated_at)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, datetime('now'))
                ON CONFLICT(request_signature, provider_id) DO UPDATE SET
                    last_task_id=excluded.last_task_id,
                    last_outcome=excluded.last_outcome,
                    failure_class=excluded.failure_class,
                    error=excluded.error,
                    retryable=excluded.retryable,
                    candidate_count=excluded.candidate_count,
                    backoff_until=excluded.backoff_until,
                    updated_at=datetime('now')
                ",
                params![
                    request_signature,
                    provider_id,
                    result.task_id,
                    summary.last_outcome,
                    summary.failure_class,
                    summary.error,
                    if summary.retryable { 1_i64 } else { 0_i64 },
                    summary.candidate_count as i64,
                    backoff_until,
                ],
            )?;
        }

        Ok(())
    }

    fn persist_provider_search_evidence(
        &self,
        result: &DirectorTaskResult,
        request_signature: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            "DELETE FROM provider_search_evidence WHERE task_id = ?1",
            params![result.task_id],
        )?;

        let mut stmt = self.conn.prepare(
            "
            INSERT INTO provider_search_evidence
                (task_id, request_signature, provider_id, outcome, candidate_count,
                 retryable, error, raw_json, retention_class, recorded_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'provenance', datetime('now'))
            ",
        )?;

        for record in &result.provider_searches {
            stmt.execute(params![
                result.task_id,
                request_signature,
                record.provider_id,
                record.outcome,
                record.candidate_count as i64,
                if record.retryable { 1_i64 } else { 0_i64 },
                record.error,
                serde_json::to_string(record)?,
            ])?;
        }
        Ok(())
    }

    fn persist_provider_candidate_evidence(
        &self,
        result: &DirectorTaskResult,
        request_signature: Option<&str>,
        selected_provider_candidate_id: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            "DELETE FROM provider_candidate_evidence WHERE task_id = ?1",
            params![result.task_id],
        )?;

        let selected_provider_id = result
            .finalized
            .as_ref()
            .map(|finalized| finalized.provenance.selected_provider.as_str());
        let mut stmt = self.conn.prepare(
            "
            INSERT INTO provider_candidate_evidence
                (task_id, request_signature, provider_id, provider_candidate_id, outcome,
                 rejection_reason, is_selected, metadata_confidence, raw_json, retention_class, recorded_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'provenance', datetime('now'))
            ",
        )?;

        for record in &result.candidate_records {
            let is_selected = selected_provider_id == Some(record.provider_id.as_str())
                && selected_provider_candidate_id
                    == Some(record.candidate.provider_candidate_id.as_str());
            stmt.execute(params![
                result.task_id,
                request_signature,
                record.provider_id,
                record.candidate.provider_candidate_id,
                record.outcome,
                record.rejection_reason,
                if is_selected { 1_i64 } else { 0_i64 },
                f64::from(record.candidate.metadata_confidence),
                serde_json::to_string(record)?,
            ])?;
        }
        Ok(())
    }

    fn refresh_provider_response_cache(
        &self,
        result: &DirectorTaskResult,
        request_signature: Option<&str>,
        failure_class: Option<&str>,
    ) -> Result<()> {
        let Some(request_signature) = request_signature else {
            return Ok(());
        };

        let mut provider_ids = std::collections::BTreeSet::<String>::new();
        for record in &result.provider_searches {
            provider_ids.insert(record.provider_id.clone());
        }
        for record in &result.candidate_records {
            provider_ids.insert(record.provider_id.clone());
        }

        for provider_id in provider_ids {
            let provider_searches = result
                .provider_searches
                .iter()
                .filter(|record| record.provider_id == provider_id)
                .cloned()
                .collect::<Vec<_>>();
            let provider_candidates = result
                .candidate_records
                .iter()
                .filter(|record| record.provider_id == provider_id)
                .cloned()
                .collect::<Vec<_>>();

            let response_json = serde_json::json!({
                "provider_searches": provider_searches,
                "candidate_records": provider_candidates,
                "task_id": result.task_id,
            })
            .to_string();

            self.conn.execute(
                "
                INSERT INTO provider_response_cache
                    (request_signature, provider_id, last_task_id, outcome, candidate_count,
                     response_json, failure_class, backoff_until, retention_class, updated_at)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, 'ephemeral_cache', datetime('now'))
                ON CONFLICT(request_signature, provider_id) DO UPDATE SET
                    last_task_id = excluded.last_task_id,
                    outcome = excluded.outcome,
                    candidate_count = excluded.candidate_count,
                    response_json = excluded.response_json,
                    failure_class = excluded.failure_class,
                    updated_at = datetime('now')
                ",
                params![
                    request_signature,
                    provider_id,
                    result.task_id,
                    provider_cache_outcome(
                        &result.disposition,
                        &provider_searches,
                        &provider_candidates
                    ),
                    provider_candidates.len() as i64,
                    response_json,
                    failure_class,
                ],
            )?;
        }
        Ok(())
    }

    fn refresh_provider_response_cache_from_planner(
        &self,
        task_id: &str,
        request_signature: &str,
        provider_searches: &[ProviderSearchRecord],
        candidate_records: &[CandidateRecord],
    ) -> Result<()> {
        let mut provider_ids = std::collections::BTreeSet::<String>::new();
        for record in provider_searches {
            provider_ids.insert(record.provider_id.clone());
        }
        for record in candidate_records {
            provider_ids.insert(record.provider_id.clone());
        }

        for provider_id in provider_ids {
            let search_rows = provider_searches
                .iter()
                .filter(|record| record.provider_id == provider_id)
                .cloned()
                .collect::<Vec<_>>();
            let candidate_rows = candidate_records
                .iter()
                .filter(|record| record.provider_id == provider_id)
                .cloned()
                .collect::<Vec<_>>();
            let outcome = search_rows
                .last()
                .map(|record| record.outcome.clone())
                .unwrap_or_else(|| {
                    if candidate_rows.is_empty() {
                        "not_searched".to_string()
                    } else {
                        "planned".to_string()
                    }
                });
            let candidate_count = candidate_rows.len() as i64;

            let response_json = serde_json::json!({
                "provider_searches": search_rows,
                "candidate_records": candidate_rows,
                "task_id": task_id,
            })
            .to_string();

            self.conn.execute(
                "
                INSERT INTO provider_response_cache
                    (request_signature, provider_id, last_task_id, outcome, candidate_count,
                     response_json, failure_class, backoff_until, retention_class, updated_at)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, 'planner_cache', datetime('now'))
                ON CONFLICT(request_signature, provider_id) DO UPDATE SET
                    last_task_id = excluded.last_task_id,
                    outcome = excluded.outcome,
                    candidate_count = excluded.candidate_count,
                    response_json = excluded.response_json,
                    failure_class = NULL,
                    updated_at = datetime('now')
                ",
                params![
                    request_signature,
                    provider_id,
                    task_id,
                    outcome,
                    candidate_count,
                    response_json,
                ],
            )?;
        }

        Ok(())
    }

    fn persist_identity_resolution_evidence(
        &self,
        result: &DirectorTaskResult,
        request: Option<&TrackTask>,
        request_signature: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            "DELETE FROM identity_resolution_evidence WHERE task_id = ?1",
            params![result.task_id],
        )?;

        let Some(request) = request else {
            return Ok(());
        };
        let raw_json = serde_json::to_string(&request.target)?;
        self.conn.execute(
            "
            INSERT INTO identity_resolution_evidence
                (task_id, request_signature, entity_type, entity_key, source_name, evidence_type,
                 canonical_artist_id, canonical_release_id, musicbrainz_recording_id, musicbrainz_release_id,
                 isrc, confidence, raw_json, retention_class, recorded_at)
            VALUES (?1, ?2, 'request_signature', ?3, 'director_request', 'normalized_target',
                    ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'identity_fact', datetime('now'))
            ",
            params![
                result.task_id,
                request_signature,
                request_signature.unwrap_or_default(),
                request.target.canonical_artist_id,
                request.target.canonical_release_id,
                request.target.musicbrainz_recording_id.as_deref(),
                request.target.musicbrainz_release_id.as_deref(),
                request.target.isrc.as_deref(),
                Some(1.0_f64),
                raw_json,
            ],
        )?;
        Ok(())
    }

    fn persist_source_aliases_for_result(
        &self,
        result: &DirectorTaskResult,
        request: Option<&TrackTask>,
        request_signature: Option<&str>,
    ) -> Result<()> {
        let Some(request_signature) = request_signature else {
            return Ok(());
        };
        let entity_type = "request_signature";
        let entity_key = request_signature;

        if let Some(request) = request {
            if let Some(spotify_track_id) = request.target.spotify_track_id.as_deref() {
                self.upsert_source_alias(
                    entity_type,
                    entity_key,
                    "spotify",
                    "track_id",
                    spotify_track_id,
                    Some(1.0),
                    Some(&serde_json::to_string(&request.target)?),
                )?;
            }
            if let Some(recording_id) = request.target.musicbrainz_recording_id.as_deref() {
                self.upsert_source_alias(
                    entity_type,
                    entity_key,
                    "musicbrainz",
                    "recording_id",
                    recording_id,
                    Some(1.0),
                    Some(&serde_json::to_string(&request.target)?),
                )?;
            }
            if let Some(release_group_id) = request.target.musicbrainz_release_group_id.as_deref() {
                self.upsert_source_alias(
                    entity_type,
                    entity_key,
                    "musicbrainz",
                    "release_group_id",
                    release_group_id,
                    Some(1.0),
                    Some(&serde_json::to_string(&request.target)?),
                )?;
            }
            if let Some(release_id) = request.target.musicbrainz_release_id.as_deref() {
                self.upsert_source_alias(
                    entity_type,
                    entity_key,
                    "musicbrainz",
                    "release_id",
                    release_id,
                    Some(1.0),
                    Some(&serde_json::to_string(&request.target)?),
                )?;
            }
        }

        for record in &result.candidate_records {
            self.upsert_source_alias(
                entity_type,
                entity_key,
                &record.provider_id,
                "candidate_id",
                &record.candidate.provider_candidate_id,
                Some(f64::from(record.candidate.metadata_confidence)),
                Some(&serde_json::to_string(record)?),
            )?;
        }
        Ok(())
    }

    /// Persist or refresh a director task that has not reached a terminal state yet.
    pub fn upsert_director_pending_task(&self, task: &TrackTask, progress: &str) -> Result<()> {
        let task_json = serde_json::to_string(task)?;
        let strategy = format!("{:?}", task.strategy);
        let request_signature = director_request_signature(task);
        self.conn.execute(
            "
            INSERT INTO director_pending_tasks
                (task_id, track_json, request_signature, strategy, progress, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, datetime('now'), datetime('now'))
            ON CONFLICT(task_id) DO UPDATE SET
                track_json=excluded.track_json,
                request_signature=excluded.request_signature,
                strategy=excluded.strategy,
                progress=excluded.progress,
                updated_at=datetime('now')
            ",
            params![
                task.task_id,
                task_json,
                request_signature,
                strategy,
                progress
            ],
        )?;
        Ok(())
    }

    /// Refresh the stored progress for a pending director task without changing its payload.
    pub fn update_director_pending_task_progress(
        &self,
        task_id: &str,
        progress: &str,
    ) -> Result<()> {
        self.conn.execute(
            "
            UPDATE director_pending_tasks
            SET progress = ?2, updated_at = datetime('now')
            WHERE task_id = ?1
            ",
            params![task_id, progress],
        )?;
        Ok(())
    }

    /// Remove a pending director task from crash recovery tracking.
    pub fn delete_director_pending_task(&self, task_id: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM director_pending_tasks WHERE task_id = ?1",
            params![task_id],
        )?;
        Ok(())
    }

    /// Read all pending director tasks for startup recovery.
    pub fn get_pending_director_tasks(&self) -> Result<Vec<PendingDirectorTask>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT task_id, track_json, strategy, progress, created_at, updated_at
            FROM director_pending_tasks
            ORDER BY created_at ASC, task_id ASC
            ",
        )?;
        let rows = stmt.query_map([], |row| {
            let task_id: String = row.get(0)?;
            let mut task: TrackTask =
                serde_json::from_str(&row.get::<_, String>(1)?).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        1,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?;
            task.task_id = task_id.clone();
            Ok(PendingDirectorTask {
                task,
                strategy: row.get(2)?,
                progress: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn get_pending_director_task(&self, task_id: &str) -> Result<Option<PendingDirectorTask>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT task_id, track_json, strategy, progress, created_at, updated_at
            FROM director_pending_tasks
            WHERE task_id = ?1
            ",
        )?;
        let mut rows = stmt.query_map(params![task_id], |row| {
            let task_id: String = row.get(0)?;
            let mut task: TrackTask =
                serde_json::from_str(&row.get::<_, String>(1)?).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        1,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?;
            task.task_id = task_id.clone();
            Ok(PendingDirectorTask {
                task,
                strategy: row.get(2)?,
                progress: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            })
        })?;
        Ok(rows.next().transpose()?)
    }

    pub fn get_director_candidate_items(
        &self,
        task_id: &str,
    ) -> Result<Vec<StoredDirectorCandidateItem>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT provider_id, provider_candidate_id, outcome, rejection_reason, is_selected
            FROM director_candidate_items
            WHERE task_id = ?1
            ORDER BY provider_order_index ASC, search_rank ASC, id ASC
            ",
        )?;
        let rows = stmt.query_map(params![task_id], |row| {
            Ok(StoredDirectorCandidateItem {
                provider_id: row.get(0)?,
                provider_candidate_id: row.get(1)?,
                outcome: row.get(2)?,
                rejection_reason: row.get(3)?,
                is_selected: row.get::<_, i64>(4)? != 0,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn get_director_provider_memory(
        &self,
        request_signature: &str,
    ) -> Result<Vec<StoredProviderMemory>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT provider_id, last_outcome, failure_class, retryable, candidate_count,
                   backoff_until, updated_at
            FROM director_provider_memory
            WHERE request_signature = ?1
            ORDER BY provider_id ASC
            ",
        )?;
        let rows = stmt.query_map(params![request_signature], |row| {
            Ok(StoredProviderMemory {
                provider_id: row.get(0)?,
                last_outcome: row.get(1)?,
                failure_class: row.get(2)?,
                retryable: row.get::<_, i64>(3)? != 0,
                candidate_count: row.get::<_, i64>(4)? as usize,
                backoff_until: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn get_provider_response_cache(
        &self,
        request_signature: &str,
    ) -> Result<Vec<StoredProviderResponseCache>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT provider_id, outcome, candidate_count, response_json, failure_class,
                   backoff_until, retention_class, updated_at
            FROM provider_response_cache
            WHERE request_signature = ?1
            ORDER BY provider_id ASC
            ",
        )?;
        let rows = stmt.query_map(params![request_signature], |row| {
            Ok(StoredProviderResponseCache {
                provider_id: row.get(0)?,
                outcome: row.get(1)?,
                candidate_count: row.get::<_, i64>(2)? as usize,
                response_json: row.get(3)?,
                failure_class: row.get(4)?,
                backoff_until: row.get(5)?,
                retention_class: row.get(6)?,
                updated_at: row.get(7)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn get_candidate_set_summary(
        &self,
        task_id: &str,
    ) -> Result<Option<StoredCandidateSetSummary>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT task_id, request_signature, request_strategy, disposition,
                   selected_provider, candidate_count, provider_count, updated_at
            FROM director_candidate_sets
            WHERE task_id = ?1
            LIMIT 1
            ",
        )?;
        let mut rows = stmt.query_map(params![task_id], |row| {
            Ok(StoredCandidateSetSummary {
                task_id: row.get(0)?,
                request_signature: row.get(1)?,
                request_strategy: row.get(2)?,
                disposition: row.get(3)?,
                selected_provider: row.get(4)?,
                candidate_count: row.get::<_, i64>(5)? as usize,
                provider_count: row.get::<_, i64>(6)? as usize,
                updated_at: row.get(7)?,
            })
        })?;
        Ok(rows.next().transpose()?)
    }

    pub fn get_provider_search_records(
        &self,
        task_id: &str,
    ) -> Result<Vec<StoredProviderSearchRecord>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT provider_id, provider_display_name, provider_trust_rank, provider_order_index,
                   outcome, candidate_count, error, retryable, recorded_at
            FROM director_provider_searches
            WHERE task_id = ?1
            ORDER BY provider_order_index ASC, id ASC
            ",
        )?;
        let rows = stmt.query_map(params![task_id], |row| {
            Ok(StoredProviderSearchRecord {
                provider_id: row.get(0)?,
                provider_display_name: row.get(1)?,
                provider_trust_rank: row.get(2)?,
                provider_order_index: row.get::<_, i64>(3)? as usize,
                outcome: row.get(4)?,
                candidate_count: row.get::<_, i64>(5)? as usize,
                error: row.get(6)?,
                retryable: row.get::<_, i64>(7)? != 0,
                recorded_at: row.get(8)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn get_identity_resolution_evidence_for_request(
        &self,
        request_signature: &str,
    ) -> Result<Vec<StoredIdentityResolutionEvidence>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT task_id, request_signature, entity_type, entity_key, source_name, evidence_type,
                   canonical_artist_id, canonical_release_id, musicbrainz_recording_id,
                   musicbrainz_release_id, isrc, confidence, raw_json, retention_class, recorded_at
            FROM identity_resolution_evidence
            WHERE request_signature = ?1
            ORDER BY recorded_at DESC, id DESC
            ",
        )?;
        let rows = stmt.query_map(params![request_signature], |row| {
            Ok(StoredIdentityResolutionEvidence {
                task_id: row.get(0)?,
                request_signature: row.get(1)?,
                entity_type: row.get(2)?,
                entity_key: row.get(3)?,
                source_name: row.get(4)?,
                evidence_type: row.get(5)?,
                canonical_artist_id: row.get(6)?,
                canonical_release_id: row.get(7)?,
                musicbrainz_recording_id: row.get(8)?,
                musicbrainz_release_id: row.get(9)?,
                isrc: row.get(10)?,
                confidence: row.get(11)?,
                raw_json: row.get(12)?,
                retention_class: row.get(13)?,
                recorded_at: row.get(14)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn get_source_aliases_for_entity(
        &self,
        entity_type: &str,
        entity_key: &str,
    ) -> Result<Vec<StoredSourceAlias>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT entity_type, entity_key, source_name, source_key, source_value,
                   confidence, raw_json, updated_at
            FROM source_aliases
            WHERE entity_type = ?1 AND entity_key = ?2
            ORDER BY source_name ASC, source_key ASC, source_value ASC
            ",
        )?;
        let rows = stmt.query_map(params![entity_type, entity_key], |row| {
            Ok(StoredSourceAlias {
                entity_type: row.get(0)?,
                entity_key: row.get(1)?,
                source_name: row.get(2)?,
                source_key: row.get(3)?,
                source_value: row.get(4)?,
                confidence: row.get(5)?,
                raw_json: row.get(6)?,
                updated_at: row.get(7)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn record_request_identity_snapshot(
        &self,
        task: &TrackTask,
        request_signature: &str,
    ) -> Result<()> {
        self.conn.execute(
            "DELETE FROM identity_resolution_evidence WHERE task_id = ?1",
            params![task.task_id],
        )?;

        let raw_json = serde_json::to_string(&task.target)?;
        self.conn.execute(
            "
            INSERT INTO identity_resolution_evidence
                (task_id, request_signature, entity_type, entity_key, source_name, evidence_type,
                 canonical_artist_id, canonical_release_id, musicbrainz_recording_id, musicbrainz_release_id,
                 isrc, confidence, raw_json, retention_class, recorded_at)
            VALUES (?1, ?2, 'request_signature', ?3, 'planner_request', 'normalized_target',
                    ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'identity_fact', datetime('now'))
            ",
            params![
                task.task_id,
                request_signature,
                request_signature,
                task.target.canonical_artist_id,
                task.target.canonical_release_id,
                task.target.musicbrainz_recording_id.as_deref(),
                task.target.musicbrainz_release_id.as_deref(),
                task.target.isrc.as_deref(),
                Some(1.0_f64),
                raw_json,
            ],
        )?;
        Ok(())
    }

    pub fn record_request_source_aliases(
        &self,
        request: &PlannerAcquisitionRequest,
        request_signature: &str,
    ) -> Result<()> {
        let entity_type = "request_signature";
        let entity_key = request_signature;
        let raw_json = request.raw_payload_json.as_deref();

        if let Some(source_track_id) = request.source_track_id.as_deref() {
            self.upsert_source_alias(
                entity_type,
                entity_key,
                &request.source_name,
                "track_id",
                source_track_id,
                Some(1.0),
                raw_json,
            )?;
        }
        if let Some(source_album_id) = request.source_album_id.as_deref() {
            self.upsert_source_alias(
                entity_type,
                entity_key,
                &request.source_name,
                "album_id",
                source_album_id,
                Some(1.0),
                raw_json,
            )?;
        }
        if let Some(source_artist_id) = request.source_artist_id.as_deref() {
            self.upsert_source_alias(
                entity_type,
                entity_key,
                &request.source_name,
                "artist_id",
                source_artist_id,
                Some(1.0),
                raw_json,
            )?;
        }
        if let Some(recording_id) = request.musicbrainz_recording_id.as_deref() {
            self.upsert_source_alias(
                entity_type,
                entity_key,
                "musicbrainz",
                "recording_id",
                recording_id,
                Some(1.0),
                raw_json,
            )?;
        }
        if let Some(release_group_id) = request.musicbrainz_release_group_id.as_deref() {
            self.upsert_source_alias(
                entity_type,
                entity_key,
                "musicbrainz",
                "release_group_id",
                release_group_id,
                Some(1.0),
                raw_json,
            )?;
        }
        if let Some(release_id) = request.musicbrainz_release_id.as_deref() {
            self.upsert_source_alias(
                entity_type,
                entity_key,
                "musicbrainz",
                "release_id",
                release_id,
                Some(1.0),
                raw_json,
            )?;
        }
        Ok(())
    }

    pub fn persist_planned_candidate_set(
        &self,
        task: &TrackTask,
        request_strategy: &str,
        provider_searches: &[ProviderSearchRecord],
        candidate_records: &[CandidateRecord],
    ) -> Result<()> {
        let request_signature = director_request_signature(task);
        let provider_count = provider_searches
            .iter()
            .map(|record| record.provider_id.as_str())
            .collect::<std::collections::BTreeSet<_>>()
            .len();
        let envelope = serde_json::json!({
            "provider_searches": provider_searches,
            "candidate_records": candidate_records,
            "task_id": task.task_id,
            "mode": "planner",
        })
        .to_string();

        self.conn.execute(
            "
            INSERT INTO director_candidate_sets
                (task_id, request_signature, request_strategy, disposition, selected_provider,
                 selected_provider_candidate_id, selected_score_total, candidate_count, provider_count,
                 result_json, created_at, updated_at)
            VALUES (?1, ?2, ?3, 'Planned', NULL, NULL, NULL, ?4, ?5, ?6, datetime('now'), datetime('now'))
            ON CONFLICT(task_id) DO UPDATE SET
                request_signature = excluded.request_signature,
                request_strategy = excluded.request_strategy,
                disposition = excluded.disposition,
                selected_provider = NULL,
                selected_provider_candidate_id = NULL,
                selected_score_total = NULL,
                candidate_count = excluded.candidate_count,
                provider_count = excluded.provider_count,
                result_json = excluded.result_json,
                updated_at = datetime('now')
            ",
            params![
                task.task_id,
                request_signature,
                request_strategy,
                candidate_records.len() as i64,
                provider_count as i64,
                envelope,
            ],
        )?;

        self.conn.execute(
            "DELETE FROM director_candidate_items WHERE task_id = ?1",
            params![task.task_id],
        )?;
        self.conn.execute(
            "DELETE FROM director_provider_searches WHERE task_id = ?1",
            params![task.task_id],
        )?;

        for record in candidate_records {
            self.conn.execute(
                "
                INSERT INTO director_candidate_items
                    (task_id, request_signature, provider_id, provider_display_name, provider_trust_rank,
                     provider_order_index, search_rank, provider_candidate_id, outcome, rejection_reason,
                     is_selected, acquisition_temp_path, score_total, candidate_json, validation_json,
                     score_json, score_reason_json, recorded_at)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 0, NULL, NULL, ?11, NULL, NULL, NULL, datetime('now'))
                ",
                params![
                    task.task_id,
                    request_signature,
                    record.provider_id,
                    record.provider_display_name,
                    record.provider_trust_rank,
                    record.provider_order_index as i64,
                    record.search_rank as i64,
                    record.candidate.provider_candidate_id,
                    record.outcome,
                    record.rejection_reason,
                    serde_json::to_string(record)?,
                ],
            )?;
        }

        for record in provider_searches {
            self.conn.execute(
                "
                INSERT INTO director_provider_searches
                    (task_id, request_signature, provider_id, provider_display_name, provider_trust_rank,
                     provider_order_index, outcome, candidate_count, error, retryable, recorded_at)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, datetime('now'))
                ",
                params![
                    task.task_id,
                    request_signature,
                    record.provider_id,
                    record.provider_display_name,
                    record.provider_trust_rank,
                    record.provider_order_index as i64,
                    record.outcome,
                    record.candidate_count as i64,
                    record.error,
                    if record.retryable { 1_i64 } else { 0_i64 },
                ],
            )?;
        }

        self.refresh_provider_response_cache_from_planner(
            &task.task_id,
            &request_signature,
            provider_searches,
            candidate_records,
        )?;

        Ok(())
    }

    // ── Batch Acquisition Helpers ──────────────────────────────────────────────

    /// Returns missing albums from Spotify history that have significant listening
    /// time (>30min) and play count (>10), ordered by play_count descending.
    pub fn get_missing_spotify_albums(&self, limit: usize) -> Result<Vec<SpotifyAlbumHistory>> {
        self.get_missing_spotify_albums_with_min_plays(10)
            .map(|mut v| {
                v.truncate(limit);
                v
            })
    }

    pub fn get_missing_spotify_albums_with_min_plays(
        &self,
        min_plays: i64,
    ) -> Result<Vec<SpotifyAlbumHistory>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT artist, album, total_ms, play_count, skip_count, in_library, imported_at
            FROM spotify_album_history
            WHERE in_library = 0 AND total_ms > 1800000 AND play_count >= ?1
            ORDER BY play_count DESC
        ",
        )?;
        let rows = stmt.query_map(params![min_plays], |row| {
            Ok(SpotifyAlbumHistory {
                artist: row.get(0)?,
                album: row.get(1)?,
                total_ms: row.get(2)?,
                play_count: row.get(3)?,
                skip_count: row.get(4)?,
                in_library: row.get::<_, i64>(5)? != 0,
                imported_at: row.get(6)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn mark_spotify_album_in_library(&self, artist: &str, album: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE spotify_album_history SET in_library = 1 WHERE artist = ?1 AND album = ?2",
            params![artist, album],
        )?;
        Ok(())
    }

    /// Returns task_ids from director_task_history that have already been finalized
    /// or marked as already-present, so batch submissions can skip them.
    pub fn get_completed_task_keys(&self) -> Result<std::collections::HashSet<String>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT task_id FROM director_task_history
            WHERE disposition IN ('Finalized', 'AlreadyPresent')
        ",
        )?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        Ok(rows.collect::<rusqlite::Result<std::collections::HashSet<_>>>()?)
    }

    /// Returns task_ids that should never be resubmitted from pending recovery.
    pub fn get_non_resumable_task_keys(&self) -> Result<std::collections::HashSet<String>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT task_id FROM director_task_history
            WHERE disposition IN ('Finalized', 'AlreadyPresent', 'Cancelled')
        ",
        )?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        Ok(rows.collect::<rusqlite::Result<std::collections::HashSet<_>>>()?)
    }

    /// Returns the latest terminal result timestamp for each task_id so startup recovery can
    /// distinguish stale pending rows from newer retry attempts that reuse a stable task key.
    pub fn get_terminal_director_task_updates(
        &self,
    ) -> Result<HashMap<String, TerminalDirectorTaskUpdate>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT task_id, disposition, updated_at
            FROM director_task_history
            ",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                TerminalDirectorTaskUpdate {
                    disposition: row.get(1)?,
                    updated_at: row.get(2)?,
                },
            ))
        })?;

        let mut updates: HashMap<String, TerminalDirectorTaskUpdate> = HashMap::new();
        for row in rows {
            let (task_id, update) = row?;
            match updates.get(&task_id) {
                Some(existing) if existing.updated_at >= update.updated_at => {}
                _ => {
                    updates.insert(task_id, update);
                }
            }
        }
        Ok(updates)
    }

    /// Returns task_ids from director_task_history where disposition = 'Failed'.
    pub fn get_failed_task_ids(&self) -> Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT task_id FROM director_task_history WHERE disposition = 'Failed'")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Return permanently failed/cancelled director task history grouped by failure_class.
    /// Each group includes up to `recent_limit` most recent items.
    pub fn get_dead_letter_summary(&self, recent_limit: usize) -> Result<DeadLetterSummary> {
        #[derive(Debug)]
        struct GroupCount {
            failure_class: String,
            count: usize,
        }

        let mut count_stmt = self.conn.prepare(
            "SELECT COALESCE(failure_class, 'provider_exhausted') AS failure_class,
                    COUNT(*) AS count
             FROM director_task_history
             WHERE disposition IN ('Failed', 'Cancelled')
             GROUP BY failure_class
             ORDER BY count DESC, failure_class ASC",
        )?;

        let group_counts = count_stmt
            .query_map([], |row| {
                Ok(GroupCount {
                    failure_class: row.get(0)?,
                    count: row.get::<_, i64>(1)? as usize,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let total_count: usize = group_counts.iter().map(|group| group.count).sum();
        if total_count == 0 {
            return Ok(DeadLetterSummary {
                groups: Vec::new(),
                total_count,
            });
        }

        let mut item_stmt = self.conn.prepare(
            "SELECT task_id,
                    json_extract(source_metadata_json, '$.artist') AS artist,
                    json_extract(source_metadata_json, '$.title') AS title,
                    json_extract(source_metadata_json, '$.album') AS album,
                    provider,
                    updated_at,
                    request_json,
                    request_signature
             FROM director_task_history
             WHERE disposition IN ('Failed', 'Cancelled')
               AND COALESCE(failure_class, 'provider_exhausted') = ?1
             ORDER BY updated_at DESC
             LIMIT ?2",
        )?;

        let recent_limit = i64::try_from(recent_limit.max(1)).unwrap_or(5);
        let mut groups = Vec::with_capacity(group_counts.len());

        for group in &group_counts {
            let recent_items = item_stmt
                .query_map(params![group.failure_class, recent_limit], |row| {
                    Ok(DeadLetterItem {
                        task_id: row.get(0)?,
                        artist: row.get(1)?,
                        title: row.get(2)?,
                        album: row.get(3)?,
                        provider: row.get(4)?,
                        failed_at: row.get(5)?,
                        request_json: row.get(6)?,
                        request_signature: row.get(7)?,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;

            let (label, suggested_fix) = dead_letter_label_and_fix(&group.failure_class);
            groups.push(DeadLetterGroup {
                failure_class: group.failure_class.clone(),
                label: label.to_string(),
                suggested_fix: suggested_fix.to_string(),
                count: group.count,
                recent_items,
            });
        }

        Ok(DeadLetterSummary { groups, total_count })
    }

    /// Insert or update a single row in spotify_album_history.
    /// If the row already exists, only updates total_ms/play_count if the new values are higher.
    pub fn upsert_spotify_album_history(
        &self,
        artist: &str,
        album: &str,
        total_ms: i64,
        play_count: i64,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO spotify_album_history (artist, album, total_ms, play_count)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(artist, album) DO UPDATE SET
               total_ms   = MAX(total_ms,   excluded.total_ms),
               play_count = MAX(play_count, excluded.play_count)",
            params![artist, album, total_ms, play_count],
        )?;
        Ok(())
    }

    // ── Playlists ─────────────────────────────────────────────────────────────

    pub fn get_playlists(&self) -> Result<Vec<Playlist>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT p.id, p.name, p.description, p.created_at, COUNT(pi.id)
            FROM playlists p LEFT JOIN playlist_items pi ON pi.playlist_id = p.id
            GROUP BY p.id ORDER BY p.name COLLATE NOCASE
        ",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Playlist {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                created_at: row.get(3)?,
                track_count: row.get::<_, i64>(4)? as usize,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn get_playlist_items(&self, playlist_id: i64) -> Result<Vec<PlaylistItem>> {
        let mut stmt = self.conn.prepare(
            "
            SELECT pi.id, pi.playlist_id, pi.track_id, pi.position,
                   t.id,t.path,t.title,t.artist,t.album,t.album_artist,t.track_number,
                   t.disc_number,t.year,t.duration_secs,t.sample_rate,t.bit_depth,
                   t.bitrate_kbps,t.format,t.file_size,t.cover_art_path,t.isrc,
                   t.musicbrainz_recording_id,t.musicbrainz_release_id,t.canonical_artist_id,
                   t.canonical_release_id,t.quality_tier,t.content_hash,t.added_at
            FROM playlist_items pi JOIN tracks t ON pi.track_id = t.id
            WHERE pi.playlist_id = ?1 ORDER BY pi.position
        ",
        )?;
        let rows = stmt.query_map(params![playlist_id], |row| {
            Ok(PlaylistItem {
                id: row.get(0)?,
                playlist_id: row.get(1)?,
                track_id: row.get(2)?,
                position: row.get(3)?,
                track: Some(Track {
                    id: row.get(4)?,
                    path: row.get(5)?,
                    title: row.get(6)?,
                    artist: row.get(7)?,
                    album: row.get(8)?,
                    album_artist: row.get(9)?,
                    track_number: row.get(10)?,
                    disc_number: row.get(11)?,
                    year: row.get(12)?,
                    duration_secs: row.get(13)?,
                    sample_rate: row.get::<_, Option<i64>>(14)?.map(|v| v as u32),
                    bit_depth: row.get::<_, Option<i64>>(15)?.map(|v| v as u32),
                    bitrate_kbps: row.get::<_, Option<i64>>(16)?.map(|v| v as u32),
                    format: row.get(17)?,
                    file_size: row.get::<_, i64>(18)? as u64,
                    cover_art_path: row.get(19)?,
                    isrc: row.get(20)?,
                    musicbrainz_recording_id: row.get(21)?,
                    musicbrainz_release_id: row.get(22)?,
                    canonical_artist_id: row.get(23)?,
                    canonical_release_id: row.get(24)?,
                    quality_tier: row.get(25)?,
                    content_hash: row.get(26)?,
                    added_at: row.get(27).unwrap_or_default(),
                }),
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn create_playlist(
        &self,
        name: &str,
        description: Option<&str>,
        track_ids: &[i64],
    ) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO playlists (name, description) VALUES (?1, ?2)",
            params![name, description],
        )?;
        let id = self.conn.last_insert_rowid();
        for (pos, tid) in track_ids.iter().enumerate() {
            self.conn.execute(
                "INSERT INTO playlist_items (playlist_id, track_id, position) VALUES (?1,?2,?3)",
                params![id, tid, pos as i64],
            )?;
        }
        Ok(id)
    }

    pub fn replace_playlist_tracks(&self, playlist_id: i64, track_ids: &[i64]) -> Result<()> {
        self.conn.execute(
            "DELETE FROM playlist_items WHERE playlist_id = ?1",
            params![playlist_id],
        )?;
        for (pos, tid) in track_ids.iter().enumerate() {
            self.conn.execute(
                "INSERT INTO playlist_items (playlist_id, track_id, position) VALUES (?1,?2,?3)",
                params![playlist_id, tid, pos as i64],
            )?;
        }
        Ok(())
    }

    pub fn delete_playlist(&self, playlist_id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM playlists WHERE id = ?1", params![playlist_id])?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct PendingDirectorTask {
    pub task: TrackTask,
    pub strategy: String,
    pub progress: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalDirectorTaskUpdate {
    pub disposition: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredDirectorCandidateItem {
    pub provider_id: String,
    pub provider_candidate_id: String,
    pub outcome: String,
    pub rejection_reason: Option<String>,
    pub is_selected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredProviderMemory {
    pub provider_id: String,
    pub last_outcome: String,
    pub failure_class: String,
    pub retryable: bool,
    pub candidate_count: usize,
    pub backoff_until: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredProviderResponseCache {
    pub provider_id: String,
    pub outcome: String,
    pub candidate_count: usize,
    pub response_json: String,
    pub failure_class: Option<String>,
    pub backoff_until: Option<String>,
    pub retention_class: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredCandidateSetSummary {
    pub task_id: String,
    pub request_signature: Option<String>,
    pub request_strategy: Option<String>,
    pub disposition: String,
    pub selected_provider: Option<String>,
    pub candidate_count: usize,
    pub provider_count: usize,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredProviderSearchRecord {
    pub provider_id: String,
    pub provider_display_name: String,
    pub provider_trust_rank: i32,
    pub provider_order_index: usize,
    pub outcome: String,
    pub candidate_count: usize,
    pub error: Option<String>,
    pub retryable: bool,
    pub recorded_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeadLetterItem {
    pub task_id: String,
    pub artist: Option<String>,
    pub title: Option<String>,
    pub album: Option<String>,
    pub provider: Option<String>,
    pub failed_at: String,
    pub request_json: Option<String>,
    pub request_signature: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeadLetterGroup {
    pub failure_class: String,
    pub label: String,
    pub suggested_fix: String,
    pub count: usize,
    pub recent_items: Vec<DeadLetterItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeadLetterSummary {
    pub groups: Vec<DeadLetterGroup>,
    pub total_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredIdentityResolutionEvidence {
    pub task_id: String,
    pub request_signature: Option<String>,
    pub entity_type: String,
    pub entity_key: String,
    pub source_name: String,
    pub evidence_type: String,
    pub canonical_artist_id: Option<i64>,
    pub canonical_release_id: Option<i64>,
    pub musicbrainz_recording_id: Option<String>,
    pub musicbrainz_release_id: Option<String>,
    pub isrc: Option<String>,
    pub confidence: Option<f64>,
    pub raw_json: String,
    pub retention_class: String,
    pub recorded_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredSourceAlias {
    pub entity_type: String,
    pub entity_key: String,
    pub source_name: String,
    pub source_key: String,
    pub source_value: String,
    pub confidence: Option<f64>,
    pub raw_json: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
struct ProviderMemorySummary {
    last_outcome: String,
    failure_class: String,
    error: Option<String>,
    retryable: bool,
    candidate_count: usize,
}

pub fn director_request_signature(task: &TrackTask) -> String {
    let target = &task.target;
    let duration = target
        .duration_secs
        .map(|value| format!("{value:.3}"))
        .unwrap_or_default();
    [
        "track".to_string(),
        normalize_signature_text(target.spotify_track_id.as_deref().unwrap_or_default()),
        normalize_signature_text(target.source_album_id.as_deref().unwrap_or_default()),
        normalize_signature_text(target.source_artist_id.as_deref().unwrap_or_default()),
        normalize_signature_text(&target.artist),
        normalize_signature_text(&target.title),
        normalize_signature_text(target.album.as_deref().unwrap_or_default()),
        normalize_signature_text(target.album_artist.as_deref().unwrap_or_default()),
        target
            .track_number
            .map(|value| value.to_string())
            .unwrap_or_default(),
        target
            .disc_number
            .map(|value| value.to_string())
            .unwrap_or_default(),
        target
            .year
            .map(|value| value.to_string())
            .unwrap_or_default(),
        duration,
        normalize_signature_text(target.isrc.as_deref().unwrap_or_default()),
        normalize_signature_text(
            target
                .musicbrainz_recording_id
                .as_deref()
                .unwrap_or_default(),
        ),
        normalize_signature_text(
            target
                .musicbrainz_release_group_id
                .as_deref()
                .unwrap_or_default(),
        ),
        normalize_signature_text(target.musicbrainz_release_id.as_deref().unwrap_or_default()),
        target
            .canonical_artist_id
            .map(|value| value.to_string())
            .unwrap_or_default(),
        target
            .canonical_release_id
            .map(|value| value.to_string())
            .unwrap_or_default(),
    ]
    .join("|")
}

fn normalize_signature_text(value: &str) -> String {
    value
        .trim()
        .to_ascii_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn summarize_provider_memory(
    _provider_id: &str,
    provider_searches: &[&crate::director::models::ProviderSearchRecord],
    provider_candidates: &[&crate::director::models::CandidateRecord],
) -> Option<ProviderMemorySummary> {
    if provider_candidates.iter().any(|record| {
        matches!(
            record.outcome.as_str(),
            "valid_candidate" | "selected_immediate"
        )
    }) {
        return None;
    }

    if let Some(search) = provider_searches.iter().rev().find(|record| {
        matches!(
            record.outcome.as_str(),
            "no_candidates" | "search_error" | "metadata_only" | "busy" | "skipped_health_down"
        )
    }) {
        return Some(ProviderMemorySummary {
            last_outcome: search.outcome.clone(),
            failure_class: match search.outcome.as_str() {
                "no_candidates" => "no_result".to_string(),
                "metadata_only" => "unsupported".to_string(),
                "busy" => "provider_busy".to_string(),
                "skipped_health_down" => "provider_unhealthy".to_string(),
                _ => classify_provider_error_text(search.error.as_deref())
                    .unwrap_or_else(|| "search_error".to_string()),
            },
            error: search.error.clone(),
            retryable: search.retryable,
            candidate_count: search.candidate_count,
        });
    }

    if provider_candidates.is_empty() {
        return None;
    }

    let last = provider_candidates.last()?;
    Some(ProviderMemorySummary {
        last_outcome: "no_usable_candidates".to_string(),
        failure_class: match last.outcome.as_str() {
            "validation_failed" => "validation_failed".to_string(),
            "rejected_non_lossless" => "policy_rejected".to_string(),
            "acquire_failed" => "acquire_failed".to_string(),
            other => other.to_string(),
        },
        error: last.rejection_reason.clone(),
        retryable: false,
        candidate_count: provider_candidates.len(),
    })
}

fn classify_provider_error_text(error: Option<&str>) -> Option<String> {
    let error = error?.to_ascii_lowercase();
    if error.contains("auth failed") {
        return Some("auth_failed".to_string());
    }
    if error.contains("too_many_requests")
        || error.contains("rate limited")
        || error.contains("429")
    {
        return Some("rate_limited".to_string());
    }
    if error.contains("provider busy") || error.contains("cooling down") {
        return Some("provider_busy".to_string());
    }
    if error.contains("temporary outage") || error.contains("provider health down") {
        return Some("provider_unhealthy".to_string());
    }
    None
}

fn derive_result_provider(result: &DirectorTaskResult) -> Option<String> {
    result
        .finalized
        .as_ref()
        .map(|finalized| finalized.provenance.selected_provider.clone())
        .or_else(|| {
            result
                .provider_searches
                .iter()
                .rev()
                .find(|record| record.error.is_some() || record.outcome != "candidates_found")
                .map(|record| record.provider_id.clone())
        })
        .or_else(|| {
            result
                .attempts
                .last()
                .map(|attempt| attempt.provider_id.clone())
        })
}

fn classify_failure(result: &DirectorTaskResult) -> Option<String> {
    if !matches!(
        result.disposition,
        crate::director::models::FinalizedTrackDisposition::Failed
            | crate::director::models::FinalizedTrackDisposition::Cancelled
    ) {
        return None;
    }

    if result
        .attempts
        .iter()
        .any(|attempt| attempt.outcome.to_ascii_lowercase().contains("auth failed"))
    {
        return Some("auth_failed".to_string());
    }
    if result.attempts.iter().any(|attempt| {
        let outcome = attempt.outcome.to_ascii_lowercase();
        outcome.contains("too_many_requests")
            || outcome.contains("rate limited")
            || outcome.contains("429")
    }) {
        return Some("rate_limited".to_string());
    }
    if result
        .provider_searches
        .iter()
        .any(|record| record.outcome.contains("cooldown") || record.outcome == "busy")
    {
        return Some("provider_busy".to_string());
    }
    if result
        .candidate_records
        .iter()
        .any(|record| record.outcome == "validation_failed")
    {
        return Some("validation_failed".to_string());
    }
    if result
        .provider_searches
        .iter()
        .any(|record| record.outcome == "metadata_only")
    {
        return Some("metadata_only".to_string());
    }
    Some("provider_exhausted".to_string())
}

fn provider_cache_outcome(
    disposition: &crate::director::models::FinalizedTrackDisposition,
    provider_searches: &[crate::director::models::ProviderSearchRecord],
    provider_candidates: &[crate::director::models::CandidateRecord],
) -> String {
    if matches!(
        disposition,
        crate::director::models::FinalizedTrackDisposition::Finalized
            | crate::director::models::FinalizedTrackDisposition::AlreadyPresent
            | crate::director::models::FinalizedTrackDisposition::MetadataOnly
    ) && provider_candidates.iter().any(|record| {
        matches!(
            record.outcome.as_str(),
            "valid_candidate" | "selected_immediate"
        )
    }) {
        return "usable_candidate".to_string();
    }

    provider_searches
        .last()
        .map(|record| record.outcome.clone())
        .or_else(|| {
            provider_candidates
                .last()
                .map(|record| record.outcome.clone())
        })
        .unwrap_or_else(|| "unknown".to_string())
}

fn dead_letter_label_and_fix(failure_class: &str) -> (&'static str, &'static str) {
    match failure_class {
        "auth_failed" => (
            "Authentication failed",
            "Check provider credentials in Settings",
        ),
        "rate_limited" => (
            "Rate limited",
            "Provider is throttling requests - wait and retry",
        ),
        "validation_failed" => (
            "File failed validation",
            "Candidate audio was corrupt or mismatched",
        ),
        "provider_busy" => (
            "Provider busy",
            "Provider was at capacity - retry automatically",
        ),
        "metadata_only" => (
            "No downloadable file found",
            "Provider returned metadata but no audio",
        ),
        _ => (
            "All providers exhausted",
            "No provider had a matching file",
        ),
    }
}

// ── Canonical Identity ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalArtist {
    pub id: i64,
    pub name: String,
    pub normalized_name: String,
    pub musicbrainz_id: Option<String>,
    pub spotify_id: Option<String>,
    pub discogs_id: Option<String>,
    pub sort_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalRelease {
    pub id: i64,
    pub canonical_artist_id: i64,
    pub title: String,
    pub normalized_title: String,
    pub release_group_mbid: Option<String>,
    pub release_mbid: Option<String>,
    pub spotify_id: Option<String>,
    pub discogs_id: Option<String>,
    pub release_type: Option<String>,
    pub year: Option<i32>,
    pub track_count: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalRecording {
    pub id: i64,
    pub canonical_artist_id: Option<i64>,
    pub canonical_release_id: Option<i64>,
    pub title: String,
    pub normalized_title: String,
    pub musicbrainz_recording_id: Option<String>,
    pub isrc: Option<String>,
    pub track_number: Option<i32>,
    pub disc_number: Option<i32>,
    pub duration_secs: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcquisitionRequest {
    pub id: i64,
    pub scope: String,
    pub artist: String,
    pub album: Option<String>,
    pub strategy: String,
    pub quality_floor: Option<String>,
    pub exclude_providers: Option<String>,
    pub edition_policy: Option<String>,
    pub canonical_artist_id: Option<i64>,
    pub canonical_release_id: Option<i64>,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateReviewItem {
    pub task_id: String,
    pub provider_id: String,
    pub provider_display_name: String,
    pub provider_trust_rank: i32,
    pub provider_candidate_id: String,
    pub outcome: String,
    pub rejection_reason: Option<String>,
    pub is_selected: bool,
    pub score_total: Option<i32>,
    pub candidate_json: String,
    pub validation_json: Option<String>,
    pub score_reason_json: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskExecutionSummary {
    pub task_id: String,
    pub disposition: String,
    pub provider: Option<String>,
    pub failure_class: Option<String>,
    pub final_path: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustLedgerOperationEvent {
    pub operation_id: String,
    pub module: String,
    pub phase: String,
    pub event_type: String,
    pub timestamp: Option<String>,
    pub event_data: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustLedgerGatekeeperAudit {
    pub operation_id: String,
    pub timestamp: String,
    pub file_path: String,
    pub decision: String,
    pub desired_track_id: Option<i64>,
    pub matched_local_file_id: Option<i64>,
    pub duration_ms: i64,
    pub notes: String,
}

impl Db {
    // ── Canonical Artists ─────────────────────────────────────────────────

    pub fn upsert_canonical_artist(
        &self,
        name: &str,
        musicbrainz_id: Option<&str>,
        spotify_id: Option<&str>,
        discogs_id: Option<&str>,
        sort_name: Option<&str>,
    ) -> Result<i64> {
        let normalized = normalize_canonical(name);

        // Try to find by MB ID first, then spotify, then normalized name
        let existing_id = if let Some(mbid) = musicbrainz_id {
            self.conn
                .query_row(
                    "SELECT id FROM canonical_artists WHERE musicbrainz_id = ?1",
                    params![mbid],
                    |row| row.get::<_, i64>(0),
                )
                .ok()
        } else if let Some(sid) = spotify_id {
            self.conn
                .query_row(
                    "SELECT id FROM canonical_artists WHERE spotify_id = ?1",
                    params![sid],
                    |row| row.get::<_, i64>(0),
                )
                .ok()
        } else {
            self.conn
                .query_row(
                    "SELECT id FROM canonical_artists WHERE normalized_name = ?1",
                    params![&normalized],
                    |row| row.get::<_, i64>(0),
                )
                .ok()
        };

        if let Some(id) = existing_id {
            self.conn.execute(
                "UPDATE canonical_artists SET
                    name = COALESCE(?2, name),
                    musicbrainz_id = COALESCE(?3, musicbrainz_id),
                    spotify_id = COALESCE(?4, spotify_id),
                    discogs_id = COALESCE(?5, discogs_id),
                    sort_name = COALESCE(?6, sort_name),
                    updated_at = datetime('now')
                WHERE id = ?1",
                params![id, name, musicbrainz_id, spotify_id, discogs_id, sort_name],
            )?;
            Ok(id)
        } else {
            self.conn.execute(
                "INSERT INTO canonical_artists
                    (name, normalized_name, musicbrainz_id, spotify_id, discogs_id, sort_name)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    name,
                    &normalized,
                    musicbrainz_id,
                    spotify_id,
                    discogs_id,
                    sort_name
                ],
            )?;
            Ok(self.conn.last_insert_rowid())
        }
    }

    pub fn get_canonical_artist_by_name(&self, name: &str) -> Result<Option<CanonicalArtist>> {
        let normalized = normalize_canonical(name);
        let mut stmt = self.conn.prepare(
            "SELECT id, name, normalized_name, musicbrainz_id, spotify_id, discogs_id, sort_name
             FROM canonical_artists WHERE normalized_name = ?1",
        )?;
        let mut rows = stmt.query_map(params![&normalized], |row| {
            Ok(CanonicalArtist {
                id: row.get(0)?,
                name: row.get(1)?,
                normalized_name: row.get(2)?,
                musicbrainz_id: row.get(3)?,
                spotify_id: row.get(4)?,
                discogs_id: row.get(5)?,
                sort_name: row.get(6)?,
            })
        })?;
        Ok(rows.next().transpose()?)
    }

    pub fn list_canonical_artists(&self) -> Result<Vec<CanonicalArtist>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, normalized_name, musicbrainz_id, spotify_id, discogs_id, sort_name
             FROM canonical_artists
             ORDER BY id ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(CanonicalArtist {
                id: row.get(0)?,
                name: row.get(1)?,
                normalized_name: row.get(2)?,
                musicbrainz_id: row.get(3)?,
                spotify_id: row.get(4)?,
                discogs_id: row.get(5)?,
                sort_name: row.get(6)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    // ── Canonical Releases ───────────────────────────────────────────────

    pub fn upsert_canonical_release(
        &self,
        canonical_artist_id: i64,
        title: &str,
        release_mbid: Option<&str>,
        release_group_mbid: Option<&str>,
        spotify_id: Option<&str>,
        release_type: Option<&str>,
        year: Option<i32>,
        track_count: Option<i32>,
    ) -> Result<i64> {
        let normalized = normalize_canonical(title);

        let existing_id = if let Some(mbid) = release_mbid {
            self.conn
                .query_row(
                    "SELECT id FROM canonical_releases WHERE release_mbid = ?1",
                    params![mbid],
                    |row| row.get::<_, i64>(0),
                )
                .ok()
        } else {
            self.conn
                .query_row(
                    "SELECT id FROM canonical_releases WHERE canonical_artist_id = ?1 AND normalized_title = ?2",
                    params![canonical_artist_id, &normalized],
                    |row| row.get::<_, i64>(0),
                )
                .ok()
        };

        if let Some(id) = existing_id {
            self.conn.execute(
                "UPDATE canonical_releases SET
                    title = COALESCE(?2, title),
                    release_group_mbid = COALESCE(?3, release_group_mbid),
                    release_mbid = COALESCE(?4, release_mbid),
                    spotify_id = COALESCE(?5, spotify_id),
                    release_type = COALESCE(?6, release_type),
                    year = COALESCE(?7, year),
                    track_count = COALESCE(?8, track_count),
                    updated_at = datetime('now')
                WHERE id = ?1",
                params![
                    id,
                    title,
                    release_group_mbid,
                    release_mbid,
                    spotify_id,
                    release_type,
                    year,
                    track_count
                ],
            )?;
            Ok(id)
        } else {
            self.conn.execute(
                "INSERT INTO canonical_releases
                    (canonical_artist_id, title, normalized_title, release_group_mbid, release_mbid,
                     spotify_id, release_type, year, track_count)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    canonical_artist_id,
                    title,
                    &normalized,
                    release_group_mbid,
                    release_mbid,
                    spotify_id,
                    release_type,
                    year,
                    track_count
                ],
            )?;
            Ok(self.conn.last_insert_rowid())
        }
    }

    pub fn list_canonical_releases(&self) -> Result<Vec<CanonicalRelease>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, canonical_artist_id, title, normalized_title, release_group_mbid,
                    release_mbid, spotify_id, discogs_id, release_type, year, track_count
             FROM canonical_releases
             ORDER BY canonical_artist_id ASC, id ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(CanonicalRelease {
                id: row.get(0)?,
                canonical_artist_id: row.get(1)?,
                title: row.get(2)?,
                normalized_title: row.get(3)?,
                release_group_mbid: row.get(4)?,
                release_mbid: row.get(5)?,
                spotify_id: row.get(6)?,
                discogs_id: row.get(7)?,
                release_type: row.get(8)?,
                year: row.get(9)?,
                track_count: row.get(10)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn upsert_canonical_recording(
        &self,
        title: &str,
        canonical_artist_id: Option<i64>,
        canonical_release_id: Option<i64>,
        musicbrainz_recording_id: Option<&str>,
        isrc: Option<&str>,
        track_number: Option<i32>,
        disc_number: Option<i32>,
        duration_secs: Option<f64>,
    ) -> Result<i64> {
        let normalized = normalize_canonical(title);

        let existing_id = if let Some(mbid) = musicbrainz_recording_id {
            self.conn
                .query_row(
                    "SELECT id FROM canonical_recordings WHERE musicbrainz_recording_id = ?1",
                    params![mbid],
                    |row| row.get::<_, i64>(0),
                )
                .ok()
        } else {
            self.conn
                .query_row(
                    "SELECT id FROM canonical_recordings
                     WHERE canonical_artist_id IS ?1
                       AND canonical_release_id IS ?2
                       AND normalized_title = ?3
                       AND COALESCE(track_number, 0) = COALESCE(?4, 0)
                       AND COALESCE(disc_number, 0) = COALESCE(?5, 0)",
                    params![
                        canonical_artist_id,
                        canonical_release_id,
                        &normalized,
                        track_number,
                        disc_number,
                    ],
                    |row| row.get::<_, i64>(0),
                )
                .ok()
        };

        if let Some(id) = existing_id {
            self.conn.execute(
                "UPDATE canonical_recordings SET
                    title = COALESCE(?2, title),
                    canonical_artist_id = COALESCE(?3, canonical_artist_id),
                    canonical_release_id = COALESCE(?4, canonical_release_id),
                    musicbrainz_recording_id = COALESCE(?5, musicbrainz_recording_id),
                    isrc = COALESCE(?6, isrc),
                    track_number = COALESCE(?7, track_number),
                    disc_number = COALESCE(?8, disc_number),
                    duration_secs = COALESCE(?9, duration_secs),
                    updated_at = datetime('now')
                 WHERE id = ?1",
                params![
                    id,
                    title,
                    canonical_artist_id,
                    canonical_release_id,
                    musicbrainz_recording_id,
                    isrc,
                    track_number,
                    disc_number,
                    duration_secs,
                ],
            )?;
            Ok(id)
        } else {
            self.conn.execute(
                "INSERT INTO canonical_recordings
                    (canonical_artist_id, canonical_release_id, title, normalized_title,
                     musicbrainz_recording_id, isrc, track_number, disc_number, duration_secs)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    canonical_artist_id,
                    canonical_release_id,
                    title,
                    &normalized,
                    musicbrainz_recording_id,
                    isrc,
                    track_number,
                    disc_number,
                    duration_secs,
                ],
            )?;
            Ok(self.conn.last_insert_rowid())
        }
    }

    pub fn list_canonical_recordings(&self) -> Result<Vec<CanonicalRecording>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, canonical_artist_id, canonical_release_id, title, normalized_title,
                    musicbrainz_recording_id, isrc, track_number, disc_number, duration_secs
             FROM canonical_recordings
             ORDER BY COALESCE(canonical_release_id, 0) ASC, COALESCE(canonical_artist_id, 0) ASC, id ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(CanonicalRecording {
                id: row.get(0)?,
                canonical_artist_id: row.get(1)?,
                canonical_release_id: row.get(2)?,
                title: row.get(3)?,
                normalized_title: row.get(4)?,
                musicbrainz_recording_id: row.get(5)?,
                isrc: row.get(6)?,
                track_number: row.get(7)?,
                disc_number: row.get(8)?,
                duration_secs: row.get(9)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    fn upsert_source_alias(
        &self,
        entity_type: &str,
        entity_key: &str,
        source_name: &str,
        source_key: &str,
        source_value: &str,
        confidence: Option<f64>,
        raw_json: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO source_aliases
                (entity_type, entity_key, source_name, source_key, source_value, confidence, raw_json, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, datetime('now'))
             ON CONFLICT(entity_type, entity_key, source_name, source_key, source_value) DO UPDATE SET
                confidence = COALESCE(excluded.confidence, source_aliases.confidence),
                raw_json = COALESCE(excluded.raw_json, source_aliases.raw_json),
                updated_at = datetime('now')",
            params![
                entity_type,
                entity_key,
                source_name,
                source_key,
                source_value,
                confidence,
                raw_json,
            ],
        )?;
        Ok(())
    }

    // ── Acquisition Requests ─────────────────────────────────────────────

    pub fn create_acquisition_request(
        &self,
        scope: &str,
        artist: &str,
        album: Option<&str>,
        strategy: &str,
        quality_floor: Option<&str>,
        exclude_providers: Option<&str>,
        edition_policy: Option<&str>,
        canonical_artist_id: Option<i64>,
        canonical_release_id: Option<i64>,
    ) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO acquisition_requests
                (scope, artist, album, strategy, quality_floor, exclude_providers,
                 edition_policy, canonical_artist_id, canonical_release_id)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                scope,
                artist,
                album,
                strategy,
                quality_floor,
                exclude_providers,
                edition_policy,
                canonical_artist_id,
                canonical_release_id
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn get_acquisition_requests(
        &self,
        status: &str,
        limit: usize,
    ) -> Result<Vec<AcquisitionRequest>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, scope, artist, album, strategy, quality_floor, exclude_providers,
                    edition_policy, canonical_artist_id, canonical_release_id, status, created_at
             FROM acquisition_requests WHERE status = ?1
             ORDER BY created_at DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![status, limit as i64], |row| {
            Ok(AcquisitionRequest {
                id: row.get(0)?,
                scope: row.get(1)?,
                artist: row.get(2)?,
                album: row.get(3)?,
                strategy: row.get(4)?,
                quality_floor: row.get(5)?,
                exclude_providers: row.get(6)?,
                edition_policy: row.get(7)?,
                canonical_artist_id: row.get(8)?,
                canonical_release_id: row.get(9)?,
                status: row.get(10)?,
                created_at: row.get(11)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn update_acquisition_request_status(&self, id: i64, status: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE acquisition_requests SET status = ?2, updated_at = datetime('now') WHERE id = ?1",
            params![id, status],
        )?;
        Ok(())
    }

    // ── Candidate Review ─────────────────────────────────────────────────

    pub fn get_candidate_review(&self, task_id: &str) -> Result<Vec<CandidateReviewItem>> {
        let mut stmt = self.conn.prepare(
            "SELECT task_id, provider_id, provider_display_name, provider_trust_rank,
                    provider_candidate_id, outcome, rejection_reason, is_selected,
                    score_total, candidate_json, validation_json, score_reason_json
             FROM director_candidate_items
             WHERE task_id = ?1
             ORDER BY provider_order_index, search_rank",
        )?;
        let rows = stmt.query_map(params![task_id], |row| {
            Ok(CandidateReviewItem {
                task_id: row.get(0)?,
                provider_id: row.get(1)?,
                provider_display_name: row.get(2)?,
                provider_trust_rank: row.get(3)?,
                provider_candidate_id: row.get(4)?,
                outcome: row.get(5)?,
                rejection_reason: row.get(6)?,
                is_selected: row.get::<_, i64>(7)? != 0,
                score_total: row.get(8)?,
                candidate_json: row.get(9)?,
                validation_json: row.get(10)?,
                score_reason_json: row.get(11)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn get_task_provenance(&self, task_id: &str) -> Result<Option<String>> {
        let result = self.conn.query_row(
            "SELECT result_json FROM director_task_history WHERE task_id = ?1",
            params![task_id],
            |row| row.get::<_, String>(0),
        );
        match result {
            Ok(json) => Ok(Some(json)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn get_recent_task_results(
        &self,
        limit: usize,
    ) -> Result<Vec<(String, String, String, Option<String>)>> {
        let mut stmt = self.conn.prepare(
            "SELECT task_id, disposition, COALESCE(provider, ''), error
             FROM director_task_history
             ORDER BY updated_at DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn get_task_execution_summary(
        &self,
        task_id: &str,
    ) -> Result<Option<TaskExecutionSummary>> {
        let result = self.conn.query_row(
            "SELECT task_id, disposition, provider, failure_class, final_path, updated_at
             FROM director_task_history
             WHERE task_id = ?1
             LIMIT 1",
            params![task_id],
            |row| {
                Ok(TaskExecutionSummary {
                    task_id: row.get(0)?,
                    disposition: row.get(1)?,
                    provider: row.get(2)?,
                    failure_class: row.get(3)?,
                    final_path: row.get(4)?,
                    updated_at: row.get(5)?,
                })
            },
        );

        match result {
            Ok(summary) => Ok(Some(summary)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    pub fn get_operation_events_for_context(
        &self,
        file_path: Option<&str>,
        desired_track_id: Option<i64>,
    ) -> Result<Vec<TrustLedgerOperationEvent>> {
        match (file_path, desired_track_id) {
            (Some(file_path), Some(desired_track_id)) => {
                let file_pattern = format!("%{file_path}%");
                let desired_pattern = format!("%\"desired_track_id\":{desired_track_id}%");
                let mut stmt = self.conn.prepare(
                    r#"
                    SELECT oe.operation_id, ol.module, ol.phase, oe.event_type, oe.timestamp, oe.event_data
                    FROM operation_events oe
                    JOIN operation_log ol ON oe.operation_id = ol.operation_id
                    WHERE oe.event_data LIKE ?1 OR oe.event_data LIKE ?2
                    ORDER BY oe.event_id ASC
                    "#,
                )?;
                let rows = stmt.query_map(params![file_pattern, desired_pattern], |row| {
                    Ok(TrustLedgerOperationEvent {
                        operation_id: row.get(0)?,
                        module: row.get(1)?,
                        phase: row.get(2)?,
                        event_type: row.get(3)?,
                        timestamp: row.get(4)?,
                        event_data: row.get(5)?,
                    })
                })?;
                Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
            }
            (Some(file_path), None) => {
                let file_pattern = format!("%{file_path}%");
                let mut stmt = self.conn.prepare(
                    r#"
                    SELECT oe.operation_id, ol.module, ol.phase, oe.event_type, oe.timestamp, oe.event_data
                    FROM operation_events oe
                    JOIN operation_log ol ON oe.operation_id = ol.operation_id
                    WHERE oe.event_data LIKE ?1
                    ORDER BY oe.event_id ASC
                    "#,
                )?;
                let rows = stmt.query_map(params![file_pattern], |row| {
                    Ok(TrustLedgerOperationEvent {
                        operation_id: row.get(0)?,
                        module: row.get(1)?,
                        phase: row.get(2)?,
                        event_type: row.get(3)?,
                        timestamp: row.get(4)?,
                        event_data: row.get(5)?,
                    })
                })?;
                Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
            }
            (None, Some(desired_track_id)) => {
                let desired_pattern = format!("%\"desired_track_id\":{desired_track_id}%");
                let mut stmt = self.conn.prepare(
                    r#"
                    SELECT oe.operation_id, ol.module, ol.phase, oe.event_type, oe.timestamp, oe.event_data
                    FROM operation_events oe
                    JOIN operation_log ol ON oe.operation_id = ol.operation_id
                    WHERE oe.event_data LIKE ?1
                    ORDER BY oe.event_id ASC
                    "#,
                )?;
                let rows = stmt.query_map(params![desired_pattern], |row| {
                    Ok(TrustLedgerOperationEvent {
                        operation_id: row.get(0)?,
                        module: row.get(1)?,
                        phase: row.get(2)?,
                        event_type: row.get(3)?,
                        timestamp: row.get(4)?,
                        event_data: row.get(5)?,
                    })
                })?;
                Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
            }
            (None, None) => Ok(Vec::new()),
        }
    }

    pub fn get_gatekeeper_audit_for_context(
        &self,
        file_path: Option<&str>,
        desired_track_id: Option<i64>,
    ) -> Result<Vec<TrustLedgerGatekeeperAudit>> {
        match (file_path, desired_track_id) {
            (Some(file_path), Some(desired_track_id)) => {
                let mut stmt = self.conn.prepare(
                    r#"
                    SELECT operation_id, timestamp, file_path, decision, desired_track_id,
                           matched_local_file_id, duration_ms, notes
                    FROM gatekeeper_audit_log
                    WHERE file_path = ?1 OR desired_track_id = ?2
                    ORDER BY created_at ASC, id ASC
                    "#,
                )?;
                let rows = stmt.query_map(params![file_path, desired_track_id], |row| {
                    Ok(TrustLedgerGatekeeperAudit {
                        operation_id: row.get(0)?,
                        timestamp: row.get(1)?,
                        file_path: row.get(2)?,
                        decision: row.get(3)?,
                        desired_track_id: row.get(4)?,
                        matched_local_file_id: row.get(5)?,
                        duration_ms: row.get(6)?,
                        notes: row.get(7)?,
                    })
                })?;
                Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
            }
            (Some(file_path), None) => {
                let mut stmt = self.conn.prepare(
                    r#"
                    SELECT operation_id, timestamp, file_path, decision, desired_track_id,
                           matched_local_file_id, duration_ms, notes
                    FROM gatekeeper_audit_log
                    WHERE file_path = ?1
                    ORDER BY created_at ASC, id ASC
                    "#,
                )?;
                let rows = stmt.query_map(params![file_path], |row| {
                    Ok(TrustLedgerGatekeeperAudit {
                        operation_id: row.get(0)?,
                        timestamp: row.get(1)?,
                        file_path: row.get(2)?,
                        decision: row.get(3)?,
                        desired_track_id: row.get(4)?,
                        matched_local_file_id: row.get(5)?,
                        duration_ms: row.get(6)?,
                        notes: row.get(7)?,
                    })
                })?;
                Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
            }
            (None, Some(desired_track_id)) => {
                let mut stmt = self.conn.prepare(
                    r#"
                    SELECT operation_id, timestamp, file_path, decision, desired_track_id,
                           matched_local_file_id, duration_ms, notes
                    FROM gatekeeper_audit_log
                    WHERE desired_track_id = ?1
                    ORDER BY created_at ASC, id ASC
                    "#,
                )?;
                let rows = stmt.query_map(params![desired_track_id], |row| {
                    Ok(TrustLedgerGatekeeperAudit {
                        operation_id: row.get(0)?,
                        timestamp: row.get(1)?,
                        file_path: row.get(2)?,
                        decision: row.get(3)?,
                        desired_track_id: row.get(4)?,
                        matched_local_file_id: row.get(5)?,
                        duration_ms: row.get(6)?,
                        notes: row.get(7)?,
                    })
                })?;
                Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
            }
            (None, None) => Ok(Vec::new()),
        }
    }
}

fn normalize_canonical(value: &str) -> String {
    normalize_artist_identity(value)
}

fn stable_entity_id(parts: &[&str]) -> i64 {
    let mut hasher = blake3::Hasher::new();
    for part in parts {
        hasher.update(part.trim().to_ascii_lowercase().as_bytes());
        hasher.update(&[0x1f]);
    }

    let digest = hasher.finalize();
    let mut id_bytes = [0_u8; 8];
    id_bytes.copy_from_slice(&digest.as_bytes()[..8]);
    let raw = u64::from_le_bytes(id_bytes) & 0x7FFF_FFFF_FFFF_FFFF;
    if raw == 0 {
        1
    } else {
        raw as i64
    }
}

const EDITION_MARKERS: [(&str, &str); 12] = [
    ("deluxe", "deluxe"),
    ("expanded", "expanded"),
    ("anniversary", "anniversary"),
    ("collector", "collector"),
    ("special edition", "special_edition"),
    ("limited edition", "limited_edition"),
    ("tour edition", "tour_edition"),
    ("bonus", "bonus"),
    ("remaster", "remaster"),
    ("remastered", "remaster"),
    ("live", "live"),
    ("acoustic", "acoustic"),
];

fn detect_edition_markers(album: &str) -> Vec<String> {
    let normalized = album.to_ascii_lowercase();
    let mut markers = Vec::new();
    for (needle, marker) in EDITION_MARKERS {
        if normalized.contains(needle) && !markers.iter().any(|existing| existing == marker) {
            markers.push(marker.to_string());
        }
    }
    markers
}

fn classify_edition_bucket(album: &str, release_type: Option<&str>) -> Option<String> {
    let markers = detect_edition_markers(album);
    if markers.iter().any(|marker| marker == "live") {
        return Some("live".to_string());
    }
    if markers.iter().any(|marker| marker == "remaster") {
        return Some("remaster".to_string());
    }
    if !markers.is_empty() {
        return Some("edition_variant".to_string());
    }

    let release_type = release_type
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase())?;

    if release_type.contains("live") {
        Some("live".to_string())
    } else if release_type.contains("compilation") {
        Some("compilation".to_string())
    } else if release_type.contains("ep") {
        Some("ep".to_string())
    } else if release_type.contains("single") {
        Some("single".to_string())
    } else {
        Some("standard".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::director::models::{
        AcquisitionStrategy, NormalizedTrack, TrackTask, TrackTaskSource,
    };
    use crate::models::{SpotifyAlbumHistory, Track};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_db_path(name: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be valid")
            .as_nanos();
        std::env::temp_dir().join(format!("cassette-{name}-{nanos}.db"))
    }

    #[test]
    fn spotify_album_history_roundtrip_replaces_rows() {
        let db_path = temp_db_path("spotify-history");
        let db = Db::open(&db_path).expect("db should open");

        let rows = vec![
            SpotifyAlbumHistory {
                artist: "The National".to_string(),
                album: "Boxer".to_string(),
                total_ms: 300_000,
                play_count: 10,
                skip_count: 1,
                in_library: true,
                imported_at: String::new(),
            },
            SpotifyAlbumHistory {
                artist: "Interpol".to_string(),
                album: "Turn on the Bright Lights".to_string(),
                total_ms: 250_000,
                play_count: 8,
                skip_count: 0,
                in_library: false,
                imported_at: String::new(),
            },
        ];

        db.replace_spotify_album_history(&rows)
            .expect("history rows should be saved");
        let count = db
            .get_spotify_album_history_count()
            .expect("history count should be readable");
        assert_eq!(count, 2);

        db.replace_spotify_album_history(&rows[..1])
            .expect("history rows should be replaceable");
        let replaced_count = db
            .get_spotify_album_history_count()
            .expect("history count should be readable");
        assert_eq!(replaced_count, 1);
        let last_imported = db
            .get_spotify_album_history_last_imported_at()
            .expect("import timestamp should be readable");
        assert!(last_imported.is_some());

        let _ = fs::remove_file(db_path);
    }

    #[test]
    fn get_dead_letter_summary_returns_empty_for_new_db() {
        let db_path = temp_db_path("dead-letter-empty");
        let db = Db::open(&db_path).expect("db should open");

        let summary = db
            .get_dead_letter_summary(5)
            .expect("dead letter summary should load");
        assert_eq!(summary.total_count, 0);
        assert!(summary.groups.is_empty());

        let _ = fs::remove_file(db_path);
    }

    #[test]
    fn pending_director_task_roundtrip() {
        let db_path = temp_db_path("pending-director");
        let db = Db::open(&db_path).expect("db should open");

        let task = TrackTask {
            task_id: "task-123".to_string(),
            source: TrackTaskSource::Manual,
            desired_track_id: None,
            source_operation_id: None,
            target: NormalizedTrack {
                spotify_track_id: None,
                source_album_id: None,
                source_artist_id: None,
                source_playlist: None,
                artist: "Artist".to_string(),
                album_artist: Some("Artist".to_string()),
                title: "Song".to_string(),
                album: Some("Album".to_string()),
                track_number: Some(1),
                disc_number: Some(1),
                year: Some(2024),
                duration_secs: Some(42.0),
                isrc: None,
                musicbrainz_recording_id: None,
                musicbrainz_release_group_id: None,
                musicbrainz_release_id: None,
                canonical_artist_id: None,
                canonical_release_id: None,
            },
            strategy: AcquisitionStrategy::Standard,
        };

        db.upsert_director_pending_task(&task, "Queued")
            .expect("pending task should save");
        let tasks = db
            .get_pending_director_tasks()
            .expect("pending tasks should load");
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].task.task_id, "task-123");
        assert_eq!(tasks[0].progress, "Queued");

        db.delete_director_pending_task("task-123")
            .expect("pending task should delete");
        let tasks = db
            .get_pending_director_tasks()
            .expect("pending tasks should load");
        assert!(tasks.is_empty());

        let _ = fs::remove_file(db_path);
    }

    #[test]
    fn track_roundtrip_preserves_identity_fields() {
        let db_path = temp_db_path("track-identity-roundtrip");
        let db = Db::open(&db_path).expect("db should open");
        let canonical_artist_id = db
            .upsert_canonical_artist("Artist", Some("artist-mbid-1"), None, None, None)
            .expect("canonical artist should save");
        let canonical_release_id = db
            .upsert_canonical_release(
                canonical_artist_id,
                "Album",
                Some("release-mbid-1"),
                None,
                None,
                Some("album"),
                Some(2024),
                Some(1),
            )
            .expect("canonical release should save");

        let track = Track {
            id: 0,
            path: "C:\\Music\\Artist\\Album\\01 - Song.flac".to_string(),
            title: "Song".to_string(),
            artist: "Artist".to_string(),
            album: "Album".to_string(),
            album_artist: "Artist".to_string(),
            track_number: Some(1),
            disc_number: Some(1),
            year: Some(2024),
            duration_secs: 42.0,
            sample_rate: Some(48_000),
            bit_depth: Some(24),
            bitrate_kbps: Some(1000),
            format: "FLAC".to_string(),
            file_size: 12_345,
            cover_art_path: Some("C:\\Music\\Artist\\Album\\cover.jpg".to_string()),
            isrc: Some("US1234567890".to_string()),
            musicbrainz_recording_id: Some("mb-recording-1".to_string()),
            musicbrainz_release_id: Some("mb-release-1".to_string()),
            canonical_artist_id: Some(canonical_artist_id),
            canonical_release_id: Some(canonical_release_id),
            quality_tier: Some("lossless_preferred".to_string()),
            content_hash: Some("hash-abc".to_string()),
            added_at: String::new(),
        };

        db.upsert_track(&track).expect("track should save");
        let stored = db
            .get_track_by_path(&track.path)
            .expect("track should load")
            .expect("track row should exist");

        assert_eq!(stored.isrc.as_deref(), Some("US1234567890"));
        assert_eq!(
            stored.musicbrainz_recording_id.as_deref(),
            Some("mb-recording-1")
        );
        assert_eq!(
            stored.musicbrainz_release_id.as_deref(),
            Some("mb-release-1")
        );
        assert_eq!(stored.canonical_artist_id, Some(canonical_artist_id));
        assert_eq!(stored.canonical_release_id, Some(canonical_release_id));
        assert_eq!(stored.quality_tier.as_deref(), Some("lossless_preferred"));
        assert_eq!(stored.content_hash.as_deref(), Some("hash-abc"));

        let _ = fs::remove_file(db_path);
    }

    #[test]
    fn apply_track_path_updates_updates_sidecar_and_marks_conflicts_stale() {
        let db_path = temp_db_path("track-path-sync");
        let sidecar_path = temp_db_path("track-path-sidecar");
        let db = Db::open(&db_path).expect("db should open");

        let old_path = std::env::temp_dir()
            .join("cassette-old-song.flac")
            .to_string_lossy()
            .to_string();
        let new_path_buf = std::env::temp_dir().join(format!(
            "cassette-new-song-{}.flac",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time should be valid")
                .as_nanos()
        ));
        fs::write(&new_path_buf, b"audio").expect("new file should exist");
        let new_path = new_path_buf.to_string_lossy().to_string();

        let track = Track {
            id: 0,
            path: old_path.clone(),
            title: "Song".to_string(),
            artist: "Artist".to_string(),
            album: "Album".to_string(),
            album_artist: "Artist".to_string(),
            track_number: Some(1),
            disc_number: Some(1),
            year: Some(2024),
            duration_secs: 42.0,
            sample_rate: None,
            bit_depth: None,
            bitrate_kbps: None,
            format: "FLAC".to_string(),
            file_size: 4,
            cover_art_path: None,
            isrc: None,
            musicbrainz_recording_id: None,
            musicbrainz_release_id: None,
            canonical_artist_id: None,
            canonical_release_id: None,
            quality_tier: None,
            content_hash: None,
            added_at: String::new(),
        };
        db.upsert_track(&track).expect("track should save");
        let stored = db
            .get_track_by_path(&old_path)
            .expect("stored track should load")
            .expect("stored track should exist");

        let sidecar = Connection::open(&sidecar_path).expect("sidecar db should open");
        sidecar
            .execute_batch(
                "
                CREATE TABLE local_files (
                    id INTEGER PRIMARY KEY,
                    file_path TEXT UNIQUE NOT NULL,
                    file_name TEXT NOT NULL,
                    extension TEXT NOT NULL,
                    file_mtime_ms INTEGER,
                    integrity_status TEXT NOT NULL,
                    last_scanned_at TEXT,
                    updated_at TEXT DEFAULT CURRENT_TIMESTAMP
                );
                ",
            )
            .expect("sidecar schema should create");
        sidecar
            .execute(
                "INSERT INTO local_files (id, file_path, file_name, extension, integrity_status)
                 VALUES (1, ?1, 'old.flac', 'flac', 'readable')",
                params![old_path.clone()],
            )
            .expect("old path should insert");
        sidecar
            .execute(
                "INSERT INTO local_files (id, file_path, file_name, extension, integrity_status)
                 VALUES (2, ?1, 'conflict.flac', 'flac', 'readable')",
                params![new_path.clone()],
            )
            .expect("conflict path should insert");
        drop(sidecar);

        db.apply_track_path_updates(
            &sidecar_path,
            &[TrackPathUpdate {
                track_id: stored.id,
                old_path: old_path.clone(),
                new_path: new_path.clone(),
            }],
        )
        .expect("path updates should converge");

        let updated_track = db
            .get_track_by_id(stored.id)
            .expect("updated track should load")
            .expect("updated track should exist");
        assert_eq!(updated_track.path, new_path);

        let sidecar = Connection::open(&sidecar_path).expect("sidecar db should reopen");
        let synced_row: (String, String, String) = sidecar
            .query_row(
                "SELECT file_path, file_name, integrity_status FROM local_files WHERE id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("synced row should load");
        assert_eq!(synced_row.0, updated_track.path);
        assert_eq!(
            synced_row.1,
            new_path_buf
                .file_name()
                .expect("file name should exist")
                .to_string_lossy()
                .to_string()
        );
        assert_eq!(synced_row.2, "readable");

        let conflict_path: String = sidecar
            .query_row(
                "SELECT file_path FROM local_files WHERE id = 2",
                [],
                |row| row.get(0),
            )
            .expect("conflict row should load");
        assert!(conflict_path.contains(".stale-conflict-2"));

        let _ = fs::remove_file(db_path);
        let _ = fs::remove_file(sidecar_path);
        let _ = fs::remove_file(new_path_buf);
    }

    #[test]
    fn terminal_director_task_updates_keep_latest_timestamp_per_task() {
        let db_path = temp_db_path("terminal-director-updates");
        let db = Db::open(&db_path).expect("db should open");

        let failed = DirectorTaskResult {
            task_id: "task-123".to_string(),
            disposition: crate::director::models::FinalizedTrackDisposition::Failed,
            finalized: None,
            attempts: Vec::new(),
            error: Some("first failure".to_string()),
            candidate_records: Vec::new(),
            provider_searches: Vec::new(),
        };
        db.save_director_task_result(&failed, None)
            .expect("failed result should save");
        std::thread::sleep(std::time::Duration::from_secs(1));
        let cancelled = DirectorTaskResult {
            task_id: "task-123".to_string(),
            disposition: crate::director::models::FinalizedTrackDisposition::Cancelled,
            finalized: None,
            attempts: Vec::new(),
            error: Some("cancelled later".to_string()),
            candidate_records: Vec::new(),
            provider_searches: Vec::new(),
        };
        db.save_director_task_result(&cancelled, None)
            .expect("cancelled result should save");

        let updates = db
            .get_terminal_director_task_updates()
            .expect("terminal updates should load");

        assert_eq!(
            updates.get("task-123"),
            Some(&TerminalDirectorTaskUpdate {
                disposition: "Cancelled".to_string(),
                updated_at: updates["task-123"].updated_at.clone(),
            })
        );

        let _ = fs::remove_file(db_path);
    }

    #[test]
    fn director_task_history_persists_request_payload_for_terminal_results() {
        let db_path = temp_db_path("director-task-request-history");
        let db = Db::open(&db_path).expect("db should open");
        let task = TrackTask {
            task_id: "task-request".to_string(),
            source: TrackTaskSource::SpotifyHistory,
            desired_track_id: Some(42),
            source_operation_id: Some("op-request".to_string()),
            target: NormalizedTrack {
                spotify_track_id: Some("spotify:track:123".to_string()),
                source_album_id: Some("spotify:album:123".to_string()),
                source_artist_id: Some("spotify:artist:123".to_string()),
                source_playlist: Some("playlist-1".to_string()),
                artist: "Artist".to_string(),
                album_artist: Some("Artist".to_string()),
                title: "Song".to_string(),
                album: Some("Album".to_string()),
                track_number: Some(1),
                disc_number: Some(1),
                year: Some(2024),
                duration_secs: Some(42.0),
                isrc: Some("US1234567890".to_string()),
                musicbrainz_recording_id: None,
                musicbrainz_release_group_id: None,
                musicbrainz_release_id: None,
                canonical_artist_id: None,
                canonical_release_id: None,
            },
            strategy: AcquisitionStrategy::DiscographyBatch,
        };
        let result = DirectorTaskResult {
            task_id: task.task_id.clone(),
            disposition: crate::director::models::FinalizedTrackDisposition::Failed,
            finalized: None,
            attempts: Vec::new(),
            error: Some("provider exhausted".to_string()),
            candidate_records: Vec::new(),
            provider_searches: Vec::new(),
        };

        db.save_director_task_result(&result, Some(&task))
            .expect("result should save with request payload");

        let (request_json, request_strategy): (Option<String>, Option<String>) = db
            .conn
            .query_row(
                "SELECT request_json, request_strategy FROM director_task_history WHERE task_id = ?1",
                params![task.task_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("request columns should be readable");
        let persisted_task: TrackTask = serde_json::from_str(
            request_json
                .as_deref()
                .expect("request json should be present"),
        )
        .expect("request json should deserialize");

        assert_eq!(persisted_task.task_id, "task-request");
        assert_eq!(persisted_task.target.artist, "Artist");
        assert_eq!(request_strategy.as_deref(), Some("DiscographyBatch"));

        let _ = fs::remove_file(db_path);
    }

    #[test]
    fn director_task_history_persists_failure_provider_class_and_evidence_tables() {
        use crate::director::models::{ProviderSearchCandidate, ProviderSearchRecord};

        let db_path = temp_db_path("director-failure-evidence");
        let db = Db::open(&db_path).expect("db should open");
        let task = TrackTask {
            task_id: "task-failure-evidence".to_string(),
            source: TrackTaskSource::SpotifyHistory,
            desired_track_id: Some(99),
            source_operation_id: Some("op-failure".to_string()),
            target: NormalizedTrack {
                spotify_track_id: Some("spotify:track:failure".to_string()),
                source_album_id: Some("spotify:album:failure".to_string()),
                source_artist_id: Some("spotify:artist:failure".to_string()),
                source_playlist: None,
                artist: "Artist".to_string(),
                album_artist: Some("Artist".to_string()),
                title: "Song".to_string(),
                album: Some("Album".to_string()),
                track_number: Some(1),
                disc_number: Some(1),
                year: Some(2024),
                duration_secs: Some(42.0),
                isrc: Some("US0000000001".to_string()),
                musicbrainz_recording_id: Some("mb-recording-failure".to_string()),
                musicbrainz_release_group_id: Some("mb-release-group-failure".to_string()),
                musicbrainz_release_id: Some("mb-release-failure".to_string()),
                canonical_artist_id: Some(5),
                canonical_release_id: Some(8),
            },
            strategy: AcquisitionStrategy::Standard,
        };
        let result = DirectorTaskResult {
            task_id: task.task_id.clone(),
            disposition: crate::director::models::FinalizedTrackDisposition::Failed,
            finalized: None,
            attempts: vec![crate::director::models::ProviderAttemptRecord {
                provider_id: "qobuz".to_string(),
                attempt: 1,
                outcome: "validation failed after download".to_string(),
            }],
            error: Some("validation failed after download".to_string()),
            candidate_records: vec![crate::director::models::CandidateRecord {
                provider_id: "qobuz".to_string(),
                provider_display_name: "Qobuz".to_string(),
                provider_trust_rank: 10,
                provider_order_index: 0,
                search_rank: 0,
                candidate: ProviderSearchCandidate {
                    provider_id: "qobuz".to_string(),
                    provider_candidate_id: "qbz-1".to_string(),
                    artist: "Artist".to_string(),
                    title: "Song".to_string(),
                    album: Some("Album".to_string()),
                    duration_secs: Some(42.0),
                    extension_hint: Some("flac".to_string()),
                    bitrate_kbps: Some(1000),
                    cover_art_url: None,
                    metadata_confidence: 0.91,
                },
                acquisition_temp_path: None,
                validation: None,
                score: None,
                score_reason: None,
                outcome: "validation_failed".to_string(),
                rejection_reason: Some("duration mismatch".to_string()),
            }],
            provider_searches: vec![ProviderSearchRecord {
                provider_id: "qobuz".to_string(),
                provider_display_name: "Qobuz".to_string(),
                provider_trust_rank: 10,
                provider_order_index: 0,
                outcome: "candidates_found".to_string(),
                candidate_count: 1,
                error: None,
                retryable: false,
            }],
        };

        db.save_director_task_result(&result, Some(&task))
            .expect("failed result should save");

        let history_row: (Option<String>, Option<String>) = db
            .conn
            .query_row(
                "SELECT provider, failure_class FROM director_task_history WHERE task_id = ?1",
                params![task.task_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("history row should load");
        assert_eq!(history_row.0.as_deref(), Some("qobuz"));
        assert_eq!(history_row.1.as_deref(), Some("validation_failed"));

        let search_evidence_count: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM provider_search_evidence WHERE task_id = ?1",
                params![task.task_id],
                |row| row.get(0),
            )
            .expect("search evidence should count");
        let candidate_evidence_count: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM provider_candidate_evidence WHERE task_id = ?1",
                params![task.task_id],
                |row| row.get(0),
            )
            .expect("candidate evidence should count");
        let cache_failure_class: Option<String> = db
            .conn
            .query_row(
                "SELECT failure_class FROM provider_response_cache WHERE request_signature = ?1 AND provider_id = 'qobuz'",
                params![director_request_signature(&task)],
                |row| row.get(0),
            )
            .expect("response cache should load");
        let identity_evidence_count: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM identity_resolution_evidence WHERE task_id = ?1",
                params![task.task_id],
                |row| row.get(0),
            )
            .expect("identity evidence should count");
        let alias_count: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM source_aliases WHERE entity_type = 'request_signature' AND entity_key = ?1",
                params![director_request_signature(&task)],
                |row| row.get(0),
            )
            .expect("aliases should count");
        let release_group_alias_count: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM source_aliases WHERE entity_type = 'request_signature' AND entity_key = ?1 AND source_name = 'musicbrainz' AND source_key = 'release_group_id' AND source_value = 'mb-release-group-failure'",
                params![director_request_signature(&task)],
                |row| row.get(0),
            )
            .expect("release-group alias should count");

        assert_eq!(search_evidence_count, 1);
        assert_eq!(candidate_evidence_count, 1);
        assert_eq!(cache_failure_class.as_deref(), Some("validation_failed"));
        assert_eq!(identity_evidence_count, 1);
        assert!(alias_count >= 2);
        assert_eq!(release_group_alias_count, 1);

        let _ = fs::remove_file(db_path);
    }

    #[test]
    fn director_task_history_persists_candidate_sets_and_negative_memory() {
        use crate::director::models::{
            CandidateQuality, CandidateRecord, CandidateScore, ProviderSearchCandidate,
            ProviderSearchRecord, SelectionReason, ValidationIssue, ValidationReport,
        };
        use std::collections::BTreeMap;
        use std::path::PathBuf;

        let db_path = temp_db_path("director-candidate-memory");
        let db = Db::open(&db_path).expect("db should open");
        let task = TrackTask {
            task_id: "task-candidates".to_string(),
            source: TrackTaskSource::Manual,
            desired_track_id: Some(7),
            source_operation_id: Some("op-candidates".to_string()),
            target: NormalizedTrack {
                spotify_track_id: None,
                source_album_id: None,
                source_artist_id: None,
                source_playlist: None,
                artist: "Artist".to_string(),
                album_artist: Some("Artist".to_string()),
                title: "Song".to_string(),
                album: Some("Album".to_string()),
                track_number: Some(1),
                disc_number: Some(1),
                year: Some(2024),
                duration_secs: Some(42.0),
                isrc: None,
                musicbrainz_recording_id: None,
                musicbrainz_release_group_id: None,
                musicbrainz_release_id: None,
                canonical_artist_id: None,
                canonical_release_id: None,
            },
            strategy: AcquisitionStrategy::Standard,
        };
        let validation = ValidationReport {
            is_valid: true,
            format_name: Some("flac".to_string()),
            duration_secs: Some(42.0),
            audio_readable: true,
            header_readable: true,
            extension_ok: true,
            file_size: 12 * 1024 * 1024,
            quality: CandidateQuality::Lossless,
            issues: Vec::<ValidationIssue>::new(),
        };
        let failed_validation = ValidationReport {
            is_valid: false,
            format_name: Some("mp3".to_string()),
            duration_secs: Some(19.0),
            audio_readable: false,
            header_readable: true,
            extension_ok: true,
            file_size: 512_000,
            quality: CandidateQuality::Lossy,
            issues: vec![ValidationIssue {
                code: "duration_mismatch".to_string(),
                message: "duration was too short".to_string(),
            }],
        };
        let reason = SelectionReason {
            summary: "Selected Song via Provider One with score 96".to_string(),
            details: BTreeMap::from([
                ("score_total".to_string(), "96".to_string()),
                ("provider".to_string(), "provider-one".to_string()),
            ]),
        };
        let result = DirectorTaskResult {
            task_id: task.task_id.clone(),
            disposition: crate::director::models::FinalizedTrackDisposition::Finalized,
            finalized: Some(crate::director::models::FinalizedTrack {
                path: PathBuf::from("C:\\Music\\Artist\\Album\\01 - Song.flac"),
                replaced_existing: false,
                provenance: crate::director::models::ProvenanceRecord {
                    task_id: task.task_id.clone(),
                    source_metadata: task.target.clone(),
                    selected_provider: "provider-one".to_string(),
                    selected_provider_candidate_id: Some("cand-1".to_string()),
                    score_reason: reason.clone(),
                    validation_summary: validation.clone(),
                    final_path: PathBuf::from("C:\\Music\\Artist\\Album\\01 - Song.flac"),
                    acquired_at: Utc::now(),
                },
            }),
            attempts: vec![
                crate::director::models::ProviderAttemptRecord {
                    provider_id: "provider-one".to_string(),
                    attempt: 1,
                    outcome: "valid candidate score 96".to_string(),
                },
                crate::director::models::ProviderAttemptRecord {
                    provider_id: "provider-two".to_string(),
                    attempt: 1,
                    outcome: "validation failed: duration mismatch".to_string(),
                },
            ],
            error: None,
            candidate_records: vec![
                CandidateRecord {
                    provider_id: "provider-one".to_string(),
                    provider_display_name: "Provider One".to_string(),
                    provider_trust_rank: 5,
                    provider_order_index: 0,
                    search_rank: 0,
                    candidate: ProviderSearchCandidate {
                        provider_id: "provider-one".to_string(),
                        provider_candidate_id: "cand-1".to_string(),
                        artist: "Artist".to_string(),
                        title: "Song".to_string(),
                        album: Some("Album".to_string()),
                        duration_secs: Some(42.0),
                        extension_hint: Some("flac".to_string()),
                        bitrate_kbps: Some(1000),
                        cover_art_url: None,
                        metadata_confidence: 0.98,
                    },
                    acquisition_temp_path: Some(PathBuf::from("C:\\Temp\\cand-1.flac")),
                    validation: Some(validation.clone()),
                    score: Some(CandidateScore {
                        total: 96,
                        metadata_match_points: 39,
                        duration_points: 25,
                        codec_points: 20,
                        provider_points: 11,
                        validation_points: 20,
                        size_points: 5,
                        bitrate_points: 0,
                        format_points: 0,
                    }),
                    score_reason: Some(reason.clone()),
                    outcome: "valid_candidate".to_string(),
                    rejection_reason: None,
                },
                CandidateRecord {
                    provider_id: "provider-two".to_string(),
                    provider_display_name: "Provider Two".to_string(),
                    provider_trust_rank: 15,
                    provider_order_index: 1,
                    search_rank: 0,
                    candidate: ProviderSearchCandidate {
                        provider_id: "provider-two".to_string(),
                        provider_candidate_id: "cand-2".to_string(),
                        artist: "Artist".to_string(),
                        title: "Song".to_string(),
                        album: Some("Album".to_string()),
                        duration_secs: Some(19.0),
                        extension_hint: Some("mp3".to_string()),
                        bitrate_kbps: Some(128),
                        cover_art_url: None,
                        metadata_confidence: 0.52,
                    },
                    acquisition_temp_path: Some(PathBuf::from("C:\\Temp\\cand-2.mp3")),
                    validation: Some(failed_validation),
                    score: None,
                    score_reason: None,
                    outcome: "validation_failed".to_string(),
                    rejection_reason: Some("duration mismatch".to_string()),
                },
            ],
            provider_searches: vec![
                ProviderSearchRecord {
                    provider_id: "provider-one".to_string(),
                    provider_display_name: "Provider One".to_string(),
                    provider_trust_rank: 5,
                    provider_order_index: 0,
                    outcome: "candidates_found".to_string(),
                    candidate_count: 1,
                    error: None,
                    retryable: false,
                },
                ProviderSearchRecord {
                    provider_id: "provider-two".to_string(),
                    provider_display_name: "Provider Two".to_string(),
                    provider_trust_rank: 15,
                    provider_order_index: 1,
                    outcome: "candidates_found".to_string(),
                    candidate_count: 1,
                    error: None,
                    retryable: false,
                },
            ],
        };

        db.save_director_task_result(&result, Some(&task))
            .expect("result should save with candidate memory");

        let stored_items = db
            .get_director_candidate_items(&task.task_id)
            .expect("candidate items should load");
        assert_eq!(stored_items.len(), 2);
        assert_eq!(
            stored_items[0],
            StoredDirectorCandidateItem {
                provider_id: "provider-one".to_string(),
                provider_candidate_id: "cand-1".to_string(),
                outcome: "valid_candidate".to_string(),
                rejection_reason: None,
                is_selected: true,
            }
        );
        assert_eq!(
            stored_items[1],
            StoredDirectorCandidateItem {
                provider_id: "provider-two".to_string(),
                provider_candidate_id: "cand-2".to_string(),
                outcome: "validation_failed".to_string(),
                rejection_reason: Some("duration mismatch".to_string()),
                is_selected: false,
            }
        );

        let signature = director_request_signature(&task);
        let provider_memory = db
            .get_director_provider_memory(&signature)
            .expect("provider memory should load");
        assert_eq!(provider_memory.len(), 1);
        assert_eq!(provider_memory[0].provider_id, "provider-two");
        assert_eq!(provider_memory[0].last_outcome, "no_usable_candidates");
        assert_eq!(provider_memory[0].failure_class, "validation_failed");
        assert!(!provider_memory[0].retryable);
        assert_eq!(provider_memory[0].candidate_count, 1);
        assert!(provider_memory[0].updated_at.contains('-'));

        let _ = fs::remove_file(db_path);
    }

    #[test]
    fn director_request_signature_separates_release_groups() {
        let base_task = TrackTask {
            task_id: "task-signature-release-group-a".to_string(),
            source: TrackTaskSource::Manual,
            desired_track_id: None,
            source_operation_id: None,
            target: NormalizedTrack {
                spotify_track_id: Some("spotify:track:1".to_string()),
                source_album_id: Some("spotify:album:1".to_string()),
                source_artist_id: Some("spotify:artist:1".to_string()),
                source_playlist: None,
                artist: "Artist".to_string(),
                album_artist: Some("Artist".to_string()),
                title: "Song".to_string(),
                album: Some("Album".to_string()),
                track_number: Some(1),
                disc_number: Some(1),
                year: Some(2024),
                duration_secs: Some(42.0),
                isrc: Some("US0000000001".to_string()),
                musicbrainz_recording_id: Some("mb-recording-1".to_string()),
                musicbrainz_release_group_id: Some("mb-release-group-a".to_string()),
                musicbrainz_release_id: Some("mb-release-1".to_string()),
                canonical_artist_id: Some(10),
                canonical_release_id: Some(20),
            },
            strategy: AcquisitionStrategy::Standard,
        };
        let mut changed_release_group = base_task.clone();
        changed_release_group.task_id = "task-signature-release-group-b".to_string();
        changed_release_group.target.musicbrainz_release_group_id =
            Some("mb-release-group-b".to_string());

        let signature_a = director_request_signature(&base_task);
        let signature_b = director_request_signature(&changed_release_group);

        assert_ne!(signature_a, signature_b);
    }

    #[test]
    fn album_ids_are_stable_across_db_reopen() {
        let db_path = temp_db_path("album-id-stability");

        {
            let db = Db::open(&db_path).expect("db should open");
            let track = Track {
                id: 0,
                path: "C:\\Music\\Artist\\Album\\01 - Song.flac".to_string(),
                title: "Song".to_string(),
                artist: "Artist".to_string(),
                album: "Album".to_string(),
                album_artist: "Artist".to_string(),
                track_number: Some(1),
                disc_number: Some(1),
                year: Some(2024),
                duration_secs: 42.0,
                sample_rate: None,
                bit_depth: None,
                bitrate_kbps: None,
                format: "FLAC".to_string(),
                file_size: 4,
                cover_art_path: None,
                isrc: None,
                musicbrainz_recording_id: None,
                musicbrainz_release_id: None,
                canonical_artist_id: None,
                canonical_release_id: None,
                quality_tier: None,
                content_hash: None,
                added_at: String::new(),
            };
            db.upsert_track(&track).expect("track should save");
        }

        let first_id = {
            let db = Db::open(&db_path).expect("db should reopen");
            let albums = db.get_albums().expect("albums should load");
            assert_eq!(albums.len(), 1);
            albums[0].id
        };

        let second_id = {
            let db = Db::open(&db_path).expect("db should reopen");
            let albums = db.get_albums().expect("albums should load");
            assert_eq!(albums.len(), 1);
            albums[0].id
        };

        assert_eq!(first_id, second_id);
        assert_eq!(first_id, stable_entity_id(&["artist", "album"]));

        let _ = fs::remove_file(db_path);
    }

    #[test]
    fn artist_ids_are_stable_across_db_reopen() {
        let db_path = temp_db_path("artist-id-stability");

        {
            let db = Db::open(&db_path).expect("db should open");
            let track = Track {
                id: 0,
                path: "C:\\Music\\Artist\\Album\\01 - Song.flac".to_string(),
                title: "Song".to_string(),
                artist: "Artist".to_string(),
                album: "Album".to_string(),
                album_artist: "Artist".to_string(),
                track_number: Some(1),
                disc_number: Some(1),
                year: Some(2024),
                duration_secs: 42.0,
                sample_rate: None,
                bit_depth: None,
                bitrate_kbps: None,
                format: "FLAC".to_string(),
                file_size: 4,
                cover_art_path: None,
                isrc: None,
                musicbrainz_recording_id: None,
                musicbrainz_release_id: None,
                canonical_artist_id: None,
                canonical_release_id: None,
                quality_tier: None,
                content_hash: None,
                added_at: String::new(),
            };
            db.upsert_track(&track).expect("track should save");
        }

        let first_id = {
            let db = Db::open(&db_path).expect("db should reopen");
            let artists = db.get_artists().expect("artists should load");
            assert_eq!(artists.len(), 1);
            artists[0].id
        };

        let second_id = {
            let db = Db::open(&db_path).expect("db should reopen");
            let artists = db.get_artists().expect("artists should load");
            assert_eq!(artists.len(), 1);
            artists[0].id
        };

        assert_eq!(first_id, second_id);
        assert_eq!(first_id, stable_entity_id(&["artist"]));

        let _ = fs::remove_file(db_path);
    }

    #[test]
    fn track_upsert_backfills_canonical_ids_from_existing_identity() {
        let db_path = temp_db_path("track-canonical-backfill");
        let db = Db::open(&db_path).expect("db should open");
        let canonical_artist_id = db
            .upsert_canonical_artist("Simon and Garfunkel", Some("mb-artist"), None, None, None)
            .expect("canonical artist should save");
        let canonical_release_id = db
            .upsert_canonical_release(
                canonical_artist_id,
                "Bridge Over Troubled Water",
                Some("mb-rg"),
                Some("mb-release"),
                Some("Album"),
                None,
                None,
                Some(1970),
            )
            .expect("canonical release should save");

        let track = Track {
            id: 0,
            path: "C:\\Music\\Simon & Garfunkel\\Bridge Over Troubled Water\\01 - Bridge Over Troubled Water.flac".to_string(),
            title: "Bridge Over Troubled Water".to_string(),
            artist: "Simon & Garfunkel".to_string(),
            album: "Bridge Over Troubled Water".to_string(),
            album_artist: "Simon & Garfunkel".to_string(),
            track_number: Some(1),
            disc_number: Some(1),
            year: Some(1970),
            duration_secs: 42.0,
            sample_rate: None,
            bit_depth: None,
            bitrate_kbps: None,
            format: "FLAC".to_string(),
            file_size: 4,
            cover_art_path: None,
            isrc: None,
            musicbrainz_recording_id: None,
            musicbrainz_release_id: None,
            canonical_artist_id: None,
            canonical_release_id: None,
            quality_tier: None,
            content_hash: None,
            added_at: String::new(),
        };

        db.upsert_track(&track).expect("track should save");
        let stored = db
            .get_track_by_path(&track.path)
            .expect("track should load")
            .expect("track should exist");

        assert_eq!(stored.canonical_artist_id, Some(canonical_artist_id));
        assert_eq!(stored.canonical_release_id, Some(canonical_release_id));

        let _ = fs::remove_file(db_path);
    }

    #[test]
    fn cached_lyrics_roundtrip_uses_normalized_identity_keys() {
        let db_path = temp_db_path("lyrics-cache");
        let db = Db::open(&db_path).expect("db should open");

        db.upsert_track_lyrics(
            None,
            "Simon & Garfunkel",
            "Bridge Over Troubled Water",
            Some("Bridge Over Troubled Water"),
            Some("When you're weary"),
            Some("[00:01.00] When you're weary"),
            "LRCLIB",
        )
        .expect("lyrics should save");

        let cached = db
            .get_cached_track_lyrics(
                "Simon and Garfunkel",
                "Bridge Over Troubled Water",
                Some("Bridge Over Troubled Water"),
            )
            .expect("lyrics lookup should succeed")
            .expect("lyrics should exist");

        assert_eq!(cached.lyrics.as_deref(), Some("When you're weary"));
        assert_eq!(
            cached.synced_lyrics.as_deref(),
            Some("[00:01.00] When you're weary")
        );
        assert_eq!(cached.source, "LRCLIB");
        assert!(!cached.fetched_at.trim().is_empty());

        let _ = fs::remove_file(db_path);
    }

    #[test]
    fn canonical_backfill_updates_existing_tracks_deterministically() {
        let db_path = temp_db_path("canonical-backfill-existing");
        let db = Db::open(&db_path).expect("db should open");
        let canonical_artist_id = db
            .upsert_canonical_artist("Simon and Garfunkel", Some("mb-artist"), None, None, None)
            .expect("canonical artist should save");
        let canonical_release_id = db
            .upsert_canonical_release(
                canonical_artist_id,
                "Bridge Over Troubled Water",
                Some("mb-rg"),
                Some("mb-release"),
                Some("Album"),
                None,
                None,
                Some(1970),
            )
            .expect("canonical release should save");

        let first_track = Track {
            id: 0,
            path: "C:\\Music\\Simon & Garfunkel\\Bridge Over Troubled Water\\01 - Bridge Over Troubled Water.flac".to_string(),
            title: "Bridge Over Troubled Water".to_string(),
            artist: "Simon & Garfunkel".to_string(),
            album: "Bridge Over Troubled Water".to_string(),
            album_artist: "Simon & Garfunkel".to_string(),
            track_number: Some(1),
            disc_number: Some(1),
            year: Some(1970),
            duration_secs: 42.0,
            sample_rate: None,
            bit_depth: None,
            bitrate_kbps: None,
            format: "FLAC".to_string(),
            file_size: 4,
            cover_art_path: None,
            isrc: None,
            musicbrainz_recording_id: None,
            musicbrainz_release_id: None,
            canonical_artist_id: None,
            canonical_release_id: None,
            quality_tier: None,
            content_hash: None,
            added_at: String::new(),
        };
        let second_track = Track {
            path:
                "C:\\Music\\Simon & Garfunkel\\Bridge Over Troubled Water\\02 - El Condor Pasa.flac"
                    .to_string(),
            title: "El Condor Pasa".to_string(),
            ..first_track.clone()
        };

        db.upsert_track(&first_track)
            .expect("first track should save");
        db.upsert_track(&second_track)
            .expect("second track should save");
        db.conn
            .execute(
                "UPDATE tracks
                 SET canonical_artist_id = NULL,
                     canonical_release_id = NULL",
                [],
            )
            .expect("canonical ids should clear");

        let updated = db
            .backfill_missing_track_canonical_ids(1)
            .expect("backfill should succeed");
        assert_eq!(updated, 1);

        let first_stored = db
            .get_track_by_path(&first_track.path)
            .expect("first track should load")
            .expect("first track should exist");
        let second_stored = db
            .get_track_by_path(&second_track.path)
            .expect("second track should load")
            .expect("second track should exist");

        assert_eq!(first_stored.canonical_artist_id, Some(canonical_artist_id));
        assert_eq!(
            first_stored.canonical_release_id,
            Some(canonical_release_id)
        );
        assert_eq!(second_stored.canonical_artist_id, None);
        assert_eq!(second_stored.canonical_release_id, None);

        let _ = fs::remove_file(db_path);
    }
}
