use super::test_manager;
use crate::orchestrator::delta::action_types::DeltaActionType;
use crate::orchestrator::delta::adapter::DeltaQueueAdapter;
use crate::orchestrator::types::DeltaQueueEntry;
use chrono::Utc;

#[tokio::test]
async fn adapter_extracts_download_tracks() {
    let manager = test_manager().await;
    sqlx::query("INSERT INTO desired_tracks (id, source_name, artist_name, track_title, imported_at) VALUES (1, 'spotify', 'Artist', 'Track', datetime('now'))")
        .execute(manager.db_pool())
        .await
        .expect("desired");

    let adapter = DeltaQueueAdapter::new(manager.db_pool().clone());
    let delta = DeltaQueueEntry {
        id: 1,
        desired_track_id: 1,
        action_type: DeltaActionType::MissingDownload,
        priority: 4,
        reason: "missing".to_string(),
        target_quality: Some("lossless".to_string()),
        source_reconciliation_id: 1,
        source_operation_id: "op".to_string(),
        created_at: Utc::now(),
        processed_at: None,
    };

    let tracks = adapter
        .extract_desired_tracks_for_download(&[delta])
        .await
        .expect("extract");

    assert_eq!(tracks.len(), 1);
    assert_eq!(tracks[0].id, 1);
}
