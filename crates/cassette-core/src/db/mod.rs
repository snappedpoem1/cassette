use crate::models::{
    Album, Artist, LibraryRoot, Playlist, PlaylistItem, QueueItem, SpotifyAlbumHistory, Track,
};
use crate::director::models::{DirectorTaskResult, TrackTask};
use crate::Result;
use chrono::{Duration, Utc};
use rusqlite::{params, Connection};
use std::collections::HashMap;
use std::path::Path;

pub struct Db {
    conn: Connection,
}

impl Db {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<()> {
        self.conn.execute_batch("
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
        ")?;
        self.ensure_column_exists("director_task_history", "request_json", "TEXT")?;
        self.ensure_column_exists("director_task_history", "request_strategy", "TEXT")?;
        self.ensure_column_exists("director_task_history", "request_signature", "TEXT")?;
        self.ensure_column_exists("director_pending_tasks", "request_signature", "TEXT")?;
        Ok(())
    }

    fn ensure_column_exists(&self, table: &str, column: &str, column_type: &str) -> Result<()> {
        let mut stmt = self
            .conn
            .prepare(&format!("PRAGMA table_info({table})"))?;
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
        self.conn.execute("INSERT OR IGNORE INTO library_roots (path) VALUES (?1)", params![path])?;
        Ok(())
    }

    pub fn remove_library_root(&self, path: &str) -> Result<()> {
        self.conn.execute("DELETE FROM library_roots WHERE path = ?1", params![path])?;
        Ok(())
    }

    pub fn get_library_roots(&self) -> Result<Vec<LibraryRoot>> {
        let mut stmt = self.conn.prepare("SELECT id, path, enabled FROM library_roots ORDER BY id")?;
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
        self.conn.execute("
            INSERT INTO tracks
                (path,title,artist,album,album_artist,track_number,disc_number,year,
                 duration_secs,sample_rate,bit_depth,bitrate_kbps,format,file_size,cover_art_path)
            VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)
            ON CONFLICT(path) DO UPDATE SET
                title=excluded.title, artist=excluded.artist, album=excluded.album,
                album_artist=excluded.album_artist, track_number=excluded.track_number,
                disc_number=excluded.disc_number, year=excluded.year,
                duration_secs=excluded.duration_secs, sample_rate=excluded.sample_rate,
                bit_depth=excluded.bit_depth, bitrate_kbps=excluded.bitrate_kbps,
                format=excluded.format, file_size=excluded.file_size,
                cover_art_path=excluded.cover_art_path
        ", params![
            t.path, t.title, t.artist, t.album, t.album_artist,
            t.track_number, t.disc_number, t.year, t.duration_secs,
            t.sample_rate, t.bit_depth, t.bitrate_kbps, t.format,
            t.file_size as i64, t.cover_art_path,
        ])?;
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
                     duration_secs,sample_rate,bit_depth,bitrate_kbps,format,file_size,cover_art_path)
                VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)
                ON CONFLICT(path) DO UPDATE SET
                    title=excluded.title, artist=excluded.artist, album=excluded.album,
                    album_artist=excluded.album_artist, track_number=excluded.track_number,
                    disc_number=excluded.disc_number, year=excluded.year,
                    duration_secs=excluded.duration_secs, sample_rate=excluded.sample_rate,
                    bit_depth=excluded.bit_depth, bitrate_kbps=excluded.bitrate_kbps,
                    format=excluded.format, file_size=excluded.file_size,
                    cover_art_path=excluded.cover_art_path
                "
            )?;

            for t in tracks {
                stmt.execute(params![
                    t.path, t.title, t.artist, t.album, t.album_artist,
                    t.track_number, t.disc_number, t.year, t.duration_secs,
                    t.sample_rate, t.bit_depth, t.bitrate_kbps, t.format,
                    t.file_size as i64, t.cover_art_path,
                ])?;
            }
            Ok(())
        })();

        if let Err(error) = write_result {
            let _ = self.conn.execute_batch("ROLLBACK;");
            return Err(error);
        }

        self.conn.execute_batch("COMMIT;")?;
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
        Ok(self.conn.query_row("SELECT COUNT(*) FROM tracks", [], |r| r.get(0))?)
    }

    pub fn get_tracks(&self, limit: i64, offset: i64) -> Result<Vec<Track>> {
        let mut stmt = self.conn.prepare("
            SELECT id,path,title,artist,album,album_artist,track_number,disc_number,year,
                   duration_secs,sample_rate,bit_depth,bitrate_kbps,format,file_size,
                   cover_art_path,added_at
            FROM tracks
            ORDER BY album_artist COLLATE NOCASE, album COLLATE NOCASE, disc_number, track_number
            LIMIT ?1 OFFSET ?2
        ")?;
        let rows = stmt.query_map(params![limit, offset], Self::row_to_track)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn search_tracks(&self, query: &str) -> Result<Vec<Track>> {
        let pattern = format!("%{query}%");
        let mut stmt = self.conn.prepare("
            SELECT id,path,title,artist,album,album_artist,track_number,disc_number,year,
                   duration_secs,sample_rate,bit_depth,bitrate_kbps,format,file_size,
                   cover_art_path,added_at
            FROM tracks
            WHERE title LIKE ?1 OR artist LIKE ?1 OR album LIKE ?1
            ORDER BY artist COLLATE NOCASE, album COLLATE NOCASE, track_number
            LIMIT 200
        ")?;
        let rows = stmt.query_map(params![pattern], Self::row_to_track)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn get_track_by_id(&self, id: i64) -> Result<Option<Track>> {
        let mut stmt = self.conn.prepare("
            SELECT id,path,title,artist,album,album_artist,track_number,disc_number,year,
                   duration_secs,sample_rate,bit_depth,bitrate_kbps,format,file_size,
                   cover_art_path,added_at
            FROM tracks WHERE id = ?1
        ")?;
        let mut rows = stmt.query_map(params![id], Self::row_to_track)?;
        Ok(rows.next().transpose()?)
    }

    pub fn get_track_by_path(&self, path: &str) -> Result<Option<Track>> {
        let mut stmt = self.conn.prepare("
            SELECT id,path,title,artist,album,album_artist,track_number,disc_number,year,
                   duration_secs,sample_rate,bit_depth,bitrate_kbps,format,file_size,
                   cover_art_path,added_at
            FROM tracks WHERE path = ?1
        ")?;
        let mut rows = stmt.query_map(params![path], Self::row_to_track)?;
        Ok(rows.next().transpose()?)
    }

    pub fn get_albums(&self) -> Result<Vec<Album>> {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut stmt = self.conn.prepare("
            SELECT album_artist, album, MIN(year), MIN(cover_art_path), COUNT(*)
            FROM tracks GROUP BY album_artist, album
            ORDER BY album_artist COLLATE NOCASE, album COLLATE NOCASE
        ")?;
        let rows = stmt.query_map([], |row| {
            let artist: String = row.get(0)?;
            let title: String = row.get(1)?;
            // Stable id derived from the group key — survives re-ordering.
            let mut hasher = DefaultHasher::new();
            artist.to_ascii_lowercase().hash(&mut hasher);
            title.to_ascii_lowercase().hash(&mut hasher);
            let id = hasher.finish() as i64;
            Ok(Album {
                id,
                title,
                artist,
                year: row.get(2)?,
                cover_art_path: row.get(3)?,
                track_count: row.get::<_, i64>(4)? as usize,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn get_album_tracks(&self, artist: &str, album: &str) -> Result<Vec<Track>> {
        let mut stmt = self.conn.prepare("
            SELECT id,path,title,artist,album,album_artist,track_number,disc_number,year,
                   duration_secs,sample_rate,bit_depth,bitrate_kbps,format,file_size,
                   cover_art_path,added_at
            FROM tracks WHERE album_artist = ?1 AND album = ?2
            ORDER BY disc_number, track_number
        ")?;
        let rows = stmt.query_map(params![artist, album], Self::row_to_track)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn get_artists(&self) -> Result<Vec<Artist>> {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut stmt = self.conn.prepare("
            SELECT album_artist, COUNT(DISTINCT album), COUNT(*)
            FROM tracks GROUP BY album_artist
            ORDER BY album_artist COLLATE NOCASE
        ")?;
        let rows = stmt.query_map([], |row| {
            let name: String = row.get(0)?;
            let mut hasher = DefaultHasher::new();
            name.to_ascii_lowercase().hash(&mut hasher);
            let id = hasher.finish() as i64;
            Ok(Artist {
                id,
                name,
                album_count: row.get::<_, i64>(1)? as usize,
                track_count: row.get::<_, i64>(2)? as usize,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Update a track's path after file move
    pub fn update_track_path(&self, track_id: i64, new_path: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE tracks SET path = ?1 WHERE id = ?2",
            params![new_path, track_id],
        )?;
        Ok(())
    }

    /// Get all tracks for a specific album (by album_artist + album name)
    pub fn get_all_tracks_unfiltered(&self) -> Result<Vec<Track>> {
        let mut stmt = self.conn.prepare("
            SELECT id,path,title,artist,album,album_artist,track_number,disc_number,year,
                   duration_secs,sample_rate,bit_depth,bitrate_kbps,format,file_size,
                   cover_art_path,added_at
            FROM tracks
            ORDER BY album_artist COLLATE NOCASE, album COLLATE NOCASE, disc_number, track_number
        ")?;
        let rows = stmt.query_map([], Self::row_to_track)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Delete a track by ID
    pub fn delete_track(&self, track_id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM tracks WHERE id = ?1", params![track_id])?;
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
                self.conn.execute("DELETE FROM tracks WHERE id = ?1", params![id])?;
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
            added_at: row.get(16).unwrap_or_default(),
        })
    }

    // ── Queue ─────────────────────────────────────────────────────────────────

    pub fn get_queue(&self) -> Result<Vec<QueueItem>> {
        let mut stmt = self.conn.prepare("
            SELECT q.id, q.track_id, q.position,
                   t.id,t.path,t.title,t.artist,t.album,t.album_artist,t.track_number,
                   t.disc_number,t.year,t.duration_secs,t.sample_rate,t.bit_depth,
                   t.bitrate_kbps,t.format,t.file_size,t.cover_art_path,t.added_at
            FROM queue_items q JOIN tracks t ON q.track_id = t.id
            ORDER BY q.position
        ")?;
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
                    added_at: row.get(19).unwrap_or_default(),
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
            "SELECT COALESCE(MAX(position), -1) FROM queue_items", [], |r| r.get(0),
        )?)
    }

    // ── Settings ──────────────────────────────────────────────────────────────

    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let mut stmt = self.conn.prepare("SELECT value FROM settings WHERE key = ?1")?;
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
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, datetime('now'))"
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
        Ok(self.conn.query_row(
            "SELECT COUNT(*) FROM spotify_album_history",
            [],
            |row| row.get(0),
        )?)
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
        let provider = result
            .finalized
            .as_ref()
            .map(|finalized| finalized.provenance.selected_provider.clone());
        let selected_provider_candidate_id = result
            .finalized
            .as_ref()
            .and_then(|finalized| {
                finalized
                    .provenance
                    .selected_provider_candidate_id
                    .clone()
            });
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
        let request_json = request
            .map(serde_json::to_string)
            .transpose()?;
        let request_strategy = request.map(|task| format!("{:?}", task.strategy));

        self.conn.execute_batch("BEGIN IMMEDIATE TRANSACTION;")?;
        let write_result: Result<()> = (|| {
            self.conn.execute(
                "
                INSERT INTO director_task_history
                    (task_id, disposition, provider, final_path, request_signature, score_total, error,
                     source_metadata_json, validation_json, score_reason_json,
                     attempts_json, result_json, request_json, request_strategy, updated_at)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, datetime('now'))
                ON CONFLICT(task_id) DO UPDATE SET
                    disposition=excluded.disposition,
                    provider=excluded.provider,
                    final_path=excluded.final_path,
                    request_signature=COALESCE(excluded.request_signature, director_task_history.request_signature),
                    score_total=excluded.score_total,
                    error=excluded.error,
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

            self.persist_candidate_set(result, request_signature.as_deref(), request_strategy.as_deref(), score_total, selected_provider_candidate_id.as_deref())?;
            self.persist_provider_searches(result, request_signature.as_deref())?;
            self.persist_candidate_items(result, request_signature.as_deref(), selected_provider_candidate_id.as_deref())?;
            self.persist_provider_attempts(result, request_signature.as_deref())?;
            self.refresh_provider_memory(result, request_signature.as_deref())?;
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
                && selected_provider_candidate_id == Some(record.candidate.provider_candidate_id.as_str());
            let validation_json = record
                .validation
                .as_ref()
                .map(serde_json::to_string)
                .transpose()?;
            let score_json = record.score.as_ref().map(serde_json::to_string).transpose()?;
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
        let selected_candidate_id = result
            .finalized
            .as_ref()
            .and_then(|finalized| {
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

            let summary = summarize_provider_memory(&provider_id, &provider_searches, &provider_candidates);
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
            params![task.task_id, task_json, request_signature, strategy, progress],
        )?;
        Ok(())
    }

    /// Refresh the stored progress for a pending director task without changing its payload.
    pub fn update_director_pending_task_progress(&self, task_id: &str, progress: &str) -> Result<()> {
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
            let mut task: TrackTask = serde_json::from_str(&row.get::<_, String>(1)?)
                .map_err(|error| rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(error)))?;
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
            let mut task: TrackTask = serde_json::from_str(&row.get::<_, String>(1)?)
                .map_err(|error| rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(error)))?;
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
            SELECT provider_id, last_outcome, failure_class, retryable, candidate_count
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
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    // ── Batch Acquisition Helpers ──────────────────────────────────────────────

    /// Returns missing albums from Spotify history that have significant listening
    /// time (>30min) and play count (>10), ordered by play_count descending.
    pub fn get_missing_spotify_albums(&self, limit: usize) -> Result<Vec<SpotifyAlbumHistory>> {
        let mut stmt = self.conn.prepare("
            SELECT artist, album, total_ms, play_count, skip_count, in_library, imported_at
            FROM spotify_album_history
            WHERE in_library = 0 AND total_ms > 1800000 AND play_count > 10
            ORDER BY play_count DESC
            LIMIT ?1
        ")?;
        let rows = stmt.query_map(params![limit as i64], |row| {
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

    /// Returns task_ids from director_task_history that have already been finalized
    /// or marked as already-present, so batch submissions can skip them.
    pub fn get_completed_task_keys(&self) -> Result<std::collections::HashSet<String>> {
        let mut stmt = self.conn.prepare("
            SELECT task_id FROM director_task_history
            WHERE disposition IN ('Finalized', 'AlreadyPresent')
        ")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        Ok(rows.collect::<rusqlite::Result<std::collections::HashSet<_>>>()?)
    }

    /// Returns task_ids that should never be resubmitted from pending recovery.
    pub fn get_non_resumable_task_keys(&self) -> Result<std::collections::HashSet<String>> {
        let mut stmt = self.conn.prepare("
            SELECT task_id FROM director_task_history
            WHERE disposition IN ('Finalized', 'AlreadyPresent', 'Cancelled')
        ")?;
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
        let mut stmt = self.conn.prepare(
            "SELECT task_id FROM director_task_history WHERE disposition = 'Failed'",
        )?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
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
        let mut stmt = self.conn.prepare("
            SELECT p.id, p.name, p.description, p.created_at, COUNT(pi.id)
            FROM playlists p LEFT JOIN playlist_items pi ON pi.playlist_id = p.id
            GROUP BY p.id ORDER BY p.name COLLATE NOCASE
        ")?;
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
        let mut stmt = self.conn.prepare("
            SELECT pi.id, pi.playlist_id, pi.track_id, pi.position,
                   t.id,t.path,t.title,t.artist,t.album,t.album_artist,t.track_number,
                   t.disc_number,t.year,t.duration_secs,t.sample_rate,t.bit_depth,
                   t.bitrate_kbps,t.format,t.file_size,t.cover_art_path,t.added_at
            FROM playlist_items pi JOIN tracks t ON pi.track_id = t.id
            WHERE pi.playlist_id = ?1 ORDER BY pi.position
        ")?;
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
                    added_at: row.get(20).unwrap_or_default(),
                }),
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn create_playlist(&self, name: &str, description: Option<&str>, track_ids: &[i64]) -> Result<i64> {
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
        self.conn.execute("DELETE FROM playlist_items WHERE playlist_id = ?1", params![playlist_id])?;
        for (pos, tid) in track_ids.iter().enumerate() {
            self.conn.execute(
                "INSERT INTO playlist_items (playlist_id, track_id, position) VALUES (?1,?2,?3)",
                params![playlist_id, tid, pos as i64],
            )?;
        }
        Ok(())
    }

    pub fn delete_playlist(&self, playlist_id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM playlists WHERE id = ?1", params![playlist_id])?;
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredProviderMemory {
    pub provider_id: String,
    pub last_outcome: String,
    pub failure_class: String,
    pub retryable: bool,
    pub candidate_count: usize,
}

#[derive(Debug, Clone)]
struct ProviderMemorySummary {
    last_outcome: String,
    failure_class: String,
    error: Option<String>,
    retryable: bool,
    candidate_count: usize,
}

fn director_request_signature(task: &TrackTask) -> String {
    let target = &task.target;
    let duration = target
        .duration_secs
        .map(|value| format!("{value:.3}"))
        .unwrap_or_default();
    [
        "track".to_string(),
        normalize_signature_text(&target.artist),
        normalize_signature_text(&target.title),
        normalize_signature_text(target.album.as_deref().unwrap_or_default()),
        normalize_signature_text(target.album_artist.as_deref().unwrap_or_default()),
        target.track_number.map(|value| value.to_string()).unwrap_or_default(),
        target.disc_number.map(|value| value.to_string()).unwrap_or_default(),
        target.year.map(|value| value.to_string()).unwrap_or_default(),
        duration,
        normalize_signature_text(target.isrc.as_deref().unwrap_or_default()),
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
        matches!(record.outcome.as_str(), "valid_candidate" | "selected_immediate")
    }) {
        return None;
    }

    if let Some(search) = provider_searches
        .iter()
        .rev()
        .find(|record| matches!(record.outcome.as_str(), "no_candidates" | "search_error" | "metadata_only" | "busy" | "skipped_health_down"))
    {
        return Some(ProviderMemorySummary {
            last_outcome: search.outcome.clone(),
            failure_class: match search.outcome.as_str() {
                "no_candidates" => "no_result".to_string(),
                "metadata_only" => "unsupported".to_string(),
                "busy" => "provider_busy".to_string(),
                "skipped_health_down" => "provider_unhealthy".to_string(),
                _ => "search_error".to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::SpotifyAlbumHistory;
    use crate::director::models::{AcquisitionStrategy, NormalizedTrack, TrackTask, TrackTaskSource};
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
    fn pending_director_task_roundtrip() {
        let db_path = temp_db_path("pending-director");
        let db = Db::open(&db_path).expect("db should open");

        let task = TrackTask {
            task_id: "task-123".to_string(),
            source: TrackTaskSource::Manual,
            target: NormalizedTrack {
                spotify_track_id: None,
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
            target: NormalizedTrack {
                spotify_track_id: Some("spotify:track:123".to_string()),
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
            request_json.as_deref().expect("request json should be present"),
        )
        .expect("request json should deserialize");

        assert_eq!(persisted_task.task_id, "task-request");
        assert_eq!(persisted_task.target.artist, "Artist");
        assert_eq!(request_strategy.as_deref(), Some("DiscographyBatch"));

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
            target: NormalizedTrack {
                spotify_track_id: None,
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
        assert_eq!(
            provider_memory,
            vec![StoredProviderMemory {
                provider_id: "provider-two".to_string(),
                last_outcome: "no_usable_candidates".to_string(),
                failure_class: "validation_failed".to_string(),
                retryable: false,
                candidate_count: 1,
            }]
        );

        let _ = fs::remove_file(db_path);
    }
}
