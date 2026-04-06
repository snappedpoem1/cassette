use crate::library::error::{ManagerError, Result};
use crate::library::manager::LibraryManager;
use crate::library::state::{Module, OperationContext, OperationStatus};
use chrono::Utc;
use uuid::Uuid;

impl LibraryManager {
    pub async fn start_operation(&self, module: Module, phase: &str) -> Result<String> {
        let operation_id = Uuid::new_v4().to_string();

        sqlx::query(
            r#"
            INSERT INTO operation_log
              (operation_id, module, phase, status, started_at)
            VALUES (?1, ?2, ?3, 'in_progress', CURRENT_TIMESTAMP)
            "#,
        )
        .bind(&operation_id)
        .bind(module.as_str())
        .bind(phase)
        .execute(&self.db_pool)
        .await?;

        self.log_event(
            &operation_id,
            "operation_started",
            None,
            None,
            None,
            None,
            &serde_json::json!({
                "module": module.as_str(),
                "phase": phase,
            }),
        )
        .await?;

        let mut active = self.active_operations.write().await;
        active.insert(
            operation_id.clone(),
            OperationContext {
                operation_id: operation_id.clone(),
                module,
                phase: phase.to_string(),
                started_at: Utc::now(),
                affected_files: Vec::new(),
                affected_tracks: Vec::new(),
                waiting_on_file: None,
                status: OperationStatus::InProgress,
            },
        );

        tracing::info!(
            operation_id = %operation_id,
            module = %module,
            phase = phase,
            "Operation started"
        );

        Ok(operation_id)
    }

    pub async fn complete_operation(
        &self,
        operation_id: &str,
        status: OperationStatus,
    ) -> Result<()> {
        let (files_affected, tracks_affected) = {
            let active = self.active_operations.read().await;
            active
                .get(operation_id)
                .map(|ctx| {
                    (
                        ctx.affected_files.len() as i64,
                        ctx.affected_tracks.len() as i64,
                    )
                })
                .unwrap_or((0, 0))
        };

        let duration_ms = self.calculate_operation_duration(operation_id).await?;
        let status_str = status.as_db_status().to_string();
        let error_msg = match &status {
            OperationStatus::FailedAt(msg) => Some(msg.clone()),
            _ => None,
        };

        sqlx::query(
            r#"
            UPDATE operation_log
            SET status = ?1,
                ended_at = CURRENT_TIMESTAMP,
                duration_ms = ?2,
                error_message = ?3,
                files_affected = ?4,
                tracks_affected = ?5
            WHERE operation_id = ?6
            "#,
        )
        .bind(&status_str)
        .bind(duration_ms)
        .bind(error_msg.clone())
        .bind(files_affected)
        .bind(tracks_affected)
        .bind(operation_id)
        .execute(&self.db_pool)
        .await?;

        self.log_event(
            operation_id,
            "operation_completed",
            None,
            None,
            None,
            None,
            &serde_json::json!({
                "status": status_str,
                "duration_ms": duration_ms,
                "files_affected": files_affected,
                "tracks_affected": tracks_affected,
                "error_message": error_msg,
            }),
        )
        .await?;

        let mut active = self.active_operations.write().await;
        active.remove(operation_id);

        if matches!(
            status,
            OperationStatus::Success | OperationStatus::FailedAt(_) | OperationStatus::RolledBack
        ) {
            sqlx::query("DELETE FROM file_locks WHERE operation_id = ?1")
                .bind(operation_id)
                .execute(&self.db_pool)
                .await?;
        }

        tracing::info!(
            operation_id = operation_id,
            status = status_str,
            duration_ms = duration_ms,
            "Operation completed"
        );

        Ok(())
    }

    pub(crate) async fn calculate_operation_duration(&self, operation_id: &str) -> Result<i64> {
        let active = self.active_operations.read().await;
        if let Some(ctx) = active.get(operation_id) {
            let elapsed = Utc::now()
                .signed_duration_since(ctx.started_at)
                .num_milliseconds();
            return Ok(elapsed.max(0));
        }

        let started = sqlx::query_scalar::<_, Option<String>>(
            "SELECT started_at FROM operation_log WHERE operation_id = ?1",
        )
        .bind(operation_id)
        .fetch_one(&self.db_pool)
        .await?;

        if started.is_none() {
            return Err(ManagerError::OperationNotFound(operation_id.to_string()));
        }

        Ok(0)
    }
}
