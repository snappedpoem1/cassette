pub mod migrations;

use crate::librarian::error::Result;
use crate::librarian::models::{
    DesiredTrack, LocalFile, LocalFileScanState, NewDeltaQueueItem, NewLocalFile,
    NewReconciliationResult, ScanCheckpoint, Track,
};
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use sqlx::Row;
use std::path::Path;

#[derive(Clone)]
pub struct LibrarianDb {
    pool: SqlitePool,
}

impl LibrarianDb {
    pub fn from_pool(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn connect(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let url = format!("sqlite://{}", path.to_string_lossy());
        let pool = SqlitePoolOptions::new()
            .max_connections(8)
            .connect(&url)
            .await?;

        sqlx::query("PRAGMA journal_mode=WAL;").execute(&pool).await?;
        sqlx::query("PRAGMA foreign_keys=ON;").execute(&pool).await?;
        sqlx::query("PRAGMA synchronous=NORMAL;").execute(&pool).await?;

        let db = Self { pool };
        db.migrate().await?;
        Ok(db)
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn migrate(&self) -> Result<()> {
        for sql in migrations::MIGRATIONS {
            sqlx::query(sql).execute(&self.pool).await?;
        }
        Ok(())
    }

    pub async fn upsert_artist(&self, canonical_name: &str, normalized_name: &str) -> Result<i64> {
        sqlx::query(
            "INSERT INTO artists (canonical_name, normalized_name) VALUES (?1, ?2)
             ON CONFLICT(normalized_name) DO UPDATE SET
                canonical_name = excluded.canonical_name,
                updated_at = CURRENT_TIMESTAMP",
        )
        .bind(canonical_name)
        .bind(normalized_name)
        .execute(&self.pool)
        .await?;

        let id = sqlx::query_scalar::<_, i64>("SELECT id FROM artists WHERE normalized_name = ?1")
            .bind(normalized_name)
            .fetch_one(&self.pool)
            .await?;
        Ok(id)
    }

    pub async fn upsert_album(
        &self,
        artist_id: i64,
        title: &str,
        normalized_title: &str,
        cover_art_path: Option<&str>,
    ) -> Result<i64> {
        let upsert_result = sqlx::query(
            "INSERT INTO albums (artist_id, title, normalized_title, cover_art_path) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(artist_id, normalized_title) DO UPDATE SET
                title = excluded.title,
                cover_art_path = COALESCE(excluded.cover_art_path, albums.cover_art_path),
                updated_at = CURRENT_TIMESTAMP",
        )
        .bind(artist_id)
        .bind(title)
        .bind(normalized_title)
        .bind(cover_art_path)
        .execute(&self.pool)
        .await;

        if upsert_result.is_err() {
            // Backward compatibility for SQLite builds where unique constraint is absent.
            let existing = sqlx::query_scalar::<_, i64>(
                "SELECT id FROM albums WHERE artist_id = ?1 AND normalized_title = ?2 LIMIT 1",
            )
            .bind(artist_id)
            .bind(normalized_title)
            .fetch_optional(&self.pool)
            .await?;
            if existing.is_none() {
                sqlx::query(
                    "INSERT INTO albums (artist_id, title, normalized_title, cover_art_path) VALUES (?1, ?2, ?3, ?4)",
                )
                .bind(artist_id)
                .bind(title)
                .bind(normalized_title)
                .bind(cover_art_path)
                .execute(&self.pool)
                .await?;
            }
        }

        let id = sqlx::query_scalar::<_, i64>(
            "SELECT id FROM albums WHERE artist_id = ?1 AND normalized_title = ?2 LIMIT 1",
        )
        .bind(artist_id)
        .bind(normalized_title)
        .fetch_one(&self.pool)
        .await?;
        Ok(id)
    }

    pub async fn upsert_track(
        &self,
        artist_id: i64,
        album_id: Option<i64>,
        title: &str,
        normalized_title: &str,
        track_number: Option<i64>,
        disc_number: Option<i64>,
        duration_ms: Option<i64>,
        isrc: Option<&str>,
    ) -> Result<i64> {
        if let Some(isrc) = isrc {
            sqlx::query(
                "INSERT INTO tracks (artist_id, album_id, title, normalized_title, track_number, disc_number, duration_ms, isrc)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT(isrc) DO UPDATE SET
                    title = excluded.title,
                    normalized_title = excluded.normalized_title,
                    album_id = COALESCE(excluded.album_id, tracks.album_id),
                    duration_ms = COALESCE(excluded.duration_ms, tracks.duration_ms),
                    updated_at = CURRENT_TIMESTAMP",
            )
            .bind(artist_id)
            .bind(album_id)
            .bind(title)
            .bind(normalized_title)
            .bind(track_number)
            .bind(disc_number)
            .bind(duration_ms)
            .bind(isrc)
            .execute(&self.pool)
            .await?;

            let id = sqlx::query_scalar::<_, i64>("SELECT id FROM tracks WHERE isrc = ?1")
                .bind(isrc)
                .fetch_one(&self.pool)
                .await?;
            return Ok(id);
        }

        let existing = sqlx::query_scalar::<_, i64>(
            "SELECT id FROM tracks
             WHERE artist_id = ?1 AND COALESCE(album_id, 0) = COALESCE(?2, 0)
               AND normalized_title = ?3
               AND COALESCE(track_number, 0) = COALESCE(?4, 0)
               AND COALESCE(disc_number, 0) = COALESCE(?5, 0)
             LIMIT 1",
        )
        .bind(artist_id)
        .bind(album_id)
        .bind(normalized_title)
        .bind(track_number)
        .bind(disc_number)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(id) = existing {
            sqlx::query(
                "UPDATE tracks SET title = ?1, normalized_title = ?2, duration_ms = COALESCE(?3, duration_ms), updated_at = CURRENT_TIMESTAMP WHERE id = ?4",
            )
            .bind(title)
            .bind(normalized_title)
            .bind(duration_ms)
            .bind(id)
            .execute(&self.pool)
            .await?;
            return Ok(id);
        }

        let result = sqlx::query(
            "INSERT INTO tracks (artist_id, album_id, title, normalized_title, track_number, disc_number, duration_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )
        .bind(artist_id)
        .bind(album_id)
        .bind(title)
        .bind(normalized_title)
        .bind(track_number)
        .bind(disc_number)
        .bind(duration_ms)
        .execute(&self.pool)
        .await?;
        Ok(result.last_insert_rowid())
    }

    pub async fn upsert_local_file(&self, file: &NewLocalFile) -> Result<i64> {
        sqlx::query(
            "INSERT INTO local_files (
                track_id, file_path, file_name, extension, codec, bitrate, sample_rate, bit_depth,
                channels, duration_ms, file_size, file_mtime_ms, content_hash, integrity_status, quality_tier, last_scanned_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, CURRENT_TIMESTAMP)
             ON CONFLICT(file_path) DO UPDATE SET
                track_id = COALESCE(excluded.track_id, local_files.track_id),
                file_name = excluded.file_name,
                extension = excluded.extension,
                codec = excluded.codec,
                bitrate = excluded.bitrate,
                sample_rate = excluded.sample_rate,
                bit_depth = excluded.bit_depth,
                channels = excluded.channels,
                duration_ms = excluded.duration_ms,
                file_size = excluded.file_size,
                file_mtime_ms = excluded.file_mtime_ms,
                content_hash = COALESCE(excluded.content_hash, local_files.content_hash),
                integrity_status = excluded.integrity_status,
                quality_tier = excluded.quality_tier,
                last_scanned_at = CURRENT_TIMESTAMP,
                updated_at = CURRENT_TIMESTAMP",
        )
        .bind(file.track_id)
        .bind(&file.file_path)
        .bind(&file.file_name)
        .bind(&file.extension)
        .bind(&file.codec)
        .bind(file.bitrate)
        .bind(file.sample_rate)
        .bind(file.bit_depth)
        .bind(file.channels)
        .bind(file.duration_ms)
        .bind(file.file_size)
        .bind(file.file_mtime_ms)
        .bind(&file.content_hash)
        .bind(file.integrity_status.as_str())
        .bind(file.quality_tier.map(|q| q.as_str().to_string()))
        .execute(&self.pool)
        .await?;

        let id = sqlx::query_scalar::<_, i64>("SELECT id FROM local_files WHERE file_path = ?1")
            .bind(&file.file_path)
            .fetch_one(&self.pool)
            .await?;
        Ok(id)
    }

    pub async fn get_local_file_scan_state(&self, path: &str) -> Result<Option<LocalFileScanState>> {
        let state = sqlx::query_as::<_, LocalFileScanState>(
            "SELECT file_path, file_size, file_mtime_ms
             FROM local_files
             WHERE file_path = ?1
             LIMIT 1",
        )
        .bind(path)
        .fetch_optional(&self.pool)
        .await?;
        Ok(state)
    }

    pub async fn upsert_scan_checkpoint(
        &self,
        root_path: &str,
        run_id: &str,
        last_scanned_path: Option<&str>,
        status: &str,
        files_seen: i64,
        files_indexed: i64,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO scan_checkpoints (
                root_path, last_run_id, last_scanned_path, status, files_seen, files_indexed, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, CURRENT_TIMESTAMP)
             ON CONFLICT(root_path) DO UPDATE SET
                last_run_id = excluded.last_run_id,
                last_scanned_path = excluded.last_scanned_path,
                status = excluded.status,
                files_seen = excluded.files_seen,
                files_indexed = excluded.files_indexed,
                updated_at = CURRENT_TIMESTAMP",
        )
        .bind(root_path)
        .bind(run_id)
        .bind(last_scanned_path)
        .bind(status)
        .bind(files_seen)
        .bind(files_indexed)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_scan_checkpoint(&self, root_path: &str) -> Result<Option<ScanCheckpoint>> {
        let checkpoint = sqlx::query_as::<_, ScanCheckpoint>(
            "SELECT id, root_path, last_run_id, last_scanned_path, status, files_seen, files_indexed, created_at, updated_at
             FROM scan_checkpoints
             WHERE root_path = ?1
             LIMIT 1",
        )
        .bind(root_path)
        .fetch_optional(&self.pool)
        .await?;
        Ok(checkpoint)
    }

    pub async fn has_completed_checkpoints(&self, roots: &[String]) -> Result<bool> {
        if roots.is_empty() {
            return Ok(false);
        }

        for root in roots {
            let status = sqlx::query_scalar::<_, Option<String>>(
                "SELECT status FROM scan_checkpoints WHERE root_path = ?1 LIMIT 1",
            )
            .bind(root)
            .fetch_one(&self.pool)
            .await?;
            if status.as_deref() != Some("completed") {
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub async fn insert_desired_track(
        &self,
        source_name: &str,
        source_track_id: Option<&str>,
        source_album_id: Option<&str>,
        source_artist_id: Option<&str>,
        artist_name: &str,
        album_title: Option<&str>,
        track_title: &str,
        track_number: Option<i64>,
        disc_number: Option<i64>,
        duration_ms: Option<i64>,
        isrc: Option<&str>,
        raw_payload_json: Option<&str>,
    ) -> Result<i64> {
        let result = sqlx::query(
            "INSERT INTO desired_tracks (
                source_name, source_track_id, source_album_id, source_artist_id,
                artist_name, album_title, track_title, track_number, disc_number,
                duration_ms, isrc, raw_payload_json
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        )
        .bind(source_name)
        .bind(source_track_id)
        .bind(source_album_id)
        .bind(source_artist_id)
        .bind(artist_name)
        .bind(album_title)
        .bind(track_title)
        .bind(track_number)
        .bind(disc_number)
        .bind(duration_ms)
        .bind(isrc)
        .bind(raw_payload_json)
        .execute(&self.pool)
        .await?;
        Ok(result.last_insert_rowid())
    }

    pub async fn insert_reconciliation_result(&self, row: &NewReconciliationResult) -> Result<i64> {
        let result = sqlx::query(
            "INSERT INTO reconciliation_results (
                desired_track_id, matched_track_id, matched_local_file_id,
                reconciliation_status, quality_assessment, reason
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind(row.desired_track_id)
        .bind(row.matched_track_id)
        .bind(row.matched_local_file_id)
        .bind(row.reconciliation_status.as_str())
        .bind(&row.quality_assessment)
        .bind(&row.reason)
        .execute(&self.pool)
        .await?;
        Ok(result.last_insert_rowid())
    }

    pub async fn enqueue_delta(&self, row: &NewDeltaQueueItem) -> Result<i64> {
        let result = sqlx::query(
            "INSERT INTO delta_queue (desired_track_id, action_type, priority, reason, target_quality)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .bind(row.desired_track_id)
        .bind(row.action_type.as_str())
        .bind(row.priority)
        .bind(&row.reason)
        .bind(&row.target_quality)
        .execute(&self.pool)
        .await?;
        Ok(result.last_insert_rowid())
    }

    pub async fn clear_reconciliation(&self) -> Result<()> {
        sqlx::query("DELETE FROM reconciliation_results").execute(&self.pool).await?;
        sqlx::query("DELETE FROM delta_queue WHERE processed_at IS NULL").execute(&self.pool).await?;
        Ok(())
    }

    pub async fn list_desired_tracks(&self) -> Result<Vec<DesiredTrack>> {
        let rows = sqlx::query_as::<_, DesiredTrack>(
            "SELECT id, source_name, source_track_id, source_album_id, source_artist_id,
                    artist_name, album_title, track_title, track_number, disc_number,
                    duration_ms, isrc, raw_payload_json, imported_at
             FROM desired_tracks ORDER BY id",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn list_local_files_for_track(&self, track_id: i64) -> Result<Vec<LocalFile>> {
        let rows = sqlx::query_as::<_, LocalFile>(
            "SELECT id, track_id, file_path, file_name, extension, codec, bitrate, sample_rate,
                    bit_depth, channels, duration_ms, file_size, file_mtime_ms, content_hash,
                    integrity_status, quality_tier, last_scanned_at, created_at, updated_at
             FROM local_files WHERE track_id = ?1 ORDER BY id",
        )
        .bind(track_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn find_track_by_isrc(&self, isrc: &str) -> Result<Option<Track>> {
        let track = sqlx::query_as::<_, Track>(
            "SELECT id, album_id, artist_id, title, normalized_title, track_number, disc_number,
                    duration_ms, isrc, spotify_id, discogs_id, created_at, updated_at
             FROM tracks WHERE isrc = ?1 LIMIT 1",
        )
        .bind(isrc)
        .fetch_optional(&self.pool)
        .await?;
        Ok(track)
    }

    pub async fn strong_match_candidates(
        &self,
        normalized_artist: &str,
        normalized_album: Option<&str>,
        normalized_title: &str,
    ) -> Result<Vec<(Track, LocalFile)>> {
        let rows = sqlx::query(
            "SELECT
                t.id as t_id,
                t.album_id as t_album_id,
                t.artist_id as t_artist_id,
                t.title as t_title,
                t.normalized_title as t_normalized_title,
                t.track_number as t_track_number,
                t.disc_number as t_disc_number,
                t.duration_ms as t_duration_ms,
                t.isrc as t_isrc,
                t.spotify_id as t_spotify_id,
                t.discogs_id as t_discogs_id,
                t.created_at as t_created_at,
                t.updated_at as t_updated_at,
                lf.id as lf_id,
                lf.track_id as lf_track_id,
                lf.file_path as lf_file_path,
                lf.file_name as lf_file_name,
                lf.extension as lf_extension,
                lf.codec as lf_codec,
                lf.bitrate as lf_bitrate,
                lf.sample_rate as lf_sample_rate,
                lf.bit_depth as lf_bit_depth,
                lf.channels as lf_channels,
                lf.duration_ms as lf_duration_ms,
                lf.file_size as lf_file_size,
                lf.file_mtime_ms as lf_file_mtime_ms,
                lf.content_hash as lf_content_hash,
                lf.integrity_status as lf_integrity_status,
                lf.quality_tier as lf_quality_tier,
                lf.last_scanned_at as lf_last_scanned_at,
                lf.created_at as lf_created_at,
                lf.updated_at as lf_updated_at
             FROM tracks t
             JOIN artists a ON a.id = t.artist_id
             LEFT JOIN albums al ON al.id = t.album_id
             JOIN local_files lf ON lf.track_id = t.id
             WHERE a.normalized_name = ?1
               AND t.normalized_title = ?2
               AND (?3 IS NULL OR al.normalized_title = ?3)",
        )
        .bind(normalized_artist)
        .bind(normalized_title)
        .bind(normalized_album)
        .fetch_all(&self.pool)
        .await?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            let track = Track {
                id: row.try_get("t_id")?,
                album_id: row.try_get("t_album_id")?,
                artist_id: row.try_get("t_artist_id")?,
                title: row.try_get("t_title")?,
                normalized_title: row.try_get("t_normalized_title")?,
                track_number: row.try_get("t_track_number")?,
                disc_number: row.try_get("t_disc_number")?,
                duration_ms: row.try_get("t_duration_ms")?,
                isrc: row.try_get("t_isrc")?,
                spotify_id: row.try_get("t_spotify_id")?,
                discogs_id: row.try_get("t_discogs_id")?,
                created_at: row.try_get("t_created_at")?,
                updated_at: row.try_get("t_updated_at")?,
            };
            let local_file = LocalFile {
                id: row.try_get("lf_id")?,
                track_id: row.try_get("lf_track_id")?,
                file_path: row.try_get("lf_file_path")?,
                file_name: row.try_get("lf_file_name")?,
                extension: row.try_get("lf_extension")?,
                codec: row.try_get("lf_codec")?,
                bitrate: row.try_get("lf_bitrate")?,
                sample_rate: row.try_get("lf_sample_rate")?,
                bit_depth: row.try_get("lf_bit_depth")?,
                channels: row.try_get("lf_channels")?,
                duration_ms: row.try_get("lf_duration_ms")?,
                file_size: row.try_get("lf_file_size")?,
                file_mtime_ms: row.try_get("lf_file_mtime_ms")?,
                content_hash: row.try_get("lf_content_hash")?,
                integrity_status: row.try_get("lf_integrity_status")?,
                quality_tier: row.try_get("lf_quality_tier")?,
                last_scanned_at: row.try_get("lf_last_scanned_at")?,
                created_at: row.try_get("lf_created_at")?,
                updated_at: row.try_get("lf_updated_at")?,
            };
            out.push((track, local_file));
        }
        Ok(out)
    }

    pub async fn fuzzy_candidates_for_artist(&self, normalized_artist: &str) -> Result<Vec<(Track, LocalFile)>> {
        let rows = sqlx::query(
            "SELECT
                t.id as t_id,
                t.album_id as t_album_id,
                t.artist_id as t_artist_id,
                t.title as t_title,
                t.normalized_title as t_normalized_title,
                t.track_number as t_track_number,
                t.disc_number as t_disc_number,
                t.duration_ms as t_duration_ms,
                t.isrc as t_isrc,
                t.spotify_id as t_spotify_id,
                t.discogs_id as t_discogs_id,
                t.created_at as t_created_at,
                t.updated_at as t_updated_at,
                lf.id as lf_id,
                lf.track_id as lf_track_id,
                lf.file_path as lf_file_path,
                lf.file_name as lf_file_name,
                lf.extension as lf_extension,
                lf.codec as lf_codec,
                lf.bitrate as lf_bitrate,
                lf.sample_rate as lf_sample_rate,
                lf.bit_depth as lf_bit_depth,
                lf.channels as lf_channels,
                lf.duration_ms as lf_duration_ms,
                lf.file_size as lf_file_size,
                lf.file_mtime_ms as lf_file_mtime_ms,
                lf.content_hash as lf_content_hash,
                lf.integrity_status as lf_integrity_status,
                lf.quality_tier as lf_quality_tier,
                lf.last_scanned_at as lf_last_scanned_at,
                lf.created_at as lf_created_at,
                lf.updated_at as lf_updated_at
             FROM tracks t
             JOIN artists a ON a.id = t.artist_id
             JOIN local_files lf ON lf.track_id = t.id
             WHERE a.normalized_name = ?1",
        )
        .bind(normalized_artist)
        .fetch_all(&self.pool)
        .await?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            let track = Track {
                id: row.try_get("t_id")?,
                album_id: row.try_get("t_album_id")?,
                artist_id: row.try_get("t_artist_id")?,
                title: row.try_get("t_title")?,
                normalized_title: row.try_get("t_normalized_title")?,
                track_number: row.try_get("t_track_number")?,
                disc_number: row.try_get("t_disc_number")?,
                duration_ms: row.try_get("t_duration_ms")?,
                isrc: row.try_get("t_isrc")?,
                spotify_id: row.try_get("t_spotify_id")?,
                discogs_id: row.try_get("t_discogs_id")?,
                created_at: row.try_get("t_created_at")?,
                updated_at: row.try_get("t_updated_at")?,
            };
            let local_file = LocalFile {
                id: row.try_get("lf_id")?,
                track_id: row.try_get("lf_track_id")?,
                file_path: row.try_get("lf_file_path")?,
                file_name: row.try_get("lf_file_name")?,
                extension: row.try_get("lf_extension")?,
                codec: row.try_get("lf_codec")?,
                bitrate: row.try_get("lf_bitrate")?,
                sample_rate: row.try_get("lf_sample_rate")?,
                bit_depth: row.try_get("lf_bit_depth")?,
                channels: row.try_get("lf_channels")?,
                duration_ms: row.try_get("lf_duration_ms")?,
                file_size: row.try_get("lf_file_size")?,
                file_mtime_ms: row.try_get("lf_file_mtime_ms")?,
                content_hash: row.try_get("lf_content_hash")?,
                integrity_status: row.try_get("lf_integrity_status")?,
                quality_tier: row.try_get("lf_quality_tier")?,
                last_scanned_at: row.try_get("lf_last_scanned_at")?,
                created_at: row.try_get("lf_created_at")?,
                updated_at: row.try_get("lf_updated_at")?,
            };
            out.push((track, local_file));
        }
        Ok(out)
    }

    pub async fn find_duplicate_hashes(&self, hash: &str) -> Result<Vec<LocalFile>> {
        let rows = sqlx::query_as::<_, LocalFile>(
            "SELECT id, track_id, file_path, file_name, extension, codec, bitrate, sample_rate,
                    bit_depth, channels, duration_ms, file_size, file_mtime_ms, content_hash,
                    integrity_status, quality_tier, last_scanned_at, created_at, updated_at
             FROM local_files
             WHERE content_hash = ?1",
        )
        .bind(hash)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn audit_counts(&self) -> Result<(i64, i64, i64, i64)> {
        let artists = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM artists")
            .fetch_one(&self.pool)
            .await?;
        let albums = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM albums")
            .fetch_one(&self.pool)
            .await?;
        let tracks = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM tracks")
            .fetch_one(&self.pool)
            .await?;
        let local_files = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM local_files")
            .fetch_one(&self.pool)
            .await?;
        Ok((artists, albums, tracks, local_files))
    }
}
