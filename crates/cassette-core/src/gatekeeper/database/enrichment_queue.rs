use crate::gatekeeper::error::Result;

pub async fn enqueue_enrichment(
    db_pool: &sqlx::SqlitePool,
    local_file_id: i64,
    track_id: Option<i64>,
    reason: &str,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO enrichment_queue (local_file_id, track_id, reason, status)
         VALUES (?1, ?2, ?3, 'pending')",
    )
    .bind(local_file_id)
    .bind(track_id)
    .bind(reason)
    .execute(db_pool)
    .await?;
    Ok(())
}
