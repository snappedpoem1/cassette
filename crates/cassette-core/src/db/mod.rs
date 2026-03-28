use crate::models::{
    Album, Artist, LibraryRoot, Playlist, PlaylistItem, QueueItem, SpotifyAlbumHistory, Track,
};
use crate::director::models::DirectorTaskResult;
use crate::Result;
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
        ")?;
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

    pub fn save_director_task_result(&self, result: &DirectorTaskResult) -> Result<()> {
        let provider = result
            .finalized
            .as_ref()
            .map(|finalized| finalized.provenance.selected_provider.clone());
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

        self.conn.execute(
            "
            INSERT INTO director_task_history
                (task_id, disposition, provider, final_path, score_total, error,
                 source_metadata_json, validation_json, score_reason_json,
                 attempts_json, result_json, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, datetime('now'))
            ON CONFLICT(task_id) DO UPDATE SET
                disposition=excluded.disposition,
                provider=excluded.provider,
                final_path=excluded.final_path,
                score_total=excluded.score_total,
                error=excluded.error,
                source_metadata_json=excluded.source_metadata_json,
                validation_json=excluded.validation_json,
                score_reason_json=excluded.score_reason_json,
                attempts_json=excluded.attempts_json,
                result_json=excluded.result_json,
                updated_at=datetime('now')
            ",
            params![
                result.task_id,
                format!("{:?}", result.disposition),
                provider,
                final_path,
                score_total,
                result.error,
                source_metadata_json,
                validation_json,
                score_reason_json,
                attempts_json,
                result_json,
            ],
        )?;
        Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::SpotifyAlbumHistory;
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
}
