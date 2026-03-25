use crate::librarian::orchestrator::run_librarian_sync;
use crate::orchestrator::config::OrchestratorConfig;
use crate::orchestrator::error::{OrchestratorError, Result};
use crate::orchestrator::sequencing::caching::check_cached_scan;
use crate::orchestrator::types::LibrarianPhaseOutcome;
use tracing_subscriber::filter::EnvFilter;

pub async fn run_librarian_phase_managed(
    manager: &crate::library::LibraryManager,
    parent_op_id: &str,
    config: &OrchestratorConfig,
) -> Result<LibrarianPhaseOutcome> {
    ensure_librarian_compat_schema(manager.db_pool()).await?;

    let op_id = manager
        .start_operation(crate::library::Module::Librarian, "scan")
        .await
        .map_err(|e| OrchestratorError::ManagerError(e.to_string()))?;

    if let Some(cached) = check_cached_scan(manager, config).await {
        manager
            .complete_operation(&op_id, crate::library::OperationStatus::Success)
            .await
            .map_err(|e| OrchestratorError::ManagerError(e.to_string()))?;
        return Ok(cached);
    }

    let (layer, handle) = tracing_subscriber::reload::Layer::new(EnvFilter::new(
        config.librarian.tracing_filter.clone(),
    ));
    drop(layer);

    let library_roots: Vec<String> = config
        .librarian
        .library_roots
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();

    manager
        .log_event(
            &op_id,
            "scan_started",
            None,
            None,
            None,
            None,
            &serde_json::json!({
                "library_roots": library_roots,
                "content_hashing": config.librarian.enable_content_hashing,
            }),
        )
        .await
        .map_err(|e| OrchestratorError::ManagerError(e.to_string()))?;

    match run_librarian_sync(manager.db_pool(), &config.librarian, None, false, &handle).await {
        Ok(outcome) => {
            let phase = LibrarianPhaseOutcome {
                files_scanned: outcome.counts.files_scanned,
                files_upserted: outcome.counts.files_upserted,
                files_quarantined: 0,
            };

            manager
                .log_event(
                    &op_id,
                    "scan_completed",
                    None,
                    None,
                    None,
                    None,
                    &serde_json::json!({
                        "files_scanned": phase.files_scanned,
                        "files_upserted": phase.files_upserted,
                        "errors": outcome.errors.len(),
                        "run_id": outcome.run_id,
                        "status": format!("{:?}", outcome.status),
                    }),
                )
                .await
                .map_err(|e| OrchestratorError::ManagerError(e.to_string()))?;

            if !outcome.errors.is_empty() {
                for (phase_name, error_msg) in &outcome.errors {
                    manager
                        .log_event(
                            &op_id,
                            "scan_phase_error",
                            None,
                            None,
                            None,
                            None,
                            &serde_json::json!({
                                "phase": phase_name,
                                "error": error_msg,
                            }),
                        )
                        .await
                        .map_err(|e| OrchestratorError::ManagerError(e.to_string()))?;
                }
            }

            manager
                .complete_operation(&op_id, crate::library::OperationStatus::Success)
                .await
                .map_err(|e| OrchestratorError::ManagerError(e.to_string()))?;

            manager
                .log_event(
                    parent_op_id,
                    "librarian_scan_complete",
                    None,
                    None,
                    None,
                    None,
                    &serde_json::json!({
                        "scanned": phase.files_scanned,
                        "upserted": phase.files_upserted,
                        "quarantined": phase.files_quarantined,
                    }),
                )
                .await
                .map_err(|e| OrchestratorError::ManagerError(e.to_string()))?;

            Ok(phase)
        }
        Err(error) => {
            manager
                .complete_operation(
                    &op_id,
                    crate::library::OperationStatus::FailedAt(error.to_string()),
                )
                .await
                .map_err(|e| OrchestratorError::ManagerError(e.to_string()))?;
            Err(OrchestratorError::LibrarianFailed(error.to_string()))
        }
    }
}

async fn ensure_librarian_compat_schema(pool: &sqlx::SqlitePool) -> Result<()> {
    let alters = [
        "ALTER TABLE tracks ADD COLUMN track_number INTEGER",
        "ALTER TABLE tracks ADD COLUMN disc_number INTEGER",
        "ALTER TABLE tracks ADD COLUMN duration_ms INTEGER",
        "ALTER TABLE tracks ADD COLUMN isrc TEXT",
        "ALTER TABLE tracks ADD COLUMN spotify_id TEXT",
        "ALTER TABLE tracks ADD COLUMN discogs_id TEXT",
        "ALTER TABLE tracks ADD COLUMN created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP",
        "ALTER TABLE tracks ADD COLUMN updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP",
        "ALTER TABLE albums ADD COLUMN release_date DATE",
        "ALTER TABLE albums ADD COLUMN spotify_id TEXT",
        "ALTER TABLE albums ADD COLUMN discogs_id TEXT",
        "ALTER TABLE albums ADD COLUMN cover_art_path TEXT",
        "ALTER TABLE albums ADD COLUMN created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP",
        "ALTER TABLE albums ADD COLUMN updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP",
        "ALTER TABLE artists ADD COLUMN spotify_id TEXT",
        "ALTER TABLE artists ADD COLUMN discogs_id TEXT",
        "ALTER TABLE artists ADD COLUMN created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP",
        "ALTER TABLE artists ADD COLUMN updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP",
        "ALTER TABLE local_files ADD COLUMN file_name TEXT",
        "ALTER TABLE local_files ADD COLUMN extension TEXT",
        "ALTER TABLE local_files ADD COLUMN codec TEXT",
        "ALTER TABLE local_files ADD COLUMN bitrate INTEGER",
        "ALTER TABLE local_files ADD COLUMN sample_rate INTEGER",
        "ALTER TABLE local_files ADD COLUMN bit_depth INTEGER",
        "ALTER TABLE local_files ADD COLUMN channels INTEGER",
        "ALTER TABLE local_files ADD COLUMN duration_ms INTEGER",
        "ALTER TABLE local_files ADD COLUMN file_size INTEGER",
        "ALTER TABLE local_files ADD COLUMN last_scanned_at TIMESTAMP",
        "ALTER TABLE local_files ADD COLUMN created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP",
        "ALTER TABLE local_files ADD COLUMN updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP",
        "ALTER TABLE local_files ADD COLUMN content_hash TEXT",
        "ALTER TABLE local_files ADD COLUMN acoustid_fingerprint TEXT",
    ];

    for sql in alters {
        let _ = sqlx::query(sql).execute(pool).await;
    }

    let indexes = [
        "CREATE UNIQUE INDEX IF NOT EXISTS uq_artists_normalized_name ON artists(normalized_name)",
        "CREATE UNIQUE INDEX IF NOT EXISTS uq_albums_artist_normalized_title ON albums(artist_id, normalized_title)",
        "CREATE UNIQUE INDEX IF NOT EXISTS uq_tracks_isrc ON tracks(isrc)",
    ];
    for sql in indexes {
        let _ = sqlx::query(sql).execute(pool).await;
    }

    Ok(())
}
