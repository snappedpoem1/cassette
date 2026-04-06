use anyhow::{anyhow, Context, Result};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use std::fs;
use std::path::{Path, PathBuf};

fn app_data_dir() -> Result<PathBuf> {
    let app_data = std::env::var("APPDATA").context("APPDATA is not set")?;
    Ok(PathBuf::from(app_data).join("dev.cassette.app"))
}

fn default_runtime_db() -> Result<PathBuf> {
    Ok(app_data_dir()?.join("cassette.db"))
}

fn default_sidecar_db(runtime_db: &Path) -> Result<PathBuf> {
    let parent = runtime_db
        .parent()
        .ok_or_else(|| anyhow!("runtime db has no parent directory"))?;
    Ok(parent.join("cassette_librarian.db"))
}

fn default_output_db(runtime_db: &Path) -> Result<PathBuf> {
    let parent = runtime_db
        .parent()
        .ok_or_else(|| anyhow!("runtime db has no parent directory"))?;
    Ok(parent.join("cassette_unified.db"))
}

fn parse_flag_value(args: &[String], flag: &str) -> Option<String> {
    args.windows(2)
        .find(|window| window[0] == flag)
        .map(|window| window[1].clone())
}

fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|arg| arg == flag)
}

fn sqlite_literal(path: &Path) -> String {
    path.to_string_lossy().replace('\'', "''")
}

async fn ensure_sidecar_column(
    pool: &sqlx::SqlitePool,
    table: &str,
    column: &str,
    column_type: &str,
) -> Result<()> {
    let sql = format!("ALTER TABLE sidecar.{table} ADD COLUMN {column} {column_type}",);
    match sqlx::query(&sql).execute(pool).await {
        Ok(_) => Ok(()),
        Err(error) => {
            let message = error.to_string().to_ascii_lowercase();
            if message.contains("duplicate column name") {
                Ok(())
            } else {
                Err(error.into())
            }
        }
    }
}

async fn sidecar_table_exists(pool: &sqlx::SqlitePool, table: &str) -> Result<bool> {
    if !table
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
    {
        return Err(anyhow!("invalid table name: {table}"));
    }

    let pragma = format!("PRAGMA sidecar.table_info({table})");
    let rows = sqlx::query(&pragma).fetch_all(pool).await?;
    Ok(!rows.is_empty())
}

async fn create_unified_control_tables(pool: &sqlx::SqlitePool) -> Result<()> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS control_artists (
            id INTEGER PRIMARY KEY,
            canonical_name TEXT NOT NULL,
            normalized_name TEXT NOT NULL,
            spotify_id TEXT,
            discogs_id TEXT,
            created_at TEXT,
            updated_at TEXT
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS control_albums (
            id INTEGER PRIMARY KEY,
            artist_id INTEGER NOT NULL REFERENCES control_artists(id),
            title TEXT NOT NULL,
            normalized_title TEXT NOT NULL,
            release_date TEXT,
            spotify_id TEXT,
            discogs_id TEXT,
            cover_art_path TEXT,
            created_at TEXT,
            updated_at TEXT
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS control_tracks (
            id INTEGER PRIMARY KEY,
            album_id INTEGER REFERENCES control_albums(id),
            artist_id INTEGER NOT NULL REFERENCES control_artists(id),
            title TEXT NOT NULL,
            normalized_title TEXT NOT NULL,
            track_number INTEGER,
            disc_number INTEGER,
            duration_ms INTEGER,
            isrc TEXT,
            spotify_id TEXT,
            discogs_id TEXT,
            created_at TEXT,
            updated_at TEXT
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS control_local_files (
            id INTEGER PRIMARY KEY,
            track_id INTEGER REFERENCES control_tracks(id),
            file_path TEXT NOT NULL,
            file_name TEXT NOT NULL,
            extension TEXT NOT NULL,
            codec TEXT,
            bitrate INTEGER,
            sample_rate INTEGER,
            bit_depth INTEGER,
            channels INTEGER,
            duration_ms INTEGER,
            file_size INTEGER,
            file_mtime_ms INTEGER,
            content_hash TEXT,
            acoustid_fingerprint TEXT,
            fingerprint_attempted_at TEXT,
            fingerprint_error TEXT,
            fingerprint_source_mtime_ms INTEGER,
            integrity_status TEXT NOT NULL,
            quality_tier TEXT,
            last_scanned_at TEXT,
            created_at TEXT,
            updated_at TEXT
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS control_scan_checkpoints (
            id INTEGER PRIMARY KEY,
            root_path TEXT NOT NULL,
            last_run_id TEXT,
            last_scanned_path TEXT,
            status TEXT NOT NULL,
            files_seen INTEGER,
            files_indexed INTEGER,
            created_at TEXT,
            updated_at TEXT
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS control_desired_tracks (
            id INTEGER PRIMARY KEY,
            source_name TEXT NOT NULL,
            source_track_id TEXT,
            source_album_id TEXT,
            source_artist_id TEXT,
            artist_name TEXT NOT NULL,
            album_title TEXT,
            track_title TEXT NOT NULL,
            track_number INTEGER,
            disc_number INTEGER,
            duration_ms INTEGER,
            isrc TEXT,
            raw_payload_json TEXT,
            imported_at TEXT
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS control_reconciliation_results (
            id INTEGER PRIMARY KEY,
            desired_track_id INTEGER NOT NULL REFERENCES control_desired_tracks(id),
            matched_track_id INTEGER REFERENCES control_tracks(id),
            matched_local_file_id INTEGER REFERENCES control_local_files(id),
            reconciliation_status TEXT NOT NULL,
            quality_assessment TEXT,
            reason TEXT NOT NULL,
            created_at TEXT
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS control_delta_queue (
            id INTEGER PRIMARY KEY,
            desired_track_id INTEGER NOT NULL REFERENCES control_desired_tracks(id),
            action_type TEXT NOT NULL,
            priority INTEGER,
            reason TEXT NOT NULL,
            target_quality TEXT,
            source_operation_id TEXT,
            created_at TEXT,
            claimed_at TEXT,
            claim_run_id TEXT,
            processed_at TEXT
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS control_sync_runs (
            id INTEGER PRIMARY KEY,
            run_id TEXT NOT NULL,
            started_at TEXT NOT NULL,
            ended_at TEXT,
            status TEXT NOT NULL,
            phase_reached TEXT NOT NULL,
            files_scanned INTEGER,
            files_upserted INTEGER,
            desired_tracks_imported INTEGER,
            reconciliation_completed INTEGER,
            delta_queue_entries INTEGER,
            error_message TEXT,
            created_at TEXT
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS control_acquisition_requests (
            id INTEGER PRIMARY KEY,
            scope TEXT NOT NULL,
            source_name TEXT NOT NULL,
            source_track_id TEXT,
            source_album_id TEXT,
            source_artist_id TEXT,
            artist TEXT NOT NULL,
            album TEXT,
            title TEXT NOT NULL,
            normalized_artist TEXT NOT NULL,
            normalized_album TEXT,
            normalized_title TEXT NOT NULL,
            track_number INTEGER,
            disc_number INTEGER,
            year INTEGER,
            duration_secs REAL,
            isrc TEXT,
            musicbrainz_recording_id TEXT,
            musicbrainz_release_id TEXT,
            canonical_artist_id INTEGER,
            canonical_release_id INTEGER,
            strategy TEXT NOT NULL,
            quality_policy TEXT,
            excluded_providers_json TEXT,
            edition_policy TEXT,
            confirmation_policy TEXT,
            desired_track_id INTEGER,
            source_operation_id TEXT,
            task_id TEXT,
            request_signature TEXT,
            status TEXT NOT NULL,
            raw_payload_json TEXT,
            created_at TEXT,
            updated_at TEXT
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS control_acquisition_request_events (
            id INTEGER PRIMARY KEY,
            request_id INTEGER NOT NULL REFERENCES control_acquisition_requests(id) ON DELETE CASCADE,
            task_id TEXT,
            event_type TEXT NOT NULL,
            status TEXT NOT NULL,
            message TEXT,
            payload_json TEXT,
            created_at TEXT
        )",
    )
    .execute(pool)
    .await?;

    Ok(())
}

async fn merge_sidecar_into_unified(pool: &sqlx::SqlitePool, sidecar_path: &Path) -> Result<()> {
    let attach = format!(
        "ATTACH DATABASE '{}' AS sidecar",
        sqlite_literal(sidecar_path)
    );
    sqlx::query(&attach).execute(pool).await?;

    let has_artists = sidecar_table_exists(pool, "artists").await?;
    let has_albums = sidecar_table_exists(pool, "albums").await?;
    let has_tracks = sidecar_table_exists(pool, "tracks").await?;
    let has_local_files = sidecar_table_exists(pool, "local_files").await?;
    let has_scan_checkpoints = sidecar_table_exists(pool, "scan_checkpoints").await?;
    let has_desired_tracks = sidecar_table_exists(pool, "desired_tracks").await?;
    let has_reconciliation_results = sidecar_table_exists(pool, "reconciliation_results").await?;
    let has_delta_queue = sidecar_table_exists(pool, "delta_queue").await?;
    let has_sync_runs = sidecar_table_exists(pool, "sync_runs").await?;
    let has_acquisition_requests = sidecar_table_exists(pool, "acquisition_requests").await?;
    let has_acquisition_request_events =
        sidecar_table_exists(pool, "acquisition_request_events").await?;

    // Older sidecar files can predate some hardening columns. Add nullable columns so
    // convergence stays backwards-compatible and does not fail on SELECT projection.
    if has_local_files {
        ensure_sidecar_column(pool, "local_files", "file_mtime_ms", "INTEGER").await?;
        ensure_sidecar_column(pool, "local_files", "acoustid_fingerprint", "TEXT").await?;
        ensure_sidecar_column(pool, "local_files", "fingerprint_attempted_at", "TIMESTAMP").await?;
        ensure_sidecar_column(pool, "local_files", "fingerprint_error", "TEXT").await?;
        ensure_sidecar_column(
            pool,
            "local_files",
            "fingerprint_source_mtime_ms",
            "INTEGER",
        )
        .await?;
    }
    if has_delta_queue {
        ensure_sidecar_column(pool, "delta_queue", "source_operation_id", "TEXT").await?;
        ensure_sidecar_column(pool, "delta_queue", "claimed_at", "TIMESTAMP").await?;
        ensure_sidecar_column(pool, "delta_queue", "claim_run_id", "TEXT").await?;
        ensure_sidecar_column(pool, "delta_queue", "processed_at", "TIMESTAMP").await?;
    }

    let mut tx = pool.begin().await?;

    if has_artists {
        sqlx::query(
        "INSERT OR IGNORE INTO control_artists (id, canonical_name, normalized_name, spotify_id, discogs_id, created_at, updated_at)
         SELECT id, canonical_name, normalized_name, spotify_id, discogs_id, created_at, updated_at
         FROM sidecar.artists",
    )
    .execute(&mut *tx)
    .await?;
    }

    if has_albums {
        sqlx::query(
        "INSERT OR IGNORE INTO control_albums (id, artist_id, title, normalized_title, release_date, spotify_id, discogs_id, cover_art_path, created_at, updated_at)
         SELECT id, artist_id, title, normalized_title, release_date, spotify_id, discogs_id, cover_art_path, created_at, updated_at
         FROM sidecar.albums",
    )
    .execute(&mut *tx)
    .await?;
    }

    if has_tracks {
        sqlx::query(
        "INSERT OR IGNORE INTO control_tracks (id, album_id, artist_id, title, normalized_title, track_number, disc_number, duration_ms, isrc, spotify_id, discogs_id, created_at, updated_at)
         SELECT id, album_id, artist_id, title, normalized_title, track_number, disc_number, duration_ms, isrc, spotify_id, discogs_id, created_at, updated_at
         FROM sidecar.tracks",
    )
    .execute(&mut *tx)
    .await?;
    }

    if has_local_files {
        sqlx::query(
        "INSERT OR IGNORE INTO control_local_files (id, track_id, file_path, file_name, extension, codec, bitrate, sample_rate, bit_depth, channels, duration_ms, file_size, file_mtime_ms, content_hash, acoustid_fingerprint, fingerprint_attempted_at, fingerprint_error, fingerprint_source_mtime_ms, integrity_status, quality_tier, last_scanned_at, created_at, updated_at)
         SELECT id, track_id, file_path, file_name, extension, codec, bitrate, sample_rate, bit_depth, channels, duration_ms, file_size, file_mtime_ms, content_hash, acoustid_fingerprint, fingerprint_attempted_at, fingerprint_error, fingerprint_source_mtime_ms, integrity_status, quality_tier, last_scanned_at, created_at, updated_at
         FROM sidecar.local_files",
    )
    .execute(&mut *tx)
    .await?;
    }

    if has_scan_checkpoints {
        sqlx::query(
        "INSERT OR IGNORE INTO control_scan_checkpoints (id, root_path, last_run_id, last_scanned_path, status, files_seen, files_indexed, created_at, updated_at)
         SELECT id, root_path, last_run_id, last_scanned_path, status, files_seen, files_indexed, created_at, updated_at
         FROM sidecar.scan_checkpoints",
    )
    .execute(&mut *tx)
    .await?;
    }

    if has_desired_tracks {
        sqlx::query(
        "INSERT OR IGNORE INTO control_desired_tracks (id, source_name, source_track_id, source_album_id, source_artist_id, artist_name, album_title, track_title, track_number, disc_number, duration_ms, isrc, raw_payload_json, imported_at)
         SELECT id, source_name, source_track_id, source_album_id, source_artist_id, artist_name, album_title, track_title, track_number, disc_number, duration_ms, isrc, raw_payload_json, imported_at
         FROM sidecar.desired_tracks",
    )
    .execute(&mut *tx)
    .await?;
    }

    if has_reconciliation_results {
        sqlx::query(
        "INSERT OR IGNORE INTO control_reconciliation_results (id, desired_track_id, matched_track_id, matched_local_file_id, reconciliation_status, quality_assessment, reason, created_at)
         SELECT id, desired_track_id, matched_track_id, matched_local_file_id, reconciliation_status, quality_assessment, reason, created_at
         FROM sidecar.reconciliation_results",
    )
    .execute(&mut *tx)
    .await?;
    }

    if has_delta_queue {
        sqlx::query(
        "INSERT OR IGNORE INTO control_delta_queue (id, desired_track_id, action_type, priority, reason, target_quality, source_operation_id, created_at, claimed_at, claim_run_id, processed_at)
         SELECT id, desired_track_id, action_type, priority, reason, target_quality, source_operation_id, created_at, claimed_at, claim_run_id, processed_at
         FROM sidecar.delta_queue",
    )
    .execute(&mut *tx)
    .await?;
    }

    if has_sync_runs {
        sqlx::query(
        "INSERT OR IGNORE INTO control_sync_runs (id, run_id, started_at, ended_at, status, phase_reached, files_scanned, files_upserted, desired_tracks_imported, reconciliation_completed, delta_queue_entries, error_message, created_at)
         SELECT id, run_id, started_at, ended_at, status, phase_reached, files_scanned, files_upserted, desired_tracks_imported, reconciliation_completed, delta_queue_entries, error_message, created_at
         FROM sidecar.sync_runs",
    )
    .execute(&mut *tx)
    .await?;
    }

    if has_acquisition_requests {
        sqlx::query(
            "INSERT OR IGNORE INTO control_acquisition_requests (id, scope, source_name, source_track_id, source_album_id, source_artist_id, artist, album, title, normalized_artist, normalized_album, normalized_title, track_number, disc_number, year, duration_secs, isrc, musicbrainz_recording_id, musicbrainz_release_id, canonical_artist_id, canonical_release_id, strategy, quality_policy, excluded_providers_json, edition_policy, confirmation_policy, desired_track_id, source_operation_id, task_id, request_signature, status, raw_payload_json, created_at, updated_at)
             SELECT id, scope, source_name, source_track_id, source_album_id, source_artist_id, artist, album, title, normalized_artist, normalized_album, normalized_title, track_number, disc_number, year, duration_secs, isrc, musicbrainz_recording_id, musicbrainz_release_id, canonical_artist_id, canonical_release_id, strategy, quality_policy, excluded_providers_json, edition_policy, confirmation_policy, desired_track_id, source_operation_id, task_id, request_signature, status, raw_payload_json, created_at, updated_at
             FROM sidecar.acquisition_requests",
        )
        .execute(&mut *tx)
        .await?;
    }

    if has_acquisition_request_events {
        sqlx::query(
            "INSERT OR IGNORE INTO control_acquisition_request_events (id, request_id, task_id, event_type, status, message, payload_json, created_at)
             SELECT id, request_id, task_id, event_type, status, message, payload_json, created_at
             FROM sidecar.acquisition_request_events",
        )
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    sqlx::query("DETACH DATABASE sidecar").execute(pool).await?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = std::env::args().collect::<Vec<_>>();

    let runtime_db = parse_flag_value(&args, "--runtime-db")
        .map(PathBuf::from)
        .unwrap_or(default_runtime_db()?);
    let sidecar_db = parse_flag_value(&args, "--sidecar-db")
        .map(PathBuf::from)
        .unwrap_or(default_sidecar_db(&runtime_db)?);
    let output_db = parse_flag_value(&args, "--output-db")
        .map(PathBuf::from)
        .unwrap_or(default_output_db(&runtime_db)?);
    let overwrite = has_flag(&args, "--overwrite");

    if !runtime_db.exists() {
        return Err(anyhow!("runtime db not found: {}", runtime_db.display()));
    }
    if !sidecar_db.exists() {
        return Err(anyhow!("sidecar db not found: {}", sidecar_db.display()));
    }

    if runtime_db == output_db {
        return Err(anyhow!("--output-db must differ from --runtime-db"));
    }

    if let Some(parent) = output_db.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create output directory: {}", parent.display()))?;
    }

    if output_db.exists() {
        if overwrite {
            fs::remove_file(&output_db)
                .with_context(|| format!("failed to remove output db: {}", output_db.display()))?;
        } else {
            return Err(anyhow!(
                "output db already exists: {} (use --overwrite)",
                output_db.display()
            ));
        }
    }

    fs::copy(&runtime_db, &output_db).with_context(|| {
        format!(
            "failed to seed unified db from runtime db: {} -> {}",
            runtime_db.display(),
            output_db.display()
        )
    })?;

    let options = SqliteConnectOptions::new()
        .filename(&output_db)
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await?;

    create_unified_control_tables(&pool).await?;
    merge_sidecar_into_unified(&pool, &sidecar_db).await?;

    let desired_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM control_desired_tracks")
        .fetch_one(&pool)
        .await?;
    let delta_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM control_delta_queue")
        .fetch_one(&pool)
        .await?;
    let request_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM control_acquisition_requests")
            .fetch_one(&pool)
            .await?;

    println!(
        "Unified db ready at {} (desired_tracks={}, delta_queue={}, acquisition_requests={})",
        output_db.display(),
        desired_count,
        delta_count,
        request_count
    );

    Ok(())
}
