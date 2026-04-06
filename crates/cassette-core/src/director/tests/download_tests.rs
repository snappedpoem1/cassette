use super::{test_director_config, test_manager};
use crate::director::download::{check_existing_staged_file, download_file};
use crate::director::sources::{LocalCacheSource, SourceProvider};
use crate::librarian::models::DesiredTrack;
use crate::library::Module;
use std::collections::HashMap;
use std::sync::Arc;

#[tokio::test]
async fn download_file_stages_from_local_cache() {
    let manager = test_manager().await;

    let source_dir = tempfile::tempdir().expect("source dir");
    let staging_dir = tempfile::tempdir().expect("staging dir");
    let source_file = source_dir.path().join("Artist One - Song One.flac");
    tokio::fs::write(&source_file, b"audio-bytes")
        .await
        .expect("write source");

    let config = test_director_config(staging_dir.path().to_path_buf());
    let sources: Vec<Arc<dyn SourceProvider>> =
        vec![Arc::new(LocalCacheSource::new(vec![source_dir
            .path()
            .to_path_buf()]))];

    let track = DesiredTrack {
        id: 11,
        source_name: "manual".to_string(),
        source_track_id: None,
        source_album_id: None,
        source_artist_id: None,
        artist_name: "Artist One".to_string(),
        album_title: Some("Album One".to_string()),
        track_title: "Song One".to_string(),
        track_number: Some(1),
        disc_number: Some(1),
        duration_ms: Some(1000),
        isrc: None,
        raw_payload_json: None,
        imported_at: "".to_string(),
    };

    let op_id = manager
        .start_operation(Module::Director, "download_test")
        .await
        .expect("op");

    let provider_semaphores = Arc::new(HashMap::new());
    let staged = download_file(
        &manager,
        &track,
        &sources,
        &config,
        &op_id,
        &provider_semaphores,
    )
    .await
    .expect("download");
    assert!(staged.path.exists());

    let existing = check_existing_staged_file(&manager, &track, &config)
        .await
        .expect("check existing");
    assert!(existing.is_some());
}
