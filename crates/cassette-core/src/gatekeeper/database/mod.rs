pub mod audit_log;
pub mod enrichment_queue;
pub mod local_files;

use crate::gatekeeper::error::Result;

pub async fn ensure_schema(db_pool: &sqlx::SqlitePool) -> Result<()> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS gatekeeper_audit_log (
            id INTEGER PRIMARY KEY,
            operation_id TEXT NOT NULL,
            timestamp TEXT NOT NULL,
            file_path TEXT NOT NULL,
            decision TEXT NOT NULL,
            desired_track_id INTEGER,
            matched_local_file_id INTEGER,
            duration_ms INTEGER NOT NULL DEFAULT 0,
            notes TEXT NOT NULL DEFAULT '',
            quality_json TEXT,
            identity_json TEXT,
            junk_flags_json TEXT,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        )",
    )
    .execute(db_pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS enrichment_queue (
            id INTEGER PRIMARY KEY,
            local_file_id INTEGER NOT NULL,
            track_id INTEGER,
            reason TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending',
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            processed_at TIMESTAMP
        )",
    )
    .execute(db_pool)
    .await?;

    // Keep this best-effort to support older databases.
    let _ = sqlx::query("ALTER TABLE local_files ADD COLUMN acoustid_fingerprint TEXT")
        .execute(db_pool)
        .await;

    Ok(())
}
