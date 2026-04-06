use super::{seed_basic_catalog, test_manager};
use crate::librarian::models::DesiredTrack;
use crate::orchestrator::config::ReconciliationConfig;
use crate::orchestrator::delta::action_types::DeltaActionType;
use crate::orchestrator::delta::generation::generate_delta_queue_managed;
use crate::orchestrator::reconciliation::engine::reconcile_desired_against_local;

#[tokio::test]
async fn delta_generation_creates_missing_download_action() {
    let manager = test_manager().await;
    seed_basic_catalog(&manager).await;

    let desired = DesiredTrack {
        id: 99,
        source_name: "spotify".to_string(),
        source_track_id: None,
        source_album_id: None,
        source_artist_id: None,
        artist_name: "Unknown".to_string(),
        album_title: Some("Unknown".to_string()),
        track_title: "Missing".to_string(),
        track_number: None,
        disc_number: None,
        duration_ms: Some(100000),
        isrc: None,
        raw_payload_json: None,
        imported_at: "".to_string(),
    };

    let reconciliation = reconcile_desired_against_local(
        &manager,
        "op-delta",
        &[desired],
        &ReconciliationConfig::default(),
    )
    .await
    .expect("reconcile");

    let deltas = generate_delta_queue_managed(&manager, "op-delta", &reconciliation)
        .await
        .expect("deltas");

    assert_eq!(deltas.len(), 1);
    assert!(matches!(
        deltas[0].action_type,
        DeltaActionType::MissingDownload
    ));
}
