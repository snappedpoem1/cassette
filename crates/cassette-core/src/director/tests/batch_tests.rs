use super::{test_director_config, test_manager};
use crate::director::download::batch_download;
use crate::director::sources::{LocalCacheSource, SourceProvider};
use crate::librarian::models::DesiredTrack;
use std::sync::Arc;

#[tokio::test]
async fn batch_download_reports_partial_failures() {
    let manager = test_manager().await;
    let source_dir = tempfile::tempdir().expect("source dir");
    let staging_dir = tempfile::tempdir().expect("staging dir");
    tokio::fs::write(source_dir.path().join("A - Good.flac"), b"ok")
        .await
        .expect("write good");

    let config = test_director_config(staging_dir.path().to_path_buf());
    let sources: Vec<Arc<dyn SourceProvider>> = vec![Arc::new(LocalCacheSource::new(vec![
        source_dir.path().to_path_buf(),
    ]))];

    let good = DesiredTrack {
        id: 21,
        source_name: "manual".to_string(),
        source_track_id: None,
        source_album_id: None,
        source_artist_id: None,
        artist_name: "A".to_string(),
        album_title: None,
        track_title: "Good".to_string(),
        track_number: None,
        disc_number: None,
        duration_ms: Some(1000),
        isrc: None,
        raw_payload_json: None,
        imported_at: "".to_string(),
    };
    let bad = DesiredTrack {
        id: 22,
        source_name: "manual".to_string(),
        source_track_id: None,
        source_album_id: None,
        source_artist_id: None,
        artist_name: "B".to_string(),
        album_title: None,
        track_title: "Missing".to_string(),
        track_number: None,
        disc_number: None,
        duration_ms: Some(1000),
        isrc: None,
        raw_payload_json: None,
        imported_at: "".to_string(),
    };

    let outcome = batch_download(&manager, &[good, bad], &sources, &config)
        .await
        .expect("batch outcome");
    assert_eq!(outcome.total_requested, 2);
    assert_eq!(outcome.successfully_downloaded, 1);
    assert_eq!(outcome.failed_downloads.len(), 1);
}
