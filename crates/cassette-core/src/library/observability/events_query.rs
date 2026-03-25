use crate::library::error::Result;
use crate::library::manager::LibraryManager;
use crate::library::types::OperationEvent;

impl LibraryManager {
    pub async fn get_events_for_operation(&self, operation_id: &str) -> Result<Vec<OperationEvent>> {
        let events = sqlx::query_as::<_, OperationEvent>(
            "SELECT event_id, operation_id, event_type, target_file_id, target_track_id, before_state_json, after_state_json, event_data, timestamp FROM operation_events WHERE operation_id = ?1 ORDER BY event_id ASC",
        )
        .bind(operation_id)
        .fetch_all(&self.db_pool)
        .await?;
        Ok(events)
    }
}
