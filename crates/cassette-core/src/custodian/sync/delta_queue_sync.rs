use crate::custodian::error::Result;

pub async fn mark_quarantine_delta(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    local_file_id: i64,
    track_id: Option<i64>,
    reason: &str,
) -> Result<()> {
    let desired_track_id = resolve_desired_track_id(tx, local_file_id, track_id).await?;
    let Some(desired_track_id) = desired_track_id else {
        tracing::debug!(
            local_file_id,
            track_id,
            reason,
            "Skipping delta_queue insert for quarantined file without a desired track mapping"
        );
        return Ok(());
    };

    let action = if track_id.is_some() {
        "manual_review"
    } else {
        "missing_download"
    };

    sqlx::query(
        "INSERT INTO delta_queue (desired_track_id, action_type, priority, reason, target_quality)
         VALUES (?1, ?2, ?3, ?4, ?5)",
    )
    .bind(desired_track_id)
    .bind(action)
    .bind(5_i64)
    .bind(format!("Original file quarantined: {reason}"))
    .bind(Some("lossless_preferred"))
    .execute(tx.as_mut())
    .await?;

    Ok(())
}

async fn resolve_desired_track_id(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    local_file_id: i64,
    track_id: Option<i64>,
) -> Result<Option<i64>> {
    let desired_from_local = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT desired_track_id
        FROM reconciliation_results
        WHERE matched_local_file_id = ?1
        ORDER BY created_at DESC, id DESC
        LIMIT 1
        "#,
    )
    .bind(local_file_id)
    .fetch_optional(tx.as_mut())
    .await?;

    if desired_from_local.is_some() {
        return Ok(desired_from_local);
    }

    let Some(track_id) = track_id else {
        return Ok(None);
    };

    let desired_from_track = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT desired_track_id
        FROM reconciliation_results
        WHERE matched_track_id = ?1
        ORDER BY created_at DESC, id DESC
        LIMIT 1
        "#,
    )
    .bind(track_id)
    .fetch_optional(tx.as_mut())
    .await?;

    Ok(desired_from_track)
}
