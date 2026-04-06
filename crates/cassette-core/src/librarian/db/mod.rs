pub mod migrations;

use crate::acquisition::AcquisitionRequest;
use crate::librarian::error::Result;
use crate::librarian::models::{
    AcquisitionRequestEvent, AcquisitionRequestRow, DesiredTrack, LocalFile, LocalFileScanState,
    NewDeltaQueueItem, NewLocalFile, NewReconciliationResult, ScanCheckpoint, Track,
};
use sqlx::sqlite::{
    SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions, SqliteSynchronous,
};
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

        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .foreign_keys(true)
            .synchronous(SqliteSynchronous::Normal);
        let pool = SqlitePoolOptions::new()
            .max_connections(8)
            .connect_with(options)
            .await?;

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
        self.ensure_column_exists(
            "acquisition_requests",
            "musicbrainz_release_group_id",
            "TEXT",
        )
        .await?;
        Ok(())
    }

    async fn ensure_column_exists(
        &self,
        table: &str,
        column: &str,
        definition: &str,
    ) -> Result<()> {
        let pragma = format!("PRAGMA table_info({table})");
        let rows = sqlx::query(&pragma).fetch_all(&self.pool).await?;
        let exists = rows.iter().any(|row| {
            row.try_get::<String, _>("name")
                .map(|name| name.eq_ignore_ascii_case(column))
                .unwrap_or(false)
        });

        if !exists {
            let alter = format!("ALTER TABLE {table} ADD COLUMN {column} {definition}");
            sqlx::query(&alter).execute(&self.pool).await?;
        }

        Ok(())
    }

    pub async fn upsert_canonical_artist(
        &self,
        name: &str,
        musicbrainz_id: Option<&str>,
        spotify_id: Option<&str>,
        discogs_id: Option<&str>,
    ) -> Result<i64> {
        let normalized_name = normalize_acquisition_text(name);
        sqlx::query(
            "INSERT INTO canonical_artists (name, normalized_name, musicbrainz_id, spotify_id, discogs_id)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(normalized_name) DO UPDATE SET
                name = excluded.name,
                musicbrainz_id = COALESCE(excluded.musicbrainz_id, canonical_artists.musicbrainz_id),
                spotify_id = COALESCE(excluded.spotify_id, canonical_artists.spotify_id),
                discogs_id = COALESCE(excluded.discogs_id, canonical_artists.discogs_id),
                updated_at = CURRENT_TIMESTAMP",
        )
        .bind(name)
        .bind(&normalized_name)
        .bind(musicbrainz_id)
        .bind(spotify_id)
        .bind(discogs_id)
        .execute(&self.pool)
        .await?;

        let id = sqlx::query_scalar::<_, i64>(
            "SELECT id FROM canonical_artists WHERE normalized_name = ?1 LIMIT 1",
        )
        .bind(normalized_name)
        .fetch_one(&self.pool)
        .await?;
        Ok(id)
    }

    pub async fn upsert_canonical_release(
        &self,
        canonical_artist_id: i64,
        title: &str,
        release_group_mbid: Option<&str>,
        release_mbid: Option<&str>,
        release_type: Option<&str>,
        year: Option<i64>,
        spotify_id: Option<&str>,
        discogs_id: Option<&str>,
    ) -> Result<i64> {
        let normalized_title = normalize_acquisition_text(title);
        sqlx::query(
            "INSERT INTO canonical_releases (
                canonical_artist_id, title, normalized_title, release_group_mbid, release_mbid,
                spotify_id, discogs_id, release_type, year
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(canonical_artist_id, normalized_title) DO UPDATE SET
                title = excluded.title,
                release_group_mbid = COALESCE(excluded.release_group_mbid, canonical_releases.release_group_mbid),
                release_mbid = COALESCE(excluded.release_mbid, canonical_releases.release_mbid),
                spotify_id = COALESCE(excluded.spotify_id, canonical_releases.spotify_id),
                discogs_id = COALESCE(excluded.discogs_id, canonical_releases.discogs_id),
                release_type = COALESCE(excluded.release_type, canonical_releases.release_type),
                year = COALESCE(excluded.year, canonical_releases.year),
                updated_at = CURRENT_TIMESTAMP",
        )
        .bind(canonical_artist_id)
        .bind(title)
        .bind(&normalized_title)
        .bind(release_group_mbid)
        .bind(release_mbid)
        .bind(spotify_id)
        .bind(discogs_id)
        .bind(release_type)
        .bind(year)
        .execute(&self.pool)
        .await?;

        let id = sqlx::query_scalar::<_, i64>(
            "SELECT id FROM canonical_releases
             WHERE canonical_artist_id = ?1 AND normalized_title = ?2
             LIMIT 1",
        )
        .bind(canonical_artist_id)
        .bind(normalized_title)
        .fetch_one(&self.pool)
        .await?;
        Ok(id)
    }

    pub async fn upsert_canonical_recording(
        &self,
        canonical_artist_id: Option<i64>,
        canonical_release_id: Option<i64>,
        title: &str,
        musicbrainz_recording_id: Option<&str>,
        isrc: Option<&str>,
        track_number: Option<i64>,
        disc_number: Option<i64>,
        duration_ms: Option<i64>,
    ) -> Result<i64> {
        let normalized_title = normalize_acquisition_text(title);
        if let Some(recording_id) = musicbrainz_recording_id {
            sqlx::query(
                "INSERT INTO canonical_recordings (
                    canonical_artist_id, canonical_release_id, title, normalized_title,
                    musicbrainz_recording_id, isrc, track_number, disc_number, duration_ms
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                 ON CONFLICT(musicbrainz_recording_id) DO UPDATE SET
                    canonical_artist_id = COALESCE(excluded.canonical_artist_id, canonical_recordings.canonical_artist_id),
                    canonical_release_id = COALESCE(excluded.canonical_release_id, canonical_recordings.canonical_release_id),
                    title = excluded.title,
                    normalized_title = excluded.normalized_title,
                    isrc = COALESCE(excluded.isrc, canonical_recordings.isrc),
                    track_number = COALESCE(excluded.track_number, canonical_recordings.track_number),
                    disc_number = COALESCE(excluded.disc_number, canonical_recordings.disc_number),
                    duration_ms = COALESCE(excluded.duration_ms, canonical_recordings.duration_ms),
                    updated_at = CURRENT_TIMESTAMP",
            )
            .bind(canonical_artist_id)
            .bind(canonical_release_id)
            .bind(title)
            .bind(&normalized_title)
            .bind(recording_id)
            .bind(isrc)
            .bind(track_number)
            .bind(disc_number)
            .bind(duration_ms)
            .execute(&self.pool)
            .await?;

            let id = sqlx::query_scalar::<_, i64>(
                "SELECT id FROM canonical_recordings WHERE musicbrainz_recording_id = ?1 LIMIT 1",
            )
            .bind(recording_id)
            .fetch_one(&self.pool)
            .await?;
            return Ok(id);
        }

        sqlx::query(
            "INSERT INTO canonical_recordings (
                canonical_artist_id, canonical_release_id, title, normalized_title, isrc,
                track_number, disc_number, duration_ms
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )
        .bind(canonical_artist_id)
        .bind(canonical_release_id)
        .bind(title)
        .bind(normalized_title)
        .bind(isrc)
        .bind(track_number)
        .bind(disc_number)
        .bind(duration_ms)
        .execute(&self.pool)
        .await?;

        Ok(sqlx::query_scalar::<_, i64>("SELECT last_insert_rowid()")
            .fetch_one(&self.pool)
            .await?)
    }

    pub async fn create_acquisition_request(
        &self,
        request: &AcquisitionRequest,
    ) -> Result<AcquisitionRequestRow> {
        let artist_id = if request.canonical_artist_id.is_none() {
            Some(
                self.upsert_canonical_artist(
                    &request.artist,
                    None,
                    request.source_artist_id.as_deref(),
                    None,
                )
                .await?,
            )
        } else {
            request.canonical_artist_id
        };

        let release_id = match (request.canonical_release_id, request.album.as_deref()) {
            (Some(id), _) => Some(id),
            (None, Some(album)) if !album.trim().is_empty() && artist_id.is_some() => Some(
                self.upsert_canonical_release(
                    artist_id.expect("checked is_some"),
                    album,
                    request.musicbrainz_release_group_id.as_deref(),
                    request.musicbrainz_release_id.as_deref(),
                    None,
                    request.year.map(i64::from),
                    request.source_album_id.as_deref(),
                    None,
                )
                .await?,
            ),
            _ => None,
        };

        let request_signature = request
            .request_signature
            .clone()
            .unwrap_or_else(|| request.request_fingerprint());
        let normalized_artist = normalize_acquisition_text(&request.artist);
        let normalized_album = request.album.as_deref().map(normalize_acquisition_text);
        let normalized_title = normalize_acquisition_text(&request.title);
        let excluded_json = (!request.excluded_providers.is_empty())
            .then(|| serde_json::to_string(&request.excluded_providers))
            .transpose()?;
        let task_id = request.effective_task_id();

        sqlx::query(
            "INSERT INTO acquisition_requests (
                scope, source_name, source_track_id, source_album_id, source_artist_id,
                artist, album, title, normalized_artist, normalized_album, normalized_title,
                track_number, disc_number, year, duration_secs, isrc,
                     musicbrainz_recording_id, musicbrainz_release_group_id, musicbrainz_release_id,
                     canonical_artist_id, canonical_release_id,
                strategy, quality_policy, excluded_providers_json, edition_policy, confirmation_policy,
                desired_track_id, source_operation_id, task_id, request_signature, status, raw_payload_json
             ) VALUES (
                ?1, ?2, ?3, ?4, ?5,
                ?6, ?7, ?8, ?9, ?10, ?11,
                ?12, ?13, ?14, ?15, ?16,
                     ?17, ?18, ?19, ?20, ?21,
                     ?22, ?23, ?24, ?25, ?26,
                     ?27, ?28, ?29, ?30, ?31, ?32
             )",
        )
        .bind(request.scope.as_str())
        .bind(&request.source_name)
        .bind(request.source_track_id.as_deref())
        .bind(request.source_album_id.as_deref())
        .bind(request.source_artist_id.as_deref())
        .bind(&request.artist)
        .bind(request.album.as_deref())
        .bind(&request.title)
        .bind(&normalized_artist)
        .bind(normalized_album.as_deref())
        .bind(&normalized_title)
        .bind(request.track_number.map(i64::from))
        .bind(request.disc_number.map(i64::from))
        .bind(request.year.map(i64::from))
        .bind(request.duration_secs)
        .bind(request.isrc.as_deref())
        .bind(request.musicbrainz_recording_id.as_deref())
        .bind(request.musicbrainz_release_group_id.as_deref())
        .bind(request.musicbrainz_release_id.as_deref())
        .bind(artist_id)
        .bind(release_id)
        .bind(request.strategy_name())
        .bind(request.quality_policy.as_deref())
        .bind(excluded_json.as_deref())
        .bind(request.edition_policy.as_deref())
        .bind(request.confirmation_policy.as_str())
        .bind(request.desired_track_id)
        .bind(request.source_operation_id.as_deref())
        .bind(&task_id)
        .bind(&request_signature)
        .bind(request.status.as_str())
        .bind(request.raw_payload_json.as_deref())
        .execute(&self.pool)
        .await?;

        let request_id = sqlx::query_scalar::<_, i64>(
            "SELECT id FROM acquisition_requests WHERE request_signature = ?1 LIMIT 1",
        )
        .bind(&request_signature)
        .fetch_one(&self.pool)
        .await?;

        self.append_acquisition_request_event(
            request_id,
            Some(task_id.as_str()),
            "request_created",
            request.status.as_str(),
            Some("request persisted in control plane"),
            request.raw_payload_json.as_deref(),
        )
        .await?;

        self.get_acquisition_request(request_id)
            .await?
            .ok_or_else(|| {
                crate::librarian::error::LibrarianError::DatabaseError(sqlx::Error::RowNotFound)
            })
    }

    pub async fn append_acquisition_request_event(
        &self,
        request_id: i64,
        task_id: Option<&str>,
        event_type: &str,
        status: &str,
        message: Option<&str>,
        payload_json: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO acquisition_request_events
                (request_id, task_id, event_type, status, message, payload_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind(request_id)
        .bind(task_id)
        .bind(event_type)
        .bind(status)
        .bind(message)
        .bind(payload_json)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_acquisition_request_status_by_task_id(
        &self,
        task_id: &str,
        status: &str,
        event_type: &str,
        message: Option<&str>,
        payload_json: Option<&str>,
    ) -> Result<Option<AcquisitionRequestRow>> {
        let row = self.get_acquisition_request_by_task_id(task_id).await?;
        let Some(row) = row else {
            return Ok(None);
        };

        sqlx::query(
            "UPDATE acquisition_requests
             SET status = ?2, updated_at = CURRENT_TIMESTAMP
             WHERE id = ?1",
        )
        .bind(row.id)
        .bind(status)
        .execute(&self.pool)
        .await?;

        self.append_acquisition_request_event(
            row.id,
            Some(task_id),
            event_type,
            status,
            message,
            payload_json,
        )
        .await?;

        self.get_acquisition_request(row.id).await
    }

    pub async fn list_acquisition_requests(
        &self,
        status: Option<&str>,
        limit: usize,
    ) -> Result<Vec<AcquisitionRequestRow>> {
        let rows = if let Some(status) = status {
            sqlx::query_as::<_, AcquisitionRequestRow>(
                "SELECT id, scope, source_name, source_track_id, source_album_id, source_artist_id,
                        artist, album, title, normalized_artist, normalized_album, normalized_title,
                        track_number, disc_number, year, duration_secs, isrc,
                    musicbrainz_recording_id, musicbrainz_release_group_id, musicbrainz_release_id,
                    canonical_artist_id, canonical_release_id,
                        strategy, quality_policy, excluded_providers_json, edition_policy, confirmation_policy,
                        desired_track_id, source_operation_id, task_id, request_signature, status,
                        raw_payload_json, created_at, updated_at
                 FROM acquisition_requests
                 WHERE status = ?1
                 ORDER BY updated_at DESC, id DESC
                 LIMIT ?2",
            )
            .bind(status)
            .bind(i64::try_from(limit).unwrap_or(i64::MAX))
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, AcquisitionRequestRow>(
                "SELECT id, scope, source_name, source_track_id, source_album_id, source_artist_id,
                        artist, album, title, normalized_artist, normalized_album, normalized_title,
                        track_number, disc_number, year, duration_secs, isrc,
                    musicbrainz_recording_id, musicbrainz_release_group_id, musicbrainz_release_id,
                    canonical_artist_id, canonical_release_id,
                        strategy, quality_policy, excluded_providers_json, edition_policy, confirmation_policy,
                        desired_track_id, source_operation_id, task_id, request_signature, status,
                        raw_payload_json, created_at, updated_at
                 FROM acquisition_requests
                 ORDER BY updated_at DESC, id DESC
                 LIMIT ?1",
            )
            .bind(i64::try_from(limit).unwrap_or(i64::MAX))
            .fetch_all(&self.pool)
            .await?
        };

        Ok(rows)
    }

    pub async fn get_acquisition_request(
        &self,
        request_id: i64,
    ) -> Result<Option<AcquisitionRequestRow>> {
        let row = sqlx::query_as::<_, AcquisitionRequestRow>(
            "SELECT id, scope, source_name, source_track_id, source_album_id, source_artist_id,
                    artist, album, title, normalized_artist, normalized_album, normalized_title,
                    track_number, disc_number, year, duration_secs, isrc,
                    musicbrainz_recording_id, musicbrainz_release_group_id, musicbrainz_release_id,
                    canonical_artist_id, canonical_release_id,
                    strategy, quality_policy, excluded_providers_json, edition_policy, confirmation_policy,
                    desired_track_id, source_operation_id, task_id, request_signature, status,
                    raw_payload_json, created_at, updated_at
             FROM acquisition_requests
             WHERE id = ?1
             LIMIT 1",
        )
        .bind(request_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn get_acquisition_request_by_task_id(
        &self,
        task_id: &str,
    ) -> Result<Option<AcquisitionRequestRow>> {
        let row = sqlx::query_as::<_, AcquisitionRequestRow>(
            "SELECT id, scope, source_name, source_track_id, source_album_id, source_artist_id,
                    artist, album, title, normalized_artist, normalized_album, normalized_title,
                    track_number, disc_number, year, duration_secs, isrc,
                    musicbrainz_recording_id, musicbrainz_release_group_id, musicbrainz_release_id,
                    canonical_artist_id, canonical_release_id,
                    strategy, quality_policy, excluded_providers_json, edition_policy, confirmation_policy,
                    desired_track_id, source_operation_id, task_id, request_signature, status,
                    raw_payload_json, created_at, updated_at
             FROM acquisition_requests
             WHERE task_id = ?1
             LIMIT 1",
        )
        .bind(task_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn get_acquisition_request_by_signature(
        &self,
        request_signature: &str,
    ) -> Result<Option<AcquisitionRequestRow>> {
        let row = sqlx::query_as::<_, AcquisitionRequestRow>(
            "SELECT id, scope, source_name, source_track_id, source_album_id, source_artist_id,
                    artist, album, title, normalized_artist, normalized_album, normalized_title,
                    track_number, disc_number, year, duration_secs, isrc,
                    musicbrainz_recording_id, musicbrainz_release_group_id, musicbrainz_release_id,
                    canonical_artist_id, canonical_release_id,
                    strategy, quality_policy, excluded_providers_json, edition_policy, confirmation_policy,
                    desired_track_id, source_operation_id, task_id, request_signature, status,
                    raw_payload_json, created_at, updated_at
             FROM acquisition_requests
             WHERE request_signature = ?1
             LIMIT 1",
        )
        .bind(request_signature)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn get_acquisition_request_timeline(
        &self,
        request_id: i64,
    ) -> Result<Vec<AcquisitionRequestEvent>> {
        let rows = sqlx::query_as::<_, AcquisitionRequestEvent>(
            "SELECT id, request_id, task_id, event_type, status, message, payload_json, created_at
             FROM acquisition_request_events
             WHERE request_id = ?1
             ORDER BY created_at ASC, id ASC",
        )
        .bind(request_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
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
                channels, duration_ms, file_size, file_mtime_ms, content_hash, acoustid_fingerprint,
                integrity_status, quality_tier, last_scanned_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, CURRENT_TIMESTAMP)
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
                acoustid_fingerprint = CASE
                    WHEN COALESCE(excluded.file_mtime_ms, -1) <> COALESCE(local_files.file_mtime_ms, -1)
                        THEN excluded.acoustid_fingerprint
                    ELSE COALESCE(excluded.acoustid_fingerprint, local_files.acoustid_fingerprint)
                END,
                fingerprint_attempted_at = CASE
                    WHEN COALESCE(excluded.file_mtime_ms, -1) <> COALESCE(local_files.file_mtime_ms, -1)
                        THEN NULL
                    ELSE local_files.fingerprint_attempted_at
                END,
                fingerprint_error = CASE
                    WHEN COALESCE(excluded.file_mtime_ms, -1) <> COALESCE(local_files.file_mtime_ms, -1)
                        THEN NULL
                    ELSE local_files.fingerprint_error
                END,
                fingerprint_source_mtime_ms = CASE
                    WHEN COALESCE(excluded.file_mtime_ms, -1) <> COALESCE(local_files.file_mtime_ms, -1)
                        THEN NULL
                    WHEN excluded.acoustid_fingerprint IS NOT NULL AND excluded.acoustid_fingerprint != ''
                        THEN excluded.file_mtime_ms
                    ELSE local_files.fingerprint_source_mtime_ms
                END,
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
        .bind(&file.acoustid_fingerprint)
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

    pub async fn get_local_file_scan_state(
        &self,
        path: &str,
    ) -> Result<Option<LocalFileScanState>> {
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
            let status = sqlx::query_scalar::<_, String>(
                "SELECT status FROM scan_checkpoints WHERE root_path = ?1 LIMIT 1",
            )
            .bind(root)
            .fetch_optional(&self.pool)
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

    pub async fn clear_desired_tracks_for_source(&self, source_name: &str) -> Result<u64> {
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "DELETE FROM delta_queue
             WHERE desired_track_id IN (
                 SELECT id FROM desired_tracks WHERE source_name = ?1
             )",
        )
        .bind(source_name)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            "DELETE FROM reconciliation_results
             WHERE desired_track_id IN (
                 SELECT id FROM desired_tracks WHERE source_name = ?1
             )",
        )
        .bind(source_name)
        .execute(&mut *tx)
        .await?;

        let result = sqlx::query("DELETE FROM desired_tracks WHERE source_name = ?1")
            .bind(source_name)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(result.rows_affected())
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
        sqlx::query("DELETE FROM reconciliation_results")
            .execute(&self.pool)
            .await?;
        // Preserve claimed rows so mid-flight coordinator work is not wiped.
        sqlx::query("DELETE FROM delta_queue WHERE processed_at IS NULL AND claimed_at IS NULL")
            .execute(&self.pool)
            .await?;
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
                    acoustid_fingerprint, fingerprint_attempted_at, fingerprint_error, fingerprint_source_mtime_ms,
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
                lf.acoustid_fingerprint as lf_acoustid_fingerprint,
                lf.fingerprint_attempted_at as lf_fingerprint_attempted_at,
                lf.fingerprint_error as lf_fingerprint_error,
                lf.fingerprint_source_mtime_ms as lf_fingerprint_source_mtime_ms,
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
                acoustid_fingerprint: row.try_get("lf_acoustid_fingerprint")?,
                fingerprint_attempted_at: row.try_get("lf_fingerprint_attempted_at")?,
                fingerprint_error: row.try_get("lf_fingerprint_error")?,
                fingerprint_source_mtime_ms: row.try_get("lf_fingerprint_source_mtime_ms")?,
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

    pub async fn fuzzy_candidates_for_artist(
        &self,
        normalized_artist: &str,
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
                lf.acoustid_fingerprint as lf_acoustid_fingerprint,
                lf.fingerprint_attempted_at as lf_fingerprint_attempted_at,
                lf.fingerprint_error as lf_fingerprint_error,
                lf.fingerprint_source_mtime_ms as lf_fingerprint_source_mtime_ms,
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
                acoustid_fingerprint: row.try_get("lf_acoustid_fingerprint")?,
                fingerprint_attempted_at: row.try_get("lf_fingerprint_attempted_at")?,
                fingerprint_error: row.try_get("lf_fingerprint_error")?,
                fingerprint_source_mtime_ms: row.try_get("lf_fingerprint_source_mtime_ms")?,
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
                    acoustid_fingerprint, fingerprint_attempted_at, fingerprint_error, fingerprint_source_mtime_ms,
                    integrity_status, quality_tier, last_scanned_at, created_at, updated_at
             FROM local_files
             WHERE content_hash = ?1",
        )
        .bind(hash)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn list_local_files_missing_fingerprint(
        &self,
        limit: usize,
    ) -> Result<Vec<LocalFile>> {
        let rows = sqlx::query_as::<_, LocalFile>(
            "SELECT id, track_id, file_path, file_name, extension, codec, bitrate, sample_rate,
                    bit_depth, channels, duration_ms, file_size, file_mtime_ms, content_hash,
                    acoustid_fingerprint, fingerprint_attempted_at, fingerprint_error, fingerprint_source_mtime_ms,
                    integrity_status, quality_tier, last_scanned_at, created_at, updated_at
             FROM local_files
             WHERE COALESCE(acoustid_fingerprint, '') = ''
               AND integrity_status IN ('readable', 'partial_metadata', 'suspicious')
               AND (
                    fingerprint_source_mtime_ms IS NULL
                    OR COALESCE(file_mtime_ms, -1) <> COALESCE(fingerprint_source_mtime_ms, -1)
               )
             ORDER BY file_size ASC, COALESCE(last_scanned_at, created_at) DESC, file_path ASC
             LIMIT ?1",
        )
        .bind(i64::try_from(limit).unwrap_or(i64::MAX))
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn set_local_file_fingerprint(
        &self,
        local_file_id: i64,
        acoustid_fingerprint: &str,
        fingerprint_source_mtime_ms: Option<i64>,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE local_files
             SET acoustid_fingerprint = ?1,
                 fingerprint_attempted_at = CURRENT_TIMESTAMP,
                 fingerprint_error = NULL,
                 fingerprint_source_mtime_ms = ?2,
                 updated_at = CURRENT_TIMESTAMP
             WHERE id = ?3",
        )
        .bind(acoustid_fingerprint)
        .bind(fingerprint_source_mtime_ms)
        .bind(local_file_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn mark_local_file_fingerprint_failure(
        &self,
        local_file_id: i64,
        fingerprint_source_mtime_ms: Option<i64>,
        fingerprint_error: &str,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE local_files
             SET acoustid_fingerprint = NULL,
                 fingerprint_attempted_at = CURRENT_TIMESTAMP,
                 fingerprint_error = ?1,
                 fingerprint_source_mtime_ms = ?2,
                 updated_at = CURRENT_TIMESTAMP
             WHERE id = ?3",
        )
        .bind(fingerprint_error)
        .bind(fingerprint_source_mtime_ms)
        .bind(local_file_id)
        .execute(&self.pool)
        .await?;
        Ok(())
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

fn normalize_acquisition_text(value: &str) -> String {
    crate::identity::normalize_identity_text(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acquisition::{
        AcquisitionRequest, AcquisitionRequestStatus, AcquisitionScope, ConfirmationPolicy,
    };
    use crate::director::models::{AcquisitionStrategy, TrackTaskSource};
    async fn test_db() -> LibrarianDb {
        let dir = std::path::PathBuf::from("target")
            .join(format!("cassette-librarian-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        let path = dir.join("librarian-test.db");
        LibrarianDb::connect(&path).await.expect("connect")
    }

    fn sample_request() -> AcquisitionRequest {
        AcquisitionRequest {
            id: None,
            scope: AcquisitionScope::Track,
            source: TrackTaskSource::Manual,
            source_name: "manual".to_string(),
            source_track_id: None,
            source_album_id: None,
            source_artist_id: None,
            artist: "Artist".to_string(),
            album: Some("Album".to_string()),
            title: "Song".to_string(),
            track_number: Some(1),
            disc_number: Some(1),
            year: Some(2024),
            duration_secs: Some(42.0),
            isrc: Some("US1234567890".to_string()),
            musicbrainz_recording_id: None,
            musicbrainz_release_group_id: Some("mb-release-group-1".to_string()),
            musicbrainz_release_id: Some("mb-release-1".to_string()),
            canonical_artist_id: None,
            canonical_release_id: None,
            strategy: AcquisitionStrategy::Standard,
            quality_policy: Some("lossless_preferred".to_string()),
            excluded_providers: vec!["yt_dlp".to_string()],
            edition_policy: Some("standard_only".to_string()),
            confirmation_policy: ConfirmationPolicy::Automatic,
            desired_track_id: Some(7),
            source_operation_id: Some("op-1".to_string()),
            task_id: Some("task-1".to_string()),
            request_signature: Some("sig-1".to_string()),
            status: AcquisitionRequestStatus::Pending,
            raw_payload_json: Some("{\"kind\":\"test\"}".to_string()),
        }
    }

    #[tokio::test]
    async fn acquisition_request_persists_and_records_timeline() {
        let db = test_db().await;
        let row = db
            .create_acquisition_request(&sample_request())
            .await
            .expect("request should persist");

        assert_eq!(row.artist, "Artist");
        assert_eq!(row.task_id.as_deref(), Some("task-1"));
        assert_eq!(row.request_signature, "sig-1");

        let timeline = db
            .get_acquisition_request_timeline(row.id)
            .await
            .expect("timeline should load");
        assert_eq!(timeline.len(), 1);
        assert_eq!(timeline[0].event_type, "request_created");
        assert_eq!(timeline[0].status, "pending");
    }

    #[tokio::test]
    async fn acquisition_request_status_updates_by_task_id() {
        let db = test_db().await;
        let row = db
            .create_acquisition_request(&sample_request())
            .await
            .expect("request should persist");

        let updated = db
            .update_acquisition_request_status_by_task_id(
                "task-1",
                "submitted",
                "director_submitted",
                Some("submitted to director"),
                None,
            )
            .await
            .expect("status update should succeed")
            .expect("request should exist");

        assert_eq!(updated.status, "submitted");

        let requests = db
            .list_acquisition_requests(Some("submitted"), 10)
            .await
            .expect("requests should load");
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].id, row.id);
    }

    #[tokio::test]
    async fn acquisition_request_contract_persists_all_scopes_and_confirmation_policies() {
        let db = test_db().await;
        let scopes = [
            AcquisitionScope::Track,
            AcquisitionScope::Album,
            AcquisitionScope::Artist,
            AcquisitionScope::Discography,
            AcquisitionScope::SelectedAlbums,
        ];
        let confirmation_policies = [
            ConfirmationPolicy::Automatic,
            ConfirmationPolicy::Advisory,
            ConfirmationPolicy::ManualReview,
        ];

        let mut index = 0usize;
        for scope in scopes {
            for policy in confirmation_policies {
                index += 1;
                let mut request = sample_request();
                request.scope = scope;
                request.confirmation_policy = policy;
                request.task_id = Some(format!("task-contract-{index}"));
                request.request_signature = Some(format!("sig-contract-{index}"));
                request.musicbrainz_release_group_id = Some(format!("mb-release-group-{index}"));
                request.musicbrainz_release_id = Some(format!("mb-release-{index}"));
                request.musicbrainz_recording_id = Some(format!("mb-recording-{index}"));
                request.quality_policy = Some("lossless_preferred".to_string());
                request.edition_policy = Some("standard_only".to_string());
                request.excluded_providers = vec!["yt_dlp".to_string(), "usenet".to_string()];

                let row = db
                    .create_acquisition_request(&request)
                    .await
                    .expect("request should persist");

                assert_eq!(row.scope, scope.as_str());
                assert_eq!(row.confirmation_policy, policy.as_str());
                assert_eq!(
                    row.musicbrainz_release_group_id.as_deref(),
                    Some(format!("mb-release-group-{index}").as_str())
                );
                assert_eq!(
                    row.musicbrainz_release_id.as_deref(),
                    Some(format!("mb-release-{index}").as_str())
                );
                assert_eq!(
                    row.musicbrainz_recording_id.as_deref(),
                    Some(format!("mb-recording-{index}").as_str())
                );
                assert_eq!(row.quality_policy.as_deref(), Some("lossless_preferred"));
                assert_eq!(row.edition_policy.as_deref(), Some("standard_only"));
                assert_eq!(row.status, "pending");

                let excluded: Vec<String> = serde_json::from_str(
                    row.excluded_providers_json
                        .as_deref()
                        .expect("excluded providers should persist as json"),
                )
                .expect("excluded providers json should parse");
                assert_eq!(excluded, vec!["yt_dlp".to_string(), "usenet".to_string()]);
            }
        }
    }

    #[tokio::test]
    async fn has_completed_checkpoints_returns_false_when_checkpoint_missing() {
        let db = test_db().await;

        let completed = db
            .has_completed_checkpoints(&["C:/missing-root".to_string()])
            .await
            .expect("checkpoint lookup should not error");

        assert!(!completed);
    }
}
