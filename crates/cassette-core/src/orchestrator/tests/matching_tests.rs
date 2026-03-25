use crate::librarian::models::DesiredTrack;
use crate::orchestrator::reconciliation::normalization::normalize_name;
use crate::orchestrator::reconciliation::scoring::compute_metadata_confidence;
use crate::orchestrator::types::{LocalFileMatch, MatchMethod};

#[test]
fn normalization_strips_punctuation_and_case() {
    assert_eq!(normalize_name("  The-ARTIST!!!  "), "theartist");
}

#[test]
fn metadata_confidence_scores_exact_high() {
    let desired = DesiredTrack {
        id: 1,
        source_name: "src".to_string(),
        source_track_id: None,
        source_album_id: None,
        source_artist_id: None,
        artist_name: "Artist".to_string(),
        album_title: Some("Album".to_string()),
        track_title: "Song".to_string(),
        track_number: None,
        disc_number: None,
        duration_ms: Some(200000),
        isrc: None,
        raw_payload_json: None,
        imported_at: "".to_string(),
    };

    let local = LocalFileMatch {
        file_id: 1,
        track_id: Some(1),
        file_path: std::path::PathBuf::from("C:/music/song.flac"),
        file_name: "song.flac".to_string(),
        codec: "flac".to_string(),
        bitrate: 900,
        quality_tier: "lossless_preferred".to_string(),
        artist_name: "Artist".to_string(),
        album_title: "Album".to_string(),
        title: "Song".to_string(),
        duration_ms: 200000,
        content_hash: None,
        acoustid_fingerprint: None,
        matched_via: MatchMethod::StrongMetadata,
    };

    let confidence = compute_metadata_confidence(&desired, &local, 2_000);
    assert!(confidence >= 0.9);
}
