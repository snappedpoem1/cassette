use crate::library::error::Result;
use crate::library::manager::LibraryManager;
use crate::library::state::Module;
use crate::library::types::{OperationDetails, OperationRecord};

impl LibraryManager {
    pub async fn get_operations_by_module(&self, module: Module) -> Result<Vec<OperationRecord>> {
        let records = sqlx::query_as::<_, OperationRecord>(
            "SELECT operation_id, module, phase, status, started_at, ended_at, duration_ms, error_message, error_context_json, files_affected, tracks_affected, metadata_json, created_at FROM operation_log WHERE module = ?1 ORDER BY started_at DESC",
        )
        .bind(module.as_str())
        .fetch_all(&self.db_pool)
        .await?;
        Ok(records)
    }

    pub async fn get_in_progress_operations(&self) -> Result<Vec<OperationRecord>> {
        let records = sqlx::query_as::<_, OperationRecord>(
            "SELECT operation_id, module, phase, status, started_at, ended_at, duration_ms, error_message, error_context_json, files_affected, tracks_affected, metadata_json, created_at FROM operation_log WHERE status = 'in_progress' ORDER BY started_at ASC",
        )
        .fetch_all(&self.db_pool)
        .await?;
        Ok(records)
    }

    pub async fn get_operation_details(&self, operation_id: &str) -> Result<OperationDetails> {
        let operation = sqlx::query_as::<_, OperationRecord>(
            "SELECT operation_id, module, phase, status, started_at, ended_at, duration_ms, error_message, error_context_json, files_affected, tracks_affected, metadata_json, created_at FROM operation_log WHERE operation_id = ?1",
        )
        .bind(operation_id)
        .fetch_one(&self.db_pool)
        .await?;

        let events = self.get_events_for_operation(operation_id).await?;
        Ok(OperationDetails { operation, events })
    }
}
