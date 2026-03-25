use crate::library::error::{ManagerError, Result};
use crate::library::manager::LibraryManager;

impl LibraryManager {
    pub async fn resume_operation(&self, operation_id: &str) -> Result<()> {
        let status = sqlx::query_scalar::<_, Option<String>>(
            "SELECT status FROM operation_log WHERE operation_id = ?1",
        )
        .bind(operation_id)
        .fetch_one(&self.db_pool)
        .await?;

        let Some(status) = status else {
            return Err(ManagerError::OperationNotFound(operation_id.to_string()));
        };

        if status != "in_progress" {
            return Err(ManagerError::OperationNotPending(operation_id.to_string()));
        }

        tracing::info!(
            operation_id = operation_id,
            "Operation resumed; module should inspect operation_events for resume point"
        );

        Ok(())
    }
}
