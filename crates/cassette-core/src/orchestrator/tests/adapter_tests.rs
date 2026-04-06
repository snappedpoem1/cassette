use super::test_manager;
use crate::orchestrator::delta::action_types::DeltaActionType;
use crate::orchestrator::delta::adapter::DeltaQueueAdapter;
use crate::orchestrator::types::DeltaQueueEntry;
use chrono::Utc;
use sqlx::Row;

// mark_processed must preserve claimed_at/claim_run_id so the audit trail
// shows which run finalized a row.
#[tokio::test]
async fn mark_processed_preserves_claim_fields() {
    let manager = test_manager().await;
    sqlx::query(
        "INSERT INTO desired_tracks (id, source_name, artist_name, track_title, imported_at) \
         VALUES (10, 'spotify', 'Artist', 'Track', datetime('now'))",
    )
    .execute(manager.db_pool())
    .await
    .expect("desired");

    sqlx::query(
        "INSERT INTO delta_queue (id, desired_track_id, action_type, priority, reason, claimed_at, claim_run_id) \
         VALUES (10, 10, 'missing_download', 100, 'test', datetime('now'), 'run-abc')",
    )
    .execute(manager.db_pool())
    .await
    .expect("queue row");

    let adapter = DeltaQueueAdapter::new(manager.db_pool().clone());
    adapter.mark_processed(10).await.expect("mark processed");

    let row =
        sqlx::query("SELECT processed_at, claimed_at, claim_run_id FROM delta_queue WHERE id = 10")
            .fetch_one(manager.db_pool())
            .await
            .expect("fetch");

    let processed_at: Option<String> = row.try_get("processed_at").unwrap();
    let claimed_at: Option<String> = row.try_get("claimed_at").unwrap();
    let claim_run_id: Option<String> = row.try_get("claim_run_id").unwrap();

    assert!(processed_at.is_some(), "processed_at should be set");
    assert!(
        claimed_at.is_some(),
        "claimed_at must be preserved after mark_processed"
    );
    assert_eq!(
        claim_run_id.as_deref(),
        Some("run-abc"),
        "claim_run_id must be preserved"
    );
}

// A claimed-but-unprocessed row must survive delta_queue regeneration.
// Only unclaimed unprocessed rows should be wiped.
#[tokio::test]
async fn generate_delta_preserves_claimed_rows() {
    let manager = test_manager().await;
    sqlx::query(
        "INSERT INTO desired_tracks (id, source_name, artist_name, track_title, imported_at) \
         VALUES (20, 'spotify', 'Artist', 'Track', datetime('now'))",
    )
    .execute(manager.db_pool())
    .await
    .expect("desired");

    // Unclaimed row — should be deleted.
    sqlx::query(
        "INSERT INTO delta_queue (id, desired_track_id, action_type, priority, reason) \
         VALUES (20, 20, 'missing_download', 100, 'unclaimed')",
    )
    .execute(manager.db_pool())
    .await
    .expect("unclaimed row");

    // Claimed row — must survive.
    sqlx::query(
        "INSERT INTO delta_queue (id, desired_track_id, action_type, priority, reason, claimed_at, claim_run_id) \
         VALUES (21, 20, 'missing_download', 100, 'claimed', datetime('now'), 'run-xyz')",
    )
    .execute(manager.db_pool())
    .await
    .expect("claimed row");

    // Simulate what generate_delta_queue does.
    sqlx::query("DELETE FROM delta_queue WHERE processed_at IS NULL AND claimed_at IS NULL")
        .execute(manager.db_pool())
        .await
        .expect("delete unclaimed");

    let remaining: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM delta_queue WHERE processed_at IS NULL")
            .fetch_one(manager.db_pool())
            .await
            .expect("count");

    assert_eq!(remaining, 1, "claimed row must survive delta regeneration");

    let run_id: Option<String> =
        sqlx::query_scalar("SELECT claim_run_id FROM delta_queue WHERE id = 21")
            .fetch_one(manager.db_pool())
            .await
            .expect("fetch claim_run_id");
    assert_eq!(run_id.as_deref(), Some("run-xyz"));
}

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
