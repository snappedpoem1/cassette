use super::{seed_basic_catalog, test_manager};
use crate::librarian::models::DesiredTrack;
use crate::orchestrator::config::ReconciliationConfig;
use crate::orchestrator::reconciliation::engine::reconcile_desired_against_local;
use crate::orchestrator::types::ReconciliationStatus;

#[tokio::test]
async fn reconciliation_finds_exact_match_by_isrc() {
    let manager = test_manager().await;
    seed_basic_catalog(&manager).await;

    let desired = DesiredTrack {
        id: 1,
        source_name: "spotify".to_string(),
        source_track_id: Some("SPOTIFY123".to_string()),
        source_album_id: None,
        source_artist_id: None,
        artist_name: "Artist".to_string(),
        album_title: Some("Album".to_string()),
        track_title: "Song".to_string(),
        track_number: None,
        disc_number: None,
        duration_ms: Some(200000),
        isrc: Some("ISRC123".to_string()),
        raw_payload_json: None,
        imported_at: "".to_string(),
    };

    let result = reconcile_desired_against_local(
        &manager,
        "op-1",
        &[desired],
        &ReconciliationConfig::default(),
    )
    .await
    .expect("reconcile");

    assert_eq!(result.matched_count, 1);
    assert!(matches!(result.reconciliations[0].status, ReconciliationStatus::ExactMatch));
}
