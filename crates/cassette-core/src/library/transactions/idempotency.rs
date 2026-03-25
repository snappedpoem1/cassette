use crate::library::error::Result;
use crate::library::manager::LibraryManager;

impl LibraryManager {
    pub async fn find_existing_local_file_by_hash(
        &self,
        content_hash: &str,
    ) -> Result<Option<i64>> {
        let id = sqlx::query_scalar::<_, Option<i64>>(
            "SELECT id FROM local_files WHERE content_hash = ?1 LIMIT 1",
        )
        .bind(content_hash)
        .fetch_one(&self.db_pool)
        .await?;
        Ok(id)
    }

    pub async fn operation_has_event_type(
        &self,
        operation_id: &str,
        event_type: &str,
    ) -> Result<bool> {
        let exists = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM operation_events WHERE operation_id = ?1 AND event_type = ?2",
        )
        .bind(operation_id)
        .bind(event_type)
        .fetch_one(&self.db_pool)
        .await?;
        Ok(exists > 0)
    }
}
