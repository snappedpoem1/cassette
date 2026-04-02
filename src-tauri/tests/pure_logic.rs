#![allow(dead_code)]

#[path = "../src/now_playing.rs"]
mod now_playing;
#[path = "../src/pending_recovery.rs"]
mod pending_recovery;
#[path = "../src/runtime_bootstrap.rs"]
mod runtime_bootstrap;
#[path = "../src/spotify_history.rs"]
mod spotify_history;

use cassette_core::db::{PendingDirectorTask, TerminalDirectorTaskUpdate};
use cassette_core::director::{AcquisitionStrategy, NormalizedTrack, TrackTask, TrackTaskSource};
use std::collections::HashMap;

fn pending_task(task_id: &str, progress: &str, updated_at: &str) -> PendingDirectorTask {
    PendingDirectorTask {
        task: TrackTask {
            task_id: task_id.to_string(),
            source: TrackTaskSource::Manual,
            desired_track_id: None,
            source_operation_id: None,
            target: NormalizedTrack {
                spotify_track_id: None,
                source_playlist: None,
                artist: "Artist".to_string(),
                album_artist: Some("Artist".to_string()),
                title: "Song".to_string(),
                album: Some("Album".to_string()),
                track_number: Some(1),
                disc_number: Some(1),
                year: Some(2024),
                duration_secs: Some(35.0),
                isrc: None,
                musicbrainz_recording_id: None,
                musicbrainz_release_id: None,
                canonical_artist_id: None,
                canonical_release_id: None,
            },
            strategy: AcquisitionStrategy::ObscureFallbackHeavy,
        },
        strategy: "ObscureFallbackHeavy".to_string(),
        progress: progress.to_string(),
        created_at: updated_at.to_string(),
        updated_at: updated_at.to_string(),
    }
}

#[test]
fn pending_recovery_plan_keeps_newer_retry_and_drops_stale_terminal_row() {
    let pending = vec![
        pending_task("stale-failed", "Queued", "2026-03-27 12:00:00"),
        pending_task("retry-failed", "Queued", "2026-03-27 12:00:03"),
        pending_task("terminal-progress", "Cancelled", "2026-03-27 12:00:04"),
    ];
    let terminal_updates = HashMap::from([
        (
            "stale-failed".to_string(),
            TerminalDirectorTaskUpdate {
                disposition: "Failed".to_string(),
                updated_at: "2026-03-27 12:00:02".to_string(),
            },
        ),
        (
            "retry-failed".to_string(),
            TerminalDirectorTaskUpdate {
                disposition: "Failed".to_string(),
                updated_at: "2026-03-27 12:00:01".to_string(),
            },
        ),
    ]);

    let plan = pending_recovery::build_pending_recovery_plan(pending, &terminal_updates);

    assert_eq!(
        plan.resumable_tasks
            .iter()
            .map(|task| task.task.task_id.as_str())
            .collect::<Vec<_>>(),
        vec!["retry-failed"]
    );
    assert_eq!(
        plan.stale_task_ids,
        vec!["stale-failed".to_string(), "terminal-progress".to_string()]
    );
}

#[test]
fn spotify_history_parser_summarizes_and_sorts() {
    let dir = std::env::temp_dir().join(format!("cassette-spotify-history-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("temp dir should be created");
    let file = dir.join("Streaming_History_Audio_2026.json");
    let payload = r#"
[
  {
    "master_metadata_album_artist_name": "The National",
    "master_metadata_album_album_name": "Boxer",
    "ms_played": 200000,
    "skipped": false
  },
  {
    "master_metadata_album_artist_name": "The National",
    "master_metadata_album_album_name": "Boxer",
    "ms_played": 100000,
    "skipped": true
  },
  {
    "master_metadata_album_artist_name": "Interpol",
    "master_metadata_album_album_name": "Turn on the Bright Lights",
    "ms_played": 250000,
    "skipped": false
  }
]
"#;
    std::fs::write(&file, payload).expect("fixture should be written");

    let files = spotify_history::collect_spotify_history_files(&dir).expect("history files should be found");
    assert_eq!(files.len(), 1);
    let entries = spotify_history::parse_spotify_entries(&files).expect("entries should parse");
    assert_eq!(entries.len(), 3);

    let mut library_set = HashMap::new();
    library_set.insert(("the national".to_string(), "boxer".to_string()), true);
    let albums = spotify_history::summarize_spotify_albums(&entries, &library_set);

    assert_eq!(albums.len(), 2);
    assert_eq!(albums[0].artist, "The National");
    assert_eq!(albums[0].album, "Boxer");
    assert_eq!(albums[0].total_ms, 300000);
    assert_eq!(albums[0].play_count, 1);
    assert_eq!(albums[0].skip_count, 1);
    assert!(albums[0].in_library);

    assert_eq!(albums[1].artist, "Interpol");
    assert_eq!(albums[1].play_count, 1);
    assert_eq!(albums[1].skip_count, 0);
    assert!(!albums[1].in_library);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn spotify_import_pipeline_persists_to_db() {
    let dir = std::env::temp_dir().join(format!("cassette-spotify-import-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("temp dir should be created");
    let file = dir.join("Streaming_History_Audio_2026.json");
    let payload = r#"
[
  {
    "master_metadata_album_artist_name": "Frightened Rabbit",
    "master_metadata_album_album_name": "The Midnight Organ Fight",
    "ms_played": 123000,
    "skipped": false
  },
  {
    "master_metadata_album_artist_name": "Frightened Rabbit",
    "master_metadata_album_album_name": "The Midnight Organ Fight",
    "ms_played": 110000,
    "skipped": true
  }
]
"#;
    std::fs::write(&file, payload).expect("fixture should be written");

    let db_path = dir.join("cassette-test.db");
    let db = cassette_core::db::Db::open(&db_path).expect("db should open");

    let files = spotify_history::collect_spotify_history_files(&dir).expect("history files should be found");
    let entries = spotify_history::parse_spotify_entries(&files).expect("entries should parse");
    let library_set = HashMap::new();
    let albums = spotify_history::summarize_spotify_albums(&entries, &library_set);

    let rows = albums
        .iter()
        .map(|album| cassette_core::models::SpotifyAlbumHistory {
            artist: album.artist.clone(),
            album: album.album.clone(),
            total_ms: album.total_ms,
            play_count: album.play_count,
            skip_count: album.skip_count,
            in_library: album.in_library,
            imported_at: String::new(),
        })
        .collect::<Vec<_>>();
    db.replace_spotify_album_history(&rows)
        .expect("history rows should persist");

    let count = db
        .get_spotify_album_history_count()
        .expect("history count should be readable");
    assert_eq!(count, 1);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn parse_lastfm_artist_info_strips_html_and_reads_tags() {
    let json = serde_json::json!({
        "artist": {
            "bio": { "summary": "Great band text <a href=\"https://example\">Read more</a>" },
            "stats": { "listeners": "12345" },
            "tags": { "tag": [{ "name": "indie" }, { "name": "post-punk" }] }
        }
    });

    let parsed = now_playing::parse_lastfm_artist_info(&json).expect("artist info should parse");
    assert_eq!(parsed.summary.as_deref(), Some("Great band text"));
    assert_eq!(parsed.listeners, Some(12345));
    assert_eq!(parsed.tags, vec!["indie".to_string(), "post-punk".to_string()]);
}

#[test]
fn parse_lastfm_album_info_prefers_largest_image() {
    let json = serde_json::json!({
        "album": {
            "wiki": { "summary": "Album summary <a href=\"https://example\">read</a>" },
            "image": [
                { "#text": "small.jpg", "size": "small" },
                { "#text": "", "size": "large" },
                { "#text": "mega.jpg", "size": "mega" }
            ]
        }
    });

    let parsed = now_playing::parse_lastfm_album_info(&json).expect("album info should parse");
    assert_eq!(parsed.summary.as_deref(), Some("Album summary"));
    assert_eq!(parsed.image_url.as_deref(), Some("mega.jpg"));
}

#[test]
fn runtime_bootstrap_creates_runtime_and_sidecar_dbs() {
    let root = std::env::temp_dir().join(format!("cassette-bootstrap-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).expect("create temp root");
    let db_path = root.join("cassette.db");

    let (_db, _control_db) =
        runtime_bootstrap::open_runtime_and_control_db(&db_path).expect("runtime and sidecar dbs");

    assert!(db_path.exists(), "runtime db should exist");
    assert!(
        runtime_bootstrap::control_db_path_for_runtime(&db_path).exists(),
        "sidecar db should exist"
    );

    let _ = std::fs::remove_dir_all(&root);
}
