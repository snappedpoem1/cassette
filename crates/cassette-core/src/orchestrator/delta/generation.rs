use crate::library::LibraryManager;
use crate::orchestrator::delta::action_types::DeltaActionType;
use crate::orchestrator::error::Result;
use crate::orchestrator::types::{DeltaQueueEntry, ReconciliationResult, ReconciliationStatus};
use chrono::Utc;
use sqlx::Row;
use std::collections::HashMap;

pub async fn generate_delta_queue_managed(
    manager: &LibraryManager,
    operation_id: &str,
    reconciliation: &ReconciliationResult,
) -> Result<Vec<DeltaQueueEntry>> {
    ensure_delta_source_operation_column(manager.db_pool()).await?;

    sqlx::query("DELETE FROM delta_queue WHERE processed_at IS NULL")
        .execute(manager.db_pool())
        .await?;

    let mut deltas = Vec::new();
    for (index, reconcile) in reconciliation.reconciliations.iter().enumerate() {
        let (action_type, priority, reason) = match &reconcile.status {
            ReconciliationStatus::ExactMatch => {
                (DeltaActionType::NoAction, 0, "Exact match in library".to_string())
            }
            ReconciliationStatus::HighConfidenceMatch => {
                if reconcile
                    .matched_local_file
                    .as_ref()
                    .map(|m| m.quality_tier.as_str())
                    == Some("below_floor")
                {
                    (
                        DeltaActionType::UpgradeQuality,
                        3,
                        "Quality below floor; upgrade recommended".to_string(),
                    )
                } else {
                    (DeltaActionType::NoAction, 0, "Acceptable quality match".to_string())
                }
            }
            ReconciliationStatus::FuzzyMatch => (
                DeltaActionType::ManualReview,
                2,
                format!("Fuzzy match ({:.0}% confidence)", reconcile.confidence * 100.0),
            ),
            ReconciliationStatus::DuplicateFound { better_candidate } => (
                DeltaActionType::DuplicateReview,
                1,
                format!("Consolidate duplicates to preferred file {better_candidate}"),
            ),
            ReconciliationStatus::Missing => (
                DeltaActionType::MissingDownload,
                4,
                "Not found in local library; download required".to_string(),
            ),
            ReconciliationStatus::ManualReviewNeeded => (
                DeltaActionType::ManualReview,
                2,
                format!(
                    "Possible match ({:.0}%); requires manual decision",
                    reconcile.confidence * 100.0
                ),
            ),
        };

        if matches!(action_type, DeltaActionType::NoAction) {
            continue;
        }

        let created_at = Utc::now();
        sqlx::query(
            r#"
            INSERT INTO delta_queue
              (desired_track_id, action_type, priority, reason, target_quality, source_operation_id, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
        )
        .bind(reconcile.desired_track_id as i64)
        .bind(action_type.as_str())
        .bind(priority)
        .bind(&reason)
        .bind(Some("lossless".to_string()))
        .bind(operation_id)
        .bind(created_at.to_rfc3339())
        .execute(manager.db_pool())
        .await?;

        let id = sqlx::query_scalar::<_, i64>("SELECT last_insert_rowid()")
            .fetch_one(manager.db_pool())
            .await?
            as u64;

        let delta = DeltaQueueEntry {
            id,
            desired_track_id: reconcile.desired_track_id,
            action_type: action_type.clone(),
            priority,
            reason: reason.clone(),
            target_quality: Some("lossless".to_string()),
            source_reconciliation_id: index as u64,
            source_operation_id: operation_id.to_string(),
            created_at,
            processed_at: None,
        };

        let _ = manager
            .log_event(
                operation_id,
                &format!("delta_{}", action_type.as_str()),
                None,
                None,
                None,
                None,
                &serde_json::json!({
                    "desired_track_id": reconcile.desired_track_id,
                    "action": action_type.as_str(),
                    "priority": priority,
                    "reason": reason,
                    "confidence": reconcile.confidence,
                }),
            )
            .await;

        deltas.push(delta);
    }

    tracing::info!(
        operation_id = operation_id,
        total_deltas = deltas.len(),
        by_action = ?count_by_action(&deltas),
        "Delta queue generation complete"
    );

    Ok(deltas)
}

fn count_by_action(deltas: &[DeltaQueueEntry]) -> HashMap<String, usize> {
    let mut counts = HashMap::new();
    for delta in deltas {
        *counts.entry(delta.action_type.to_string()).or_insert(0) += 1;
    }
    counts
}

async fn ensure_delta_source_operation_column(pool: &sqlx::SqlitePool) -> Result<()> {
    let columns = sqlx::query("PRAGMA table_info(delta_queue)")
        .fetch_all(pool)
        .await?;
    let has_source_operation_id = columns.iter().any(|c| {
        c.try_get::<String, _>("name")
            .map(|name| name == "source_operation_id")
            .unwrap_or(false)
    });

    if !has_source_operation_id {
        sqlx::query("ALTER TABLE delta_queue ADD COLUMN source_operation_id TEXT")
            .execute(pool)
            .await?;
    }

    Ok(())
}
