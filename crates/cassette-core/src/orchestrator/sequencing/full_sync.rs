use crate::librarian::models::DesiredTrack;
use crate::library::{LibraryManager, Module, OperationStatus};
use crate::orchestrator::config::OrchestratorConfig;
use crate::orchestrator::delta::generation::generate_delta_queue_managed;
use crate::orchestrator::error::{OrchestratorError, Result};
use crate::orchestrator::reconciliation::engine::reconcile_desired_against_local;
use crate::orchestrator::sequencing::custodian_phase::run_custodian_phase_managed;
use crate::orchestrator::sequencing::librarian_phase::run_librarian_phase_managed;
use crate::orchestrator::types::{CustodianPhaseOutcome, FullSyncOutcome, LibrarianPhaseOutcome};
use sqlx::Row;

pub async fn run_full_library_sync_with_manager(
    manager: &LibraryManager,
    config: &OrchestratorConfig,
) -> Result<FullSyncOutcome> {
    ensure_orchestrator_tables(manager.db_pool()).await?;

    let root_op_id = manager
        .start_operation(Module::Orchestrator, "full_library_sync")
        .await
        .map_err(|e| OrchestratorError::ManagerError(e.to_string()))?;

    let scan_outcome = if config.run_librarian {
        run_librarian_phase_managed(manager, &root_op_id, config).await?
    } else {
        LibrarianPhaseOutcome::default()
    };

    let cleanup_outcome = if config.run_custodian {
        run_custodian_phase_managed(manager, &root_op_id, config).await?
    } else {
        CustodianPhaseOutcome::default()
    };

    let desired_tracks: Vec<DesiredTrack> = if config.run_reconciliation {
        sqlx::query_as::<_, DesiredTrack>(
            "SELECT id, source_name, source_track_id, source_album_id, source_artist_id, artist_name, album_title, track_title, track_number, disc_number, duration_ms, isrc, raw_payload_json, imported_at FROM desired_tracks ORDER BY id",
        )
        .fetch_all(manager.db_pool())
        .await
        .map_err(|e| OrchestratorError::DatabaseError(e.to_string()))?
    } else {
        Vec::new()
    };

    let reconciliation = if config.run_reconciliation {
        reconcile_desired_against_local(
            manager,
            &root_op_id,
            &desired_tracks,
            &config.reconciliation,
        )
        .await?
    } else {
        crate::orchestrator::types::ReconciliationResult::default()
    };

    let delta_queue = if config.run_reconciliation {
        generate_delta_queue_managed(manager, &root_op_id, &reconciliation).await?
    } else {
        Vec::new()
    };

    manager
        .log_event(
            &root_op_id,
            "full_sync_complete",
            None,
            None,
            None,
            None,
            &serde_json::json!({
                "scanned": scan_outcome.files_scanned,
                "cleaned": cleanup_outcome.files_sorted,
                "reconciled": reconciliation.matched_count,
                "missing": reconciliation.missing_count,
                "deltas_generated": delta_queue.len(),
            }),
        )
        .await
        .map_err(|e| OrchestratorError::ManagerError(e.to_string()))?;

    manager
        .complete_operation(&root_op_id, OperationStatus::Success)
        .await
        .map_err(|e| OrchestratorError::ManagerError(e.to_string()))?;

    Ok(FullSyncOutcome {
        root_operation_id: root_op_id,
        scan_outcome,
        cleanup_outcome,
        reconciliation,
        delta_queue,
    })
}

async fn ensure_orchestrator_tables(pool: &sqlx::SqlitePool) -> Result<()> {
        sqlx::query(
                r#"
                CREATE TABLE IF NOT EXISTS desired_tracks (
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
                    imported_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
                )
                "#,
        )
        .execute(pool)
        .await
        .map_err(|e| OrchestratorError::DatabaseError(e.to_string()))?;

        sqlx::query(
                r#"
                CREATE TABLE IF NOT EXISTS reconciliation_results (
                    id INTEGER PRIMARY KEY,
                    desired_track_id INTEGER NOT NULL REFERENCES desired_tracks(id),
                    matched_track_id INTEGER REFERENCES tracks(id),
                    matched_local_file_id INTEGER REFERENCES local_files(id),
                    reconciliation_status TEXT NOT NULL,
                    quality_assessment TEXT,
                    reason TEXT NOT NULL,
                    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
                )
                "#,
        )
        .execute(pool)
        .await
        .map_err(|e| OrchestratorError::DatabaseError(e.to_string()))?;

        sqlx::query(
                r#"
                CREATE TABLE IF NOT EXISTS delta_queue (
                    id INTEGER PRIMARY KEY,
                    desired_track_id INTEGER NOT NULL REFERENCES desired_tracks(id),
                    action_type TEXT NOT NULL,
                    priority INTEGER DEFAULT 0,
                    reason TEXT NOT NULL,
                    target_quality TEXT,
                    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                    processed_at TIMESTAMP
                )
                "#,
        )
        .execute(pool)
        .await
        .map_err(|e| OrchestratorError::DatabaseError(e.to_string()))?;

    let columns = sqlx::query("PRAGMA table_info(delta_queue)")
        .fetch_all(pool)
        .await
        .map_err(|e| OrchestratorError::DatabaseError(e.to_string()))?;
    let has_source_operation_id = columns.iter().any(|c| {
        c.try_get::<String, _>("name")
            .map(|name| name == "source_operation_id")
            .unwrap_or(false)
    });
    if !has_source_operation_id {
        sqlx::query("ALTER TABLE delta_queue ADD COLUMN source_operation_id TEXT")
            .execute(pool)
            .await
            .map_err(|e| OrchestratorError::DatabaseError(e.to_string()))?;
    }

    Ok(())
}
