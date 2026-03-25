use crate::librarian::models::DesiredTrack;
use crate::orchestrator::delta::action_types::DeltaActionType;
use crate::orchestrator::error::{OrchestratorError, Result};
use crate::orchestrator::types::DeltaQueueEntry;

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
}
