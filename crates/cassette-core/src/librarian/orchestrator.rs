use crate::librarian::config::{LibrarianConfig, ScanMode};
use crate::librarian::db::{migrations, LibrarianDb};
use crate::librarian::error::{LibrarianError, Result};
use crate::librarian::import::import_desired_spotify_json;
use crate::librarian::reconcile::reconcile_desired_state as reconcile_pipeline;
use crate::librarian::scanner::{backfill_missing_fingerprints, scan_library};
use chrono::Utc;
use sqlx::sqlite::SqlitePool;
use sqlx::Row;
use std::path::{Path, PathBuf};
use tracing::{error, info, instrument, warn};
use tracing_subscriber::filter::EnvFilter;
use uuid::Uuid;

const STALE_SYNC_RUN_MINUTES: i64 = 15;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncPhase {
    DbInit,
    Scan,
    Import,
    Reconciliation,
    DeltaGeneration,
}

impl SyncPhase {
    fn as_str(self) -> &'static str {
        match self {
            Self::DbInit => "db_init",
            Self::Scan => "scan",
            Self::Import => "import",
            Self::Reconciliation => "reconciliation",
            Self::DeltaGeneration => "delta_generation",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncStatus {
    Success,
    FailedAt(SyncPhase),
    PartialSuccess,
}

#[derive(Debug, Clone, Default)]
pub struct SyncCounts {
    pub files_scanned: usize,
    pub files_upserted: usize,
    pub fingerprints_backfilled: usize,
    pub desired_tracks_imported: usize,
    pub reconciliation_results: usize,
    pub delta_queue_entries: usize,
    pub errors: usize,
}

#[derive(Debug, Clone)]
pub struct SyncOutcome {
    pub run_id: String,
    pub status: SyncStatus,
    pub summary: String,
    pub counts: SyncCounts,
    pub errors: Vec<(String, String)>,
}

#[instrument(skip(db_pool, config, _tracing_guard), fields(run_id))]
pub async fn run_librarian_sync(
    db_pool: &SqlitePool,
    config: &LibrarianConfig,
    desired_state_override: Option<PathBuf>,
    skip_import: bool,
    _tracing_guard: &tracing_subscriber::reload::Handle<EnvFilter, tracing_subscriber::Registry>,
) -> Result<SyncOutcome> {
    let run_id = Uuid::new_v4().to_string();
    tracing::Span::current().record("run_id", tracing::field::display(&run_id));

    if let Err(error) = initialize_database(db_pool).await {
        error!(run_id = %run_id, phase = "db_init", error = %error, "database initialization failed");
        return Err(error);
    }
    let recovered = recover_stale_sync_runs(db_pool, STALE_SYNC_RUN_MINUTES).await?;
    if recovered > 0 {
        warn!(run_id = %run_id, recovered, "recovered stale in-progress sync runs");
    }
    let sync_run_row_id = create_sync_run(db_pool, &run_id).await?;
    let mut counts = SyncCounts::default();
    let mut errors = Vec::<(String, String)>::new();

    info!(run_id = %run_id, "starting librarian sync");
    let _ = update_sync_run_phase(db_pool, sync_run_row_id, SyncPhase::DbInit, &counts).await;

    if config.skip_scan {
        info!(run_id = %run_id, phase = "scan", "scan phase skipped by request");
    } else {
        if let Err(error) = validate_roots_accessible(&config.library_roots).await {
            let message = error.to_string();
            let _ = update_sync_run_failed(db_pool, sync_run_row_id, SyncPhase::Scan, &message).await;
            error!(run_id = %run_id, phase = "scan", error = %message, "library roots inaccessible");
            return Err(error);
        }

        match scan_local_library(db_pool, config, &run_id).await {
            Ok((scanned, upserted)) => {
                counts.files_scanned = scanned;
                counts.files_upserted = upserted;
                let _ = update_sync_run_phase(db_pool, sync_run_row_id, SyncPhase::Scan, &counts).await;
                info!(run_id = %run_id, files_scanned = scanned, files_upserted = upserted, "scan phase completed");
            }
            Err(error) => {
                let message = error.to_string();
                let _ = update_sync_run_failed(db_pool, sync_run_row_id, SyncPhase::Scan, &message).await;
                error!(run_id = %run_id, phase = "scan", error = %message, "scan phase failed");
                return Err(error);
            }
        }
    }

    if config.enable_fingerprint_backfill && config.fingerprint_backfill_limit > 0 {
        let db = LibrarianDb::from_pool(db_pool.clone());
        match backfill_missing_fingerprints(
            &db,
            config.fingerprint_backfill_limit,
            config.fingerprint_backfill_concurrency,
        )
        .await
        {
            Ok(backfilled) => {
                counts.fingerprints_backfilled = backfilled;
                info!(
                    run_id = %run_id,
                    fingerprints_backfilled = backfilled,
                    limit = config.fingerprint_backfill_limit,
                    concurrency = config.fingerprint_backfill_concurrency,
                    "fingerprint backfill completed"
                );
            }
            Err(error) => {
                let message = error.to_string();
                errors.push(("fingerprint_backfill".to_string(), message.clone()));
                counts.errors += 1;
                warn!(
                    run_id = %run_id,
                    phase = "fingerprint_backfill",
                    error = %message,
                    "fingerprint backfill failed, continuing"
                );
            }
        }
    }

    if !skip_import {
        let import_source = desired_state_override.or_else(|| config.desired_state_path.clone());
        if let Some(source) = import_source {
            match import_desired_state(db_pool, &source, &run_id).await {
                Ok(imported) => {
                    counts.desired_tracks_imported = imported;
                    let _ = update_sync_run_phase(db_pool, sync_run_row_id, SyncPhase::Import, &counts).await;
                    info!(run_id = %run_id, desired_tracks_imported = imported, "import phase completed");
                }
                Err(error) => {
                    let message = error.to_string();
                    errors.push((SyncPhase::Import.as_str().to_string(), message.clone()));
                    counts.errors += 1;
                    warn!(run_id = %run_id, phase = "import", error = %message, "import failed, continuing with existing desired tracks");
                }
            }
        } else {
            info!(run_id = %run_id, phase = "import", "no desired-state source provided, skipping import");
        }
    } else {
        info!(run_id = %run_id, phase = "import", "desired-state import skipped by request");
    }

    match run_reconciliation_phase(db_pool, &run_id).await {
        Ok(reconciled) => {
            counts.reconciliation_results = reconciled;
            let _ = update_sync_run_phase(db_pool, sync_run_row_id, SyncPhase::Reconciliation, &counts).await;
            info!(run_id = %run_id, reconciliation_results = reconciled, "reconciliation phase completed");
        }
        Err(error) => {
            let message = error.to_string();
            errors.push((SyncPhase::Reconciliation.as_str().to_string(), message.clone()));
            counts.errors += 1;
            warn!(run_id = %run_id, phase = "reconciliation", error = %message, "reconciliation failed for some items; continuing to delta generation");
        }
    }

    match generate_delta_queue(db_pool, &run_id).await {
        Ok(delta_count) => {
            counts.delta_queue_entries = delta_count;
            let _ = update_sync_run_success(db_pool, sync_run_row_id, &counts).await;
            info!(run_id = %run_id, delta_queue_entries = delta_count, "delta generation phase completed");
        }
        Err(error) => {
            let message = error.to_string();
            let _ = update_sync_run_failed(db_pool, sync_run_row_id, SyncPhase::DeltaGeneration, &message).await;
            error!(run_id = %run_id, phase = "delta_generation", error = %message, "delta generation failed");
            return Err(error);
        }
    }

    let status = if errors.is_empty() {
        SyncStatus::Success
    } else {
        SyncStatus::PartialSuccess
    };
    let summary = format!(
        "Librarian sync completed [{}]: {} scanned, {} upserted, {} fingerprints, {} imported, {} reconciled, {} deltas",
        if config.skip_scan {
            "skip-scan"
        } else {
            config.scan_mode.as_str()
        },
        counts.files_scanned,
        counts.files_upserted,
        counts.fingerprints_backfilled,
        counts.desired_tracks_imported,
        counts.reconciliation_results,
        counts.delta_queue_entries
    );

    Ok(SyncOutcome {
        run_id,
        status,
        summary,
        counts,
        errors,
    })
}

async fn initialize_database(db_pool: &SqlitePool) -> Result<()> {
    sqlx::query("PRAGMA journal_mode=WAL;").execute(db_pool).await?;
    sqlx::query("PRAGMA foreign_keys=ON;").execute(db_pool).await?;

    for sql in migrations::MIGRATIONS {
        sqlx::query(sql).execute(db_pool).await?;
    }

    let required = [
        "artists",
        "albums",
        "tracks",
        "local_files",
        "scan_checkpoints",
        "desired_tracks",
        "reconciliation_results",
        "delta_queue",
        "sync_runs",
    ];

    for table in required {
        let exists = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
        )
        .bind(table)
        .fetch_one(db_pool)
        .await?;
        if exists == 0 {
            return Err(LibrarianError::ConfigError(format!(
                "required table missing after migrations: {table}"
            )));
        }
    }

    ensure_delta_queue_columns(db_pool).await?;
    ensure_local_file_columns(db_pool).await?;
    ensure_artist_constraints(db_pool).await?;

    Ok(())
}

async fn ensure_delta_queue_columns(db_pool: &SqlitePool) -> Result<()> {
    let columns = sqlx::query("PRAGMA table_info(delta_queue)")
        .fetch_all(db_pool)
        .await?;
    for (column_name, column_type) in [
        ("source_operation_id", "TEXT"),
        ("claimed_at", "TIMESTAMP"),
        ("claim_run_id", "TEXT"),
    ] {
        let has_column = columns.iter().any(|c| {
            c.try_get::<String, _>("name")
                .map(|name| name == column_name)
                .unwrap_or(false)
        });
        if !has_column {
            sqlx::query(&format!("ALTER TABLE delta_queue ADD COLUMN {column_name} {column_type}"))
                .execute(db_pool)
                .await?;
        }
    }
    Ok(())
}

async fn ensure_artist_constraints(db_pool: &SqlitePool) -> Result<()> {
    sqlx::query("DROP INDEX IF EXISTS idx_artists_normalized_name")
        .execute(db_pool)
        .await?;
    sqlx::query(
        "CREATE UNIQUE INDEX IF NOT EXISTS uq_artists_normalized_name ON artists(normalized_name)",
    )
    .execute(db_pool)
    .await?;
    Ok(())
}

async fn ensure_local_file_columns(db_pool: &SqlitePool) -> Result<()> {
    let columns = sqlx::query("PRAGMA table_info(local_files)")
        .fetch_all(db_pool)
        .await?;
    for (column_name, column_type) in [
        ("file_mtime_ms", "INTEGER"),
        ("acoustid_fingerprint", "TEXT"),
        ("fingerprint_attempted_at", "TIMESTAMP"),
        ("fingerprint_error", "TEXT"),
        ("fingerprint_source_mtime_ms", "INTEGER"),
    ] {
        let has_column = columns.iter().any(|column| {
            column
                .try_get::<String, _>("name")
                .map(|name| name == column_name)
                .unwrap_or(false)
        });
        if !has_column {
            sqlx::query(&format!("ALTER TABLE local_files ADD COLUMN {column_name} {column_type}"))
                .execute(db_pool)
                .await?;
        }
    }
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_local_files_acoustid_fingerprint ON local_files(acoustid_fingerprint)",
    )
    .execute(db_pool)
    .await?;
    Ok(())
}

async fn validate_roots_accessible(roots: &[PathBuf]) -> Result<()> {
    if roots.is_empty() {
        return Err(LibrarianError::ConfigError(
            "library_roots is empty; configure at least one root".to_string(),
        ));
    }

    let mut accessible = 0usize;
    for root in roots {
        if tokio::fs::metadata(root).await.is_ok() {
            accessible += 1;
        }
    }

    if accessible == 0 {
        return Err(LibrarianError::ScanError(
            "no configured library root is accessible".to_string(),
        ));
    }

    Ok(())
}

async fn scan_local_library(
    db_pool: &SqlitePool,
    config: &LibrarianConfig,
    run_id: &str,
) -> Result<(usize, usize)> {
    let db = LibrarianDb::from_pool(db_pool.clone());
    if should_skip_scan_phase(db_pool, config).await? {
        info!(run_id = %run_id, mode = "queue-only", "scan phase skipped because completed checkpoints already exist");
        return Ok((0, 0));
    }

    let before = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM local_files")
        .fetch_one(db_pool)
        .await?;

    let stats = scan_library(&db, config, run_id).await?;
    let after = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM local_files")
        .fetch_one(db_pool)
        .await?;

    info!(
        run_id = %run_id,
        mode = config.scan_mode.as_str(),
        discovered = stats.discovered_files,
        scanned = stats.scanned_files,
        skipped = stats.skipped_files,
        unreadable = stats.unreadable_files,
        suspicious = stats.suspicious_files,
        "scan statistics"
    );

    let scanned = usize::try_from(stats.scanned_files).unwrap_or(usize::MAX);
    let upserted = usize::try_from((after - before).max(0)).unwrap_or(usize::MAX);
    Ok((scanned, upserted))
}

async fn should_skip_scan_phase(db_pool: &SqlitePool, config: &LibrarianConfig) -> Result<bool> {
    if config.scan_mode != ScanMode::Resume {
        return Ok(false);
    }

    let db = LibrarianDb::from_pool(db_pool.clone());
    let roots = config
        .library_roots
        .iter()
        .map(|root| root.to_string_lossy().to_string())
        .collect::<Vec<_>>();
    if !db.has_completed_checkpoints(&roots).await? {
        return Ok(false);
    }

    let local_files = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM local_files")
        .fetch_one(db_pool)
        .await?;
    let tracks = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM tracks")
        .fetch_one(db_pool)
        .await?;
    Ok(local_files > 0 && tracks > 0)
}

async fn import_desired_state(db_pool: &SqlitePool, source: &Path, run_id: &str) -> Result<usize> {
    let payload = tokio::fs::read_to_string(source)
        .await
        .map_err(|error| LibrarianError::ImportError(error.to_string()))?;
    let db = LibrarianDb::from_pool(db_pool.clone());
    let imported = import_desired_spotify_json(&db, &payload).await?;
    info!(run_id = %run_id, source = %source.display(), imported, "desired-state import finished");
    Ok(imported)
}

async fn run_reconciliation_phase(db_pool: &SqlitePool, run_id: &str) -> Result<usize> {
    let db = LibrarianDb::from_pool(db_pool.clone());
    let reconciled = reconcile_pipeline(&db).await?;
    info!(run_id = %run_id, reconciled, "reconciliation finished");
    Ok(reconciled)
}

async fn generate_delta_queue(db_pool: &SqlitePool, run_id: &str) -> Result<usize> {
    // Only remove unclaimed unprocessed rows. Rows with claimed_at IS NOT NULL
    // are mid-flight in a coordinator run and must not be wiped.
    sqlx::query("DELETE FROM delta_queue WHERE processed_at IS NULL AND claimed_at IS NULL")
        .execute(db_pool)
        .await?;

    let rows = sqlx::query(
        "SELECT desired_track_id, reconciliation_status, reason, quality_assessment
         FROM reconciliation_results",
    )
    .fetch_all(db_pool)
    .await?;

    let mut generated = 0usize;
    for row in rows {
        let desired_track_id: i64 = row.try_get("desired_track_id")?;
        let status: String = row.try_get("reconciliation_status")?;
        let reason: String = row.try_get("reason")?;
        let quality: Option<String> = row.try_get("quality_assessment")?;

        let (action, priority) = match status.as_str() {
            "missing" => ("missing_download", 100_i64),
            "duplicate" => ("duplicate_review", 60_i64),
            "weak_match" | "manual_review" => ("manual_review", 40_i64),
            "exact_match" | "strong_match" => {
                if quality.as_deref() == Some("upgrade_candidate") {
                    ("upgrade_quality", 80_i64)
                } else {
                    ("no_action", 0_i64)
                }
            }
            "upgrade_needed" => ("upgrade_quality", 80_i64),
            _ => ("manual_review", 40_i64),
        };

        if action == "no_action" {
            continue;
        }

        sqlx::query(
            "INSERT INTO delta_queue (desired_track_id, action_type, priority, reason, target_quality, source_operation_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind(desired_track_id)
        .bind(action)
        .bind(priority)
        .bind(format!("run_id={run_id}; {reason}"))
        .bind(if action == "upgrade_quality" || action == "missing_download" {
            Some("lossless_preferred")
        } else {
            None
        })
        .bind(run_id)
        .execute(db_pool)
        .await?;

        generated += 1;
    }

    info!(run_id = %run_id, generated, "delta queue generation finished");
    Ok(generated)
}

pub async fn create_sync_run(db_pool: &SqlitePool, run_id: &str) -> Result<i64> {
    let started_at = Utc::now().to_rfc3339();
    let result = sqlx::query(
        "INSERT INTO sync_runs (run_id, started_at, status, phase_reached)
         VALUES (?1, ?2, 'in_progress', 'db_init')",
    )
    .bind(run_id)
    .bind(started_at)
    .execute(db_pool)
    .await?;

    Ok(result.last_insert_rowid())
}

pub async fn recover_stale_sync_runs(db_pool: &SqlitePool, stale_minutes: i64) -> Result<u64> {
    let result = sqlx::query(
        "UPDATE sync_runs
         SET ended_at = ?1,
             status = 'interrupted',
             error_message = COALESCE(error_message, 'startup recovery marked stale in-progress run as interrupted')
         WHERE status = 'in_progress'
           AND ended_at IS NULL
           AND julianday(started_at) <= julianday('now') - (?2 / 1440.0)",
    )
    .bind(Utc::now().to_rfc3339())
    .bind(stale_minutes as f64)
    .execute(db_pool)
    .await?;

    Ok(result.rows_affected())
}

pub async fn update_sync_run_phase(
    db_pool: &SqlitePool,
    sync_run_id: i64,
    phase: SyncPhase,
    counts: &SyncCounts,
) -> Result<()> {
    sqlx::query(
        "UPDATE sync_runs
         SET phase_reached = ?1,
             files_scanned = ?2,
             files_upserted = ?3,
             desired_tracks_imported = ?4,
             reconciliation_completed = ?5,
             delta_queue_entries = ?6
         WHERE id = ?7",
    )
    .bind(phase.as_str())
    .bind(i64::try_from(counts.files_scanned).unwrap_or(i64::MAX))
    .bind(i64::try_from(counts.files_upserted).unwrap_or(i64::MAX))
    .bind(i64::try_from(counts.desired_tracks_imported).unwrap_or(i64::MAX))
    .bind(counts.reconciliation_results > 0)
    .bind(i64::try_from(counts.delta_queue_entries).unwrap_or(i64::MAX))
    .bind(sync_run_id)
    .execute(db_pool)
    .await?;
    Ok(())
}

pub async fn update_sync_run_success(
    db_pool: &SqlitePool,
    sync_run_id: i64,
    counts: &SyncCounts,
) -> Result<()> {
    sqlx::query(
        "UPDATE sync_runs
         SET ended_at = ?1,
             status = 'success',
             phase_reached = 'delta_generation',
             files_scanned = ?2,
             files_upserted = ?3,
             desired_tracks_imported = ?4,
             reconciliation_completed = ?5,
             delta_queue_entries = ?6,
             error_message = NULL
         WHERE id = ?7",
    )
    .bind(Utc::now().to_rfc3339())
    .bind(i64::try_from(counts.files_scanned).unwrap_or(i64::MAX))
    .bind(i64::try_from(counts.files_upserted).unwrap_or(i64::MAX))
    .bind(i64::try_from(counts.desired_tracks_imported).unwrap_or(i64::MAX))
    .bind(counts.reconciliation_results > 0)
    .bind(i64::try_from(counts.delta_queue_entries).unwrap_or(i64::MAX))
    .bind(sync_run_id)
    .execute(db_pool)
    .await?;
    Ok(())
}

pub async fn update_sync_run_failed(
    db_pool: &SqlitePool,
    sync_run_id: i64,
    phase: SyncPhase,
    error_message: &str,
) -> Result<()> {
    sqlx::query(
        "UPDATE sync_runs
         SET ended_at = ?1,
             status = 'failed',
             phase_reached = ?2,
             error_message = ?3
         WHERE id = ?4",
    )
    .bind(Utc::now().to_rfc3339())
    .bind(phase.as_str())
    .bind(error_message)
    .bind(sync_run_id)
    .execute(db_pool)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;
    use tempfile::tempdir;

    async fn test_pool() -> SqlitePool {
        SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("memory db")
    }

    #[tokio::test]
    async fn sync_run_helpers_track_success_and_failure() {
        let pool = test_pool().await;
        initialize_database(&pool).await.expect("migrate");

        let run_row = create_sync_run(&pool, "run-1").await.expect("create run");
        let counts = SyncCounts {
            files_scanned: 10,
            files_upserted: 9,
            fingerprints_backfilled: 0,
            desired_tracks_imported: 2,
            reconciliation_results: 2,
            delta_queue_entries: 3,
            errors: 0,
        };
        update_sync_run_phase(&pool, run_row, SyncPhase::Scan, &counts)
            .await
            .expect("phase update");
        update_sync_run_success(&pool, run_row, &counts)
            .await
            .expect("success update");

        let status: String = sqlx::query_scalar("SELECT status FROM sync_runs WHERE id = ?1")
            .bind(run_row)
            .fetch_one(&pool)
            .await
            .expect("status");
        assert_eq!(status, "success");

        let failed_row = create_sync_run(&pool, "run-2").await.expect("create run 2");
        update_sync_run_failed(&pool, failed_row, SyncPhase::Import, "bad json")
            .await
            .expect("failed update");

        let failed_status: String =
            sqlx::query_scalar("SELECT status FROM sync_runs WHERE id = ?1")
                .bind(failed_row)
                .fetch_one(&pool)
                .await
                .expect("failed status");
        assert_eq!(failed_status, "failed");
    }

    #[tokio::test]
    async fn stale_in_progress_runs_are_marked_interrupted() {
        let pool = test_pool().await;
        initialize_database(&pool).await.expect("migrate");

        let stale_row = create_sync_run(&pool, "stale-run").await.expect("create stale run");
        sqlx::query("UPDATE sync_runs SET started_at = '2026-01-01T00:00:00+00:00' WHERE id = ?1")
            .bind(stale_row)
            .execute(&pool)
            .await
            .expect("age stale run");

        let fresh_row = create_sync_run(&pool, "fresh-run").await.expect("create fresh run");
        let recovered = recover_stale_sync_runs(&pool, 15)
            .await
            .expect("recover stale runs");
        assert_eq!(recovered, 1);

        let stale_status: String =
            sqlx::query_scalar("SELECT status FROM sync_runs WHERE id = ?1")
                .bind(stale_row)
                .fetch_one(&pool)
                .await
                .expect("stale status");
        assert_eq!(stale_status, "interrupted");

        let fresh_status: String =
            sqlx::query_scalar("SELECT status FROM sync_runs WHERE id = ?1")
                .bind(fresh_row)
                .fetch_one(&pool)
                .await
                .expect("fresh status");
        assert_eq!(fresh_status, "in_progress");
    }

    #[tokio::test]
    async fn orchestrator_fails_when_all_roots_missing() {
        let pool = test_pool().await;
        initialize_database(&pool).await.expect("migrate");

        let mut config = LibrarianConfig::default();
        config.library_roots = vec![PathBuf::from("Z:/definitely-missing-cassette-root")];

        let (_reload, handle) = tracing_subscriber::reload::Layer::new(EnvFilter::new("info"));
        let result = run_librarian_sync(&pool, &config, None, true, &handle).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn orchestrator_completes_on_empty_accessible_root() {
        let pool = test_pool().await;
        initialize_database(&pool).await.expect("migrate");

        let root = tempdir().expect("tempdir");
        let mut config = LibrarianConfig::default();
        config.library_roots = vec![root.path().to_path_buf()];

        let (_reload, handle) = tracing_subscriber::reload::Layer::new(EnvFilter::new("info"));
        let outcome = run_librarian_sync(&pool, &config, None, true, &handle)
            .await
            .expect("sync outcome");

        assert!(matches!(outcome.status, SyncStatus::Success));
        assert_eq!(outcome.counts.files_scanned, 0);
    }
}
