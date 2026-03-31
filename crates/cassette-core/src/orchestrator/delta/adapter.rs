use crate::librarian::models::DesiredTrack;
use crate::orchestrator::delta::action_types::DeltaActionType;
use crate::orchestrator::error::{OrchestratorError, Result};
use crate::orchestrator::types::DeltaQueueEntry;
use sqlx::Row;

#[derive(Debug, Clone)]
pub struct ClaimedDownloadRow {
    pub delta_id: u64,
    pub action_type: String,
    pub desired: DesiredTrack,
    pub source_operation_id: String,
}

#[derive(Clone)]
pub struct DeltaQueueAdapter {
    db_pool: sqlx::SqlitePool,
}

impl DeltaQueueAdapter {
    pub fn new(db_pool: sqlx::SqlitePool) -> Self {
        Self { db_pool }
    }

    pub async fn extract_desired_tracks_for_download(
        &self,
        delta_queue: &[DeltaQueueEntry],
    ) -> Result<Vec<DesiredTrack>> {
        let mut desired_tracks = Vec::new();
        for delta in delta_queue {
            if !matches!(
                delta.action_type,
                DeltaActionType::MissingDownload | DeltaActionType::UpgradeQuality
            ) {
                continue;
            }

            let desired = sqlx::query_as::<_, DesiredTrack>(
                "SELECT id, source_name, source_track_id, source_album_id, source_artist_id, artist_name, album_title, track_title, track_number, disc_number, duration_ms, isrc, raw_payload_json, imported_at FROM desired_tracks WHERE id = ?1",
            )
            .bind(delta.desired_track_id as i64)
            .fetch_optional(&self.db_pool)
            .await?
            .ok_or(OrchestratorError::DesiredTrackNotFound(delta.desired_track_id))?;

            desired_tracks.push(desired);
        }
        Ok(desired_tracks)
    }

    pub async fn extract_desired_tracks_for_review(
        &self,
        delta_queue: &[DeltaQueueEntry],
    ) -> Result<Vec<(DesiredTrack, String)>> {
        let mut review_items = Vec::new();
        for delta in delta_queue {
            if !matches!(
                delta.action_type,
                DeltaActionType::ManualReview | DeltaActionType::DuplicateReview
            ) {
                continue;
            }

            let desired = sqlx::query_as::<_, DesiredTrack>(
                "SELECT id, source_name, source_track_id, source_album_id, source_artist_id, artist_name, album_title, track_title, track_number, disc_number, duration_ms, isrc, raw_payload_json, imported_at FROM desired_tracks WHERE id = ?1",
            )
            .bind(delta.desired_track_id as i64)
            .fetch_optional(&self.db_pool)
            .await?
            .ok_or(OrchestratorError::DesiredTrackNotFound(delta.desired_track_id))?;

            review_items.push((desired, delta.reason.clone()));
        }
        Ok(review_items)
    }

    pub async fn link_delta_to_operation(&self, delta_id: u64, operation_id: &str) -> Result<()> {
        sqlx::query("UPDATE delta_queue SET source_operation_id = ?1 WHERE id = ?2")
            .bind(operation_id)
            .bind(delta_id as i64)
            .execute(&self.db_pool)
            .await?;
        Ok(())
    }

    pub async fn reclaim_stale_claims(&self, stale_before_minutes: i64) -> Result<u64> {
        let result = sqlx::query(
            "UPDATE delta_queue
             SET claimed_at = NULL, claim_run_id = NULL
             WHERE processed_at IS NULL
               AND claimed_at IS NOT NULL
               AND claimed_at < datetime('now', ?1)",
        )
        .bind(format!("-{stale_before_minutes} minutes"))
        .execute(&self.db_pool)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn claim_download_rows(
        &self,
        claim_run_id: &str,
        limit: usize,
    ) -> Result<Vec<ClaimedDownloadRow>> {
        let mut tx = self.db_pool.begin().await?;
        let rows = sqlx::query(
            "SELECT dq.id AS delta_id,
                    dq.action_type AS action_type,
                    dq.source_operation_id AS source_operation_id,
                    d.id AS desired_id,
                    d.source_name,
                    d.source_track_id,
                    d.source_album_id,
                    d.source_artist_id,
                    d.artist_name,
                    d.album_title,
                    d.track_title,
                    d.track_number,
                    d.disc_number,
                    d.duration_ms,
                    d.isrc,
                    d.raw_payload_json,
                    d.imported_at
             FROM delta_queue dq
             INNER JOIN desired_tracks d ON d.id = dq.desired_track_id
             WHERE dq.action_type IN ('missing_download', 'upgrade_quality')
               AND dq.processed_at IS NULL
               AND dq.claimed_at IS NULL
             ORDER BY dq.priority DESC, dq.created_at ASC, dq.desired_track_id ASC
             LIMIT ?1",
        )
        .bind(limit as i64)
        .fetch_all(&mut *tx)
        .await?;

        let mut claimed = Vec::with_capacity(rows.len());
        for row in rows {
            let delta_id: i64 = row.try_get("delta_id")?;
            sqlx::query(
                "UPDATE delta_queue
                 SET claimed_at = CURRENT_TIMESTAMP, claim_run_id = ?1
                 WHERE id = ?2 AND processed_at IS NULL AND claimed_at IS NULL",
            )
            .bind(claim_run_id)
            .bind(delta_id)
            .execute(&mut *tx)
            .await?;

            let desired = DesiredTrack {
                id: row.try_get("desired_id")?,
                source_name: row.try_get("source_name")?,
                source_track_id: row.try_get("source_track_id")?,
                source_album_id: row.try_get("source_album_id")?,
                source_artist_id: row.try_get("source_artist_id")?,
                artist_name: row.try_get("artist_name")?,
                album_title: row.try_get("album_title")?,
                track_title: row.try_get("track_title")?,
                track_number: row.try_get("track_number")?,
                disc_number: row.try_get("disc_number")?,
                duration_ms: row.try_get("duration_ms")?,
                isrc: row.try_get("isrc")?,
                raw_payload_json: row.try_get("raw_payload_json")?,
                imported_at: row.try_get("imported_at")?,
            };
            let source_operation_id = row
                .try_get::<Option<String>, _>("source_operation_id")?
                .unwrap_or_default();
            let action_type: String = row.try_get("action_type")?;
            claimed.push(ClaimedDownloadRow {
                delta_id: delta_id as u64,
                action_type,
                desired,
                source_operation_id,
            });
        }

        tx.commit().await?;
        Ok(claimed)
    }

    pub async fn mark_processed(&self, desired_track_id: i64) -> Result<()> {
        sqlx::query(
            "UPDATE delta_queue
             SET processed_at = CURRENT_TIMESTAMP, claimed_at = NULL, claim_run_id = NULL
             WHERE desired_track_id = ?1 AND processed_at IS NULL",
        )
        .bind(desired_track_id)
        .execute(&self.db_pool)
        .await?;
        Ok(())
    }

    pub async fn release_claim(&self, desired_track_id: i64) -> Result<()> {
        sqlx::query(
            "UPDATE delta_queue
             SET claimed_at = NULL, claim_run_id = NULL
             WHERE desired_track_id = ?1 AND processed_at IS NULL",
        )
        .bind(desired_track_id)
        .execute(&self.db_pool)
        .await?;
        Ok(())
    }
}
