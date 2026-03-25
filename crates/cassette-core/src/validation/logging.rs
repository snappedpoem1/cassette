use crate::library::types::{FileLineageEvent, OperationDetails};
use crate::library::{LibraryManager, Module};
use crate::validation::error::{Result, ValidationError};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogVerification {
    pub total_operations: usize,
    pub total_events: usize,
    pub stalled_operations: usize,
    pub orphaned_operations: usize,
    pub max_concurrent_locks: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct FileLineageLikeEvent {
    pub event_id: i64,
    pub operation_id: String,
    pub module: String,
    pub phase: String,
    pub event_type: String,
    pub timestamp: Option<String>,
    pub event_data: Option<String>,
}

pub async fn verify_operation_log(
    manager: &LibraryManager,
    module: Module,
    phase: &str,
) -> Result<()> {
    let ops = sqlx::query_as::<_, (String,)>(
        "SELECT operation_id FROM operation_log WHERE module = ?1 AND phase = ?2 ORDER BY started_at DESC LIMIT 10",
    )
    .bind(module.as_str())
    .bind(phase)
    .fetch_all(manager.db_pool())
    .await?;

    if ops.is_empty() {
        return Err(ValidationError::NoOperationsLogged(format!(
            "{} {}",
            module, phase
        )));
    }

    let latest_operation_id = &ops[0].0;
    let event_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM operation_events WHERE operation_id = ?1",
    )
    .bind(latest_operation_id)
    .fetch_one(manager.db_pool())
    .await? as usize;

    if event_count == 0 {
        return Err(ValidationError::OperationHasNoEvents(
            latest_operation_id.clone(),
        ));
    }

    Ok(())
}

pub async fn verify_complete_operation_log(manager: &LibraryManager) -> Result<LogVerification> {
    let total_operations =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM operation_log")
            .fetch_one(manager.db_pool())
            .await? as usize;

    let total_events =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM operation_events")
            .fetch_one(manager.db_pool())
            .await? as usize;

    let stalled_operations = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*) FROM operation_log
        WHERE status = 'in_progress'
          AND datetime(started_at) < datetime('now', '-1 hour')
        "#,
    )
    .fetch_one(manager.db_pool())
    .await? as usize;

    let orphaned_operations = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*) FROM operation_log ol
        WHERE NOT EXISTS (
            SELECT 1 FROM operation_events oe WHERE oe.operation_id = ol.operation_id
        )
          AND ol.status IN ('success', 'partial_success')
        "#,
    )
    .fetch_one(manager.db_pool())
    .await? as usize;

    let max_concurrent_locks = sqlx::query_scalar::<_, Option<i64>>(
        r#"
        SELECT MAX(lock_count)
        FROM (
            SELECT COUNT(*) AS lock_count, acquired_at
            FROM file_locks
            GROUP BY acquired_at
        )
        "#,
    )
    .fetch_one(manager.db_pool())
    .await?
    .unwrap_or(0) as usize;

    Ok(LogVerification {
        total_operations,
        total_events,
        stalled_operations,
        orphaned_operations,
        max_concurrent_locks,
    })
}

pub async fn get_file_lineage(
    manager: &LibraryManager,
    file_path: &str,
) -> Result<Vec<FileLineageLikeEvent>> {
    let filename = std::path::Path::new(file_path)
        .file_name()
        .and_then(|v| v.to_str())
        .unwrap_or(file_path)
        .to_string();

    let like_full = format!("%{file_path}%");
    let like_name = format!("%{filename}%");

    let rows = sqlx::query_as::<_, FileLineageLikeEvent>(
        r#"
        SELECT
            oe.event_id,
            oe.operation_id,
            ol.module,
            ol.phase,
            oe.event_type,
            oe.timestamp,
            oe.event_data
        FROM operation_events oe
        JOIN operation_log ol ON oe.operation_id = ol.operation_id
        WHERE oe.event_data LIKE ?1 OR oe.event_data LIKE ?2
        ORDER BY oe.event_id ASC
        "#,
    )
    .bind(like_full)
    .bind(like_name)
    .fetch_all(manager.db_pool())
    .await?;

    Ok(rows)
}

pub async fn get_operation_summary(
    manager: &LibraryManager,
    operation_id: &str,
) -> Result<OperationDetails> {
    manager.get_operation_details(operation_id).await.map_err(Into::into)
}

pub async fn get_file_lineage_by_local_file_target(
    manager: &LibraryManager,
    file_path: &std::path::Path,
) -> Result<Vec<FileLineageEvent>> {
    let lineage = manager.get_file_lineage(file_path).await?;
    Ok(lineage.events)
}
