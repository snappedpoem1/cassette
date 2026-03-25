use crate::orchestrator::config::OrchestratorConfig;
use crate::orchestrator::types::LibrarianPhaseOutcome;
use sqlx::Row;

pub async fn check_cached_scan(
    manager: &crate::library::LibraryManager,
    _config: &OrchestratorConfig,
) -> Option<LibrarianPhaseOutcome> {
    let row = sqlx::query(
        r#"
        SELECT files_affected, tracks_affected
        FROM operation_log
        WHERE module = 'librarian'
          AND phase = 'scan'
          AND status = 'success'
          AND started_at > datetime('now', '-24 hours')
        ORDER BY started_at DESC
        LIMIT 1
        "#,
    )
    .fetch_optional(manager.db_pool())
    .await
    .ok()??;

    Some(LibrarianPhaseOutcome {
        files_scanned: row
            .try_get::<i64, _>("files_affected")
            .unwrap_or_default()
            .max(0) as usize,
        files_upserted: row
            .try_get::<i64, _>("tracks_affected")
            .unwrap_or_default()
            .max(0) as usize,
        files_quarantined: 0,
    })
}
