use crate::library::error::Result;
use crate::library::manager::LibraryManager;

impl LibraryManager {
    pub async fn log_event(
        &self,
        operation_id: &str,
        event_type: &str,
        target_file_id: Option<u64>,
        target_track_id: Option<u64>,
        before_state: Option<&serde_json::Value>,
        after_state: Option<&serde_json::Value>,
        details: &serde_json::Value,
    ) -> Result<()> {
        if !self.config.enable_operation_event_logging {
            return Ok(());
        }

        sqlx::query(
            r#"
            INSERT INTO operation_events
              (operation_id, event_type, target_file_id, target_track_id, before_state_json, after_state_json, event_data)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
        )
        .bind(operation_id)
        .bind(event_type)
        .bind(target_file_id.map(|v| v as i64))
        .bind(target_track_id.map(|v| v as i64))
        .bind(before_state.map(|s| s.to_string()))
        .bind(after_state.map(|s| s.to_string()))
        .bind(details.to_string())
        .execute(&self.db_pool)
        .await?;

        tracing::debug!(
            operation_id = operation_id,
            event_type = event_type,
            target_file_id = ?target_file_id,
            target_track_id = ?target_track_id,
            "Event logged"
        );

        Ok(())
    }
}
