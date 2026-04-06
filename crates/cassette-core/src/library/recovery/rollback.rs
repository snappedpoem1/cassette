use crate::library::error::Result;
use crate::library::manager::LibraryManager;
use crate::library::state::OperationStatus;
use crate::library::types::{OperationEvent, RollbackReport};

impl LibraryManager {
    pub async fn rollback_operation(&self, operation_id: &str) -> Result<RollbackReport> {
        let events = self.get_undo_events_reverse(operation_id).await?;

        let mut rolled_back = 0usize;
        let mut failed_rollbacks = 0usize;

        for event in events {
            match self.undo_event(&event).await {
                Ok(_) => {
                    rolled_back += 1;
                    tracing::info!(
                        operation_id = operation_id,
                        event_id = event.event_id,
                        "Event rolled back"
                    );
                }
                Err(error) => {
                    failed_rollbacks += 1;
                    tracing::error!(operation_id = operation_id, event_id = event.event_id, error = %error, "Failed to roll back event");
                }
            }
        }

        self.complete_operation(operation_id, OperationStatus::RolledBack)
            .await?;

        Ok(RollbackReport {
            operation_id: operation_id.to_string(),
            events_rolled_back: rolled_back,
            failed_rollbacks,
        })
    }

    pub(crate) async fn undo_event(&self, event: &OperationEvent) -> Result<()> {
        match event.event_type.as_str() {
            "file_admitted" => {
                if let Some(file_id) = event.target_file_id {
                    sqlx::query("DELETE FROM local_files WHERE id = ?1")
                        .bind(file_id)
                        .execute(&self.db_pool)
                        .await?;
                }
            }
            "track_updated" => {
                if let (Some(track_id), Some(before_json)) =
                    (event.target_track_id, event.before_state_json.as_ref())
                {
                    let before: serde_json::Value = serde_json::from_str(before_json)?;
                    let title = before
                        .get("title")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string();
                    sqlx::query("UPDATE tracks SET title = ?1 WHERE id = ?2")
                        .bind(title)
                        .bind(track_id)
                        .execute(&self.db_pool)
                        .await?;
                }
            }
            _ => {}
        }
        Ok(())
    }
}
