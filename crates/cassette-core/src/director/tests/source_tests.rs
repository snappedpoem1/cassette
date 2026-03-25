use crate::director::sources::{HttpSource, LocalCacheSource, SourceProvider};
use crate::librarian::models::DesiredTrack;

#[tokio::test]
async fn http_source_parses_payload_url() {
    let source = HttpSource::new();
    let track = DesiredTrack {
        id: 31,
        source_name: "manual".to_string(),
        source_track_id: None,
        source_album_id: None,
        source_artist_id: None,
        artist_name: "Artist".to_string(),
        album_title: None,
        track_title: "Track".to_string(),
        track_number: None,
        disc_number: None,
        duration_ms: Some(1200),
        isrc: None,
        raw_payload_json: Some("{\"download_url\":\"https://example.com/a.mp3\"}".to_string()),
        imported_at: "".to_string(),
    };

    assert!(source.can_handle(&track));
    let resolved = source.resolve_download_url(&track).await.expect("resolve");
    assert!(resolved.download_url.contains("example.com"));
}

#[tokio::test]
async fn local_cache_source_returns_not_available() {
    let source = LocalCacheSource::new(vec![]);
    let track = DesiredTrack {
        id: 32,
        source_name: "manual".to_string(),
        source_track_id: None,
        source_album_id: None,
        source_artist_id: None,
        artist_name: "X".to_string(),
        album_title: None,
        track_title: "Y".to_string(),
        track_number: None,
        disc_number: None,
        duration_ms: None,
        isrc: None,
        raw_payload_json: None,
        imported_at: "".to_string(),
    };

    let result = source.resolve_download_url(&track).await;
    assert!(result.is_err());
}
