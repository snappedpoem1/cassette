use crate::library::error::Result;
use crate::library::manager::LibraryManager;
use crate::library::types::{FileLineage, FileLineageEvent};
use std::path::Path;

impl LibraryManager {
    pub async fn get_file_lineage(&self, file_path: &Path) -> Result<FileLineage> {
        let events = sqlx::query_as::<_, FileLineageEvent>(
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
            WHERE oe.target_file_id = (
                SELECT id FROM local_files WHERE file_path = ?1
            )
            ORDER BY oe.event_id ASC
            "#,
        )
        .bind(file_path.to_string_lossy().to_string())
        .fetch_all(&self.db_pool)
        .await?;

        Ok(FileLineage {
            file_path: file_path.to_path_buf(),
            events,
        })
    }
}
