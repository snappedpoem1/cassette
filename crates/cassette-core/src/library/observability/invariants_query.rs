use crate::library::error::Result;
use crate::library::manager::LibraryManager;
use crate::library::types::InvariantViolation;

impl LibraryManager {
    pub async fn get_invariant_violations(&self) -> Result<Vec<InvariantViolation>> {
        let rows = sqlx::query_as::<_, InvariantViolation>(
            "SELECT violation_id, invariant_name, violating_record_json, attempted_operation_id, detected_at, resolved FROM invariant_violations WHERE resolved = FALSE ORDER BY detected_at DESC",
        )
        .fetch_all(&self.db_pool)
        .await?;
        Ok(rows)
    }
}
