mod adapter_tests;
mod delta_tests;
mod integration_tests;
mod matching_tests;
mod reconciliation_tests;

use crate::library::{LibraryManager, ManagerConfig};
use tempfile::NamedTempFile;

pub async fn test_manager() -> LibraryManager {
    let file = NamedTempFile::new().expect("temp db file");
    let url = format!("sqlite://{}", file.path().to_string_lossy());
    let manager = LibraryManager::connect(&url, ManagerConfig::default())
        .await
        .expect("manager");

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS desired_tracks (
          id INTEGER PRIMARY KEY,
          source_name TEXT NOT NULL,
          source_track_id TEXT,
          source_album_id TEXT,
          source_artist_id TEXT,
          artist_name TEXT NOT NULL,
          album_title TEXT,
          track_title TEXT NOT NULL,
          track_number INTEGER,
          disc_number INTEGER,
          duration_ms INTEGER,
          isrc TEXT,
          raw_payload_json TEXT,
          imported_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        )
        "#,
    )
    .execute(manager.db_pool())
    .await
    .expect("desired tracks table");

    let _ = sqlx::query("ALTER TABLE tracks ADD COLUMN duration_ms INTEGER")
        .execute(manager.db_pool())
        .await;
    let _ = sqlx::query("ALTER TABLE tracks ADD COLUMN isrc TEXT")
        .execute(manager.db_pool())
        .await;
    let _ = sqlx::query("ALTER TABLE tracks ADD COLUMN spotify_id TEXT")
        .execute(manager.db_pool())
        .await;
    let _ = sqlx::query("ALTER TABLE local_files ADD COLUMN file_name TEXT")
        .execute(manager.db_pool())
        .await;
    let _ = sqlx::query("ALTER TABLE local_files ADD COLUMN extension TEXT")
        .execute(manager.db_pool())
        .await;
    let _ = sqlx::query("ALTER TABLE local_files ADD COLUMN codec TEXT")
        .execute(manager.db_pool())
        .await;
    let _ = sqlx::query("ALTER TABLE local_files ADD COLUMN bitrate INTEGER")
        .execute(manager.db_pool())
        .await;
    let _ = sqlx::query("ALTER TABLE local_files ADD COLUMN duration_ms INTEGER")
        .execute(manager.db_pool())
        .await;
    let _ = sqlx::query("ALTER TABLE local_files ADD COLUMN file_size INTEGER")
        .execute(manager.db_pool())
        .await;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS delta_queue (
          id INTEGER PRIMARY KEY,
          desired_track_id INTEGER NOT NULL,
          action_type TEXT NOT NULL,
          priority INTEGER DEFAULT 0,
          reason TEXT NOT NULL,
          target_quality TEXT,
          created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
          processed_at TIMESTAMP
        )
        "#,
    )
    .execute(manager.db_pool())
    .await
    .expect("delta queue table");

    let cols = sqlx::query("PRAGMA table_info(delta_queue)")
        .fetch_all(manager.db_pool())
        .await
        .expect("delta queue columns");
    use sqlx::Row;
    let col_names: std::collections::HashSet<String> = cols
        .iter()
        .filter_map(|c| c.try_get::<String, _>("name").ok())
        .collect();
    if !col_names.contains("source_operation_id") {
        sqlx::query("ALTER TABLE delta_queue ADD COLUMN source_operation_id TEXT")
            .execute(manager.db_pool())
            .await
            .expect("add source op id");
    }
    if !col_names.contains("claimed_at") {
        sqlx::query("ALTER TABLE delta_queue ADD COLUMN claimed_at TIMESTAMP")
            .execute(manager.db_pool())
            .await
            .expect("add claimed_at");
    }
    if !col_names.contains("claim_run_id") {
        sqlx::query("ALTER TABLE delta_queue ADD COLUMN claim_run_id TEXT")
            .execute(manager.db_pool())
            .await
            .expect("add claim_run_id");
    }

    manager
}

pub async fn seed_basic_catalog(manager: &LibraryManager) {
    sqlx::query("INSERT INTO artists(id, canonical_name, normalized_name) VALUES(1, 'Artist', 'artist')")
        .execute(manager.db_pool())
        .await
        .expect("artist");
    sqlx::query("INSERT INTO albums(id, artist_id, title, normalized_title) VALUES(1, 1, 'Album', 'album')")
        .execute(manager.db_pool())
        .await
        .expect("album");
    sqlx::query("INSERT INTO tracks(id, album_id, artist_id, title, normalized_title, duration_ms, isrc, spotify_id) VALUES(1, 1, 1, 'Song', 'song', 200000, 'ISRC123', 'SPOTIFY123')")
        .execute(manager.db_pool())
        .await
        .expect("track");
    sqlx::query("INSERT INTO local_files(id, track_id, file_path, file_name, extension, codec, bitrate, duration_ms, file_size, content_hash, integrity_status, quality_tier) VALUES(1, 1, 'C:/music/song.flac', 'song.flac', 'flac', 'flac', 900, 200000, 1000, 'HASH1', 'readable', 'lossless_preferred')")
        .execute(manager.db_pool())
        .await
        .expect("local file");
}
