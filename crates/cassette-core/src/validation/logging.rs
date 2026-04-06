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

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct GatekeeperAuditLikeEvent {
    pub operation_id: String,
    pub timestamp: String,
    pub file_path: String,
    pub decision: String,
    pub desired_track_id: Option<i64>,
    pub matched_local_file_id: Option<i64>,
    pub duration_ms: i64,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditTrace {
    pub file_path: Option<String>,
    pub desired_track_id: Option<i64>,
    pub operation_events: Vec<FileLineageLikeEvent>,
    pub gatekeeper_audit: Vec<GatekeeperAuditLikeEvent>,
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
    let total_operations = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM operation_log")
        .fetch_one(manager.db_pool())
        .await? as usize;

    let total_events = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM operation_events")
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
    let has_path_context = file_path.contains('\\') || file_path.contains('/');
    let filename = std::path::Path::new(file_path)
        .file_name()
        .and_then(|v| v.to_str())
        .unwrap_or(file_path)
        .to_string();

    let like_full = format!("%{file_path}%");
    let escaped_file_path = file_path.replace('\\', "\\\\");
    let like_full_escaped = format!("%{escaped_file_path}%");
    let like_name = format!("%{filename}%");

    let rows = if has_path_context {
        sqlx::query_as::<_, FileLineageLikeEvent>(
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
        .bind(like_full_escaped)
        .fetch_all(manager.db_pool())
        .await?
    } else {
        sqlx::query_as::<_, FileLineageLikeEvent>(
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
        .await?
    };

    Ok(rows)
}

pub async fn get_operation_summary(
    manager: &LibraryManager,
    operation_id: &str,
) -> Result<OperationDetails> {
    manager
        .get_operation_details(operation_id)
        .await
        .map_err(Into::into)
}

pub async fn get_file_lineage_by_local_file_target(
    manager: &LibraryManager,
    file_path: &std::path::Path,
) -> Result<Vec<FileLineageEvent>> {
    let lineage = manager.get_file_lineage(file_path).await?;
    Ok(lineage.events)
}

pub async fn explain_audit_trace(
    manager: &LibraryManager,
    file_path: Option<&str>,
    desired_track_id: Option<i64>,
) -> Result<AuditTrace> {
    let operation_events = match (file_path, desired_track_id) {
        (Some(file_path), Some(desired_track_id)) => {
            let file_pattern = format!("%{file_path}%");
            let desired_pattern = format!("%\"desired_track_id\":{desired_track_id}%");
            sqlx::query_as::<_, FileLineageLikeEvent>(
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
            .bind(file_pattern)
            .bind(desired_pattern)
            .fetch_all(manager.db_pool())
            .await?
        }
        (Some(file_path), None) => get_file_lineage(manager, file_path).await?,
        (None, Some(desired_track_id)) => {
            let desired_pattern = format!("%\"desired_track_id\":{desired_track_id}%");
            sqlx::query_as::<_, FileLineageLikeEvent>(
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
                WHERE oe.event_data LIKE ?1
                ORDER BY oe.event_id ASC
                "#,
            )
            .bind(desired_pattern)
            .fetch_all(manager.db_pool())
            .await?
        }
        (None, None) => Vec::new(),
    };

    let gatekeeper_audit = match (file_path, desired_track_id) {
        (Some(file_path), Some(desired_track_id)) => {
            sqlx::query_as::<_, GatekeeperAuditLikeEvent>(
                r#"
                SELECT operation_id, timestamp, file_path, decision, desired_track_id,
                       matched_local_file_id, duration_ms, notes
                FROM gatekeeper_audit_log
                WHERE file_path = ?1 OR desired_track_id = ?2
                ORDER BY created_at ASC, id ASC
                "#,
            )
            .bind(file_path)
            .bind(desired_track_id)
            .fetch_all(manager.db_pool())
            .await?
        }
        (Some(file_path), None) => {
            sqlx::query_as::<_, GatekeeperAuditLikeEvent>(
                r#"
                SELECT operation_id, timestamp, file_path, decision, desired_track_id,
                       matched_local_file_id, duration_ms, notes
                FROM gatekeeper_audit_log
                WHERE file_path = ?1
                ORDER BY created_at ASC, id ASC
                "#,
            )
            .bind(file_path)
            .fetch_all(manager.db_pool())
            .await?
        }
        (None, Some(desired_track_id)) => {
            sqlx::query_as::<_, GatekeeperAuditLikeEvent>(
                r#"
                SELECT operation_id, timestamp, file_path, decision, desired_track_id,
                       matched_local_file_id, duration_ms, notes
                FROM gatekeeper_audit_log
                WHERE desired_track_id = ?1
                ORDER BY created_at ASC, id ASC
                "#,
            )
            .bind(desired_track_id)
            .fetch_all(manager.db_pool())
            .await?
        }
        (None, None) => Vec::new(),
    };

    Ok(AuditTrace {
        file_path: file_path.map(ToString::to_string),
        desired_track_id,
        operation_events,
        gatekeeper_audit,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gatekeeper::config::GatekeeperConfig;
    use crate::gatekeeper::database::ensure_schema as ensure_gatekeeper_schema;
    use crate::library::{LibraryManager, ManagerConfig, Module};
    use std::collections::HashSet;
    use std::path::Path;

    #[tokio::test]
    async fn explain_audit_trace_collects_operation_events_and_gatekeeper_rows() {
        let db_file = tempfile::NamedTempFile::new().expect("temp db");
        let db_url = format!("sqlite://{}", db_file.path().to_string_lossy());
        let manager = LibraryManager::connect(&db_url, ManagerConfig::default())
            .await
            .expect("manager");
        ensure_gatekeeper_schema(manager.db_pool())
            .await
            .expect("gatekeeper schema");

        let operation_id = manager
            .start_operation(Module::Gatekeeper, "batch_ingest")
            .await
            .expect("start operation");
        let file_path = "C:\\temp\\track.wav";
        let desired_track_id = 42_i64;

        manager
            .log_event(
                &operation_id,
                "gatekeeper_admitted",
                None,
                None,
                None,
                None,
                &serde_json::json!({
                    "file_path": file_path,
                    "desired_track_id": desired_track_id,
                    "decision": "Admitted"
                }),
            )
            .await
            .expect("log event");

        sqlx::query(
            "INSERT INTO gatekeeper_audit_log (
                operation_id, timestamp, file_path, decision, desired_track_id,
                matched_local_file_id, duration_ms, notes
             ) VALUES (?1, datetime('now'), ?2, ?3, ?4, ?5, ?6, ?7)",
        )
        .bind(&operation_id)
        .bind(file_path)
        .bind("Admitted")
        .bind(desired_track_id)
        .bind(5_i64)
        .bind(100_i64)
        .bind("audit test")
        .execute(manager.db_pool())
        .await
        .expect("insert audit row");

        let trace = explain_audit_trace(&manager, Some(file_path), Some(desired_track_id))
            .await
            .expect("trace should load");

        assert!(!trace.operation_events.is_empty());
        assert!(!trace.gatekeeper_audit.is_empty());
        assert_eq!(
            trace.gatekeeper_audit[0].desired_track_id,
            Some(desired_track_id)
        );

        let operation_ids: HashSet<&str> = trace
            .operation_events
            .iter()
            .map(|row| row.operation_id.as_str())
            .collect();
        for audit_row in &trace.gatekeeper_audit {
            assert!(
                operation_ids.contains(audit_row.operation_id.as_str()),
                "gatekeeper_audit operation_id must correlate to operation_events"
            );
        }
    }

    #[tokio::test]
    async fn explain_audit_trace_filters_to_exact_file_path_context() {
        let db_file = tempfile::NamedTempFile::new().expect("temp db");
        let db_url = format!("sqlite://{}", db_file.path().to_string_lossy());
        let manager = LibraryManager::connect(&db_url, ManagerConfig::default())
            .await
            .expect("manager");

        let operation_id = manager
            .start_operation(Module::Gatekeeper, "batch_ingest")
            .await
            .expect("start operation");

        let wanted = "C:\\library\\a\\track.wav";
        let other = "C:\\library\\b\\track.wav";

        manager
            .log_event(
                &operation_id,
                "gatekeeper_admitted",
                None,
                None,
                None,
                None,
                &serde_json::json!({
                    "file_path": wanted,
                    "desired_track_id": 77,
                }),
            )
            .await
            .expect("wanted event");

        manager
            .log_event(
                &operation_id,
                "gatekeeper_admitted",
                None,
                None,
                None,
                None,
                &serde_json::json!({
                    "file_path": other,
                    "desired_track_id": 88,
                }),
            )
            .await
            .expect("other event");

        let lineage = get_file_lineage(&manager, wanted)
            .await
            .expect("lineage for wanted path");

        assert!(
            !lineage.is_empty(),
            "expected at least one matching lineage row"
        );
        let wanted_escaped = wanted.replace('\\', "\\\\");
        assert!(
            lineage.iter().all(|row| {
                let data = row.event_data.as_deref().unwrap_or_default();
                data.contains(wanted) || data.contains(&wanted_escaped)
            }),
            "lineage rows must only include event_data for the requested full path"
        );
    }

    #[tokio::test]
    async fn gatekeeper_failure_emits_failure_and_completion_events() {
        let db_file = tempfile::NamedTempFile::new().expect("temp db");
        let db_url = format!("sqlite://{}", db_file.path().to_string_lossy());
        let manager = LibraryManager::connect(&db_url, ManagerConfig::default())
            .await
            .expect("manager");
        ensure_gatekeeper_schema(manager.db_pool())
            .await
            .expect("gatekeeper schema");

        let config = GatekeeperConfig::default();
        let missing = Path::new("C:\\definitely-missing\\audit-gap-test.wav");
        let batch = vec![(missing, None)];

        let outcome = manager
            .run_gatekeeper_with_manager(&batch, &config)
            .await
            .expect("batch ingest should complete even with rejected rows");

        assert_eq!(outcome.total_files, 1);
        assert_eq!(outcome.rejected, 1);

        let failed_events = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM operation_events WHERE event_type = 'gatekeeper_ingest_failed'",
        )
        .fetch_one(manager.db_pool())
        .await
        .expect("count failed events");

        let completion_events = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM operation_events WHERE event_type = 'batch_ingest_complete'",
        )
        .fetch_one(manager.db_pool())
        .await
        .expect("count completion events");

        assert!(failed_events >= 1, "expected failure event to be logged");
        assert!(
            completion_events >= 1,
            "expected batch completion event to be logged"
        );
    }
}
