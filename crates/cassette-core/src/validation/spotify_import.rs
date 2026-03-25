use crate::validation::error::{Result, ValidationError};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::collections::HashSet;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportSummary {
    pub total_input: usize,
    pub total_imported: usize,
    pub duplicates_skipped: usize,
    pub db_total_spotify_tracks: usize,
    pub timestamp: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpotifyTrack {
    pub artist: String,
    pub name: String,
    pub album: Option<String>,
    pub duration_ms: Option<i64>,
    pub uri: Option<String>,
    pub raw_payload: serde_json::Value,
}

pub async fn import_spotify_export(
    db_pool: &sqlx::SqlitePool,
    spotify_json_path: &Path,
) -> Result<ImportSummary> {
    ensure_desired_tracks_table(db_pool).await?;

    let json_text = tokio::fs::read_to_string(spotify_json_path).await?;
    let tracks = parse_spotify_tracks(&json_text)?;

    let mut imported = 0usize;
    let mut duplicates = 0usize;
    let mut seen_in_file: HashSet<String> = HashSet::new();

    for track in &tracks {
        let source_track_id = track.uri.clone();
        let unique_key = source_track_id
            .clone()
            .unwrap_or_else(|| format!("{}::{}", track.artist, track.name));

        if !seen_in_file.insert(unique_key) {
            duplicates += 1;
            continue;
        }

        if let Some(uri) = source_track_id.as_deref() {
            let existing = sqlx::query_scalar::<_, i64>(
                "SELECT id FROM desired_tracks WHERE source_track_id = ?1 AND source_name = 'spotify' LIMIT 1",
            )
            .bind(uri)
            .fetch_optional(db_pool)
            .await?;

            if existing.is_some() {
                duplicates += 1;
                continue;
            }
        } else {
            let existing = sqlx::query_scalar::<_, i64>(
                r#"
                SELECT id
                FROM desired_tracks
                WHERE source_name = 'spotify'
                  AND lower(artist_name) = lower(?1)
                  AND lower(track_title) = lower(?2)
                LIMIT 1
                "#,
            )
            .bind(&track.artist)
            .bind(&track.name)
            .fetch_optional(db_pool)
            .await?;

            if existing.is_some() {
                duplicates += 1;
                continue;
            }
        }

        sqlx::query(
            r#"
            INSERT INTO desired_tracks
                            (source_name, source_track_id, artist_name, album_title, track_title, duration_ms, raw_payload_json)
                        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
        )
        .bind("spotify")
        .bind(source_track_id)
        .bind(&track.artist)
        .bind(track.album.clone())
        .bind(&track.name)
        .bind(track.duration_ms)
        .bind(track.raw_payload.to_string())
        .execute(db_pool)
        .await?;

        imported += 1;
    }

    let db_total_spotify_tracks = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM desired_tracks WHERE source_name = 'spotify'",
    )
    .fetch_one(db_pool)
    .await? as usize;

    Ok(ImportSummary {
        total_input: tracks.len(),
        total_imported: imported,
        duplicates_skipped: duplicates,
        db_total_spotify_tracks,
        timestamp: Utc::now(),
    })
}

pub async fn verify_spotify_import(
    db_pool: &sqlx::SqlitePool,
    expected_count: usize,
) -> Result<()> {
    let actual = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM desired_tracks WHERE source_name = 'spotify'",
    )
    .fetch_one(db_pool)
    .await? as usize;

    if actual != expected_count {
        return Err(ValidationError::InvalidConfig(format!(
            "Spotify import mismatch: expected {expected_count}, got {actual}",
        )));
    }

    Ok(())
}

fn parse_spotify_tracks(input: &str) -> Result<Vec<SpotifyTrack>> {
    let value: serde_json::Value = serde_json::from_str(input)?;

    if let Some(items) = value.get("items").and_then(|v| v.as_array()) {
        let mut tracks = Vec::new();
        for item in items {
            let track_obj = item.get("track").unwrap_or(item);
            if let Some(track) = spotify_track_from_value(track_obj.clone()) {
                tracks.push(track);
            }
        }
        return Ok(tracks);
    }

    if let Some(array) = value.as_array() {
        let mut tracks = Vec::new();
        for item in array {
            if let Some(track) = spotify_track_from_value(item.clone()) {
                tracks.push(track);
            }
        }
        return Ok(tracks);
    }

    Err(ValidationError::InvalidConfig(
        "Unsupported Spotify export JSON format".to_string(),
    ))
}

fn spotify_track_from_value(value: serde_json::Value) -> Option<SpotifyTrack> {
    let artist = value
        .get("artist")
        .and_then(|v| v.as_str())
        .map(ToString::to_string)
        .or_else(|| {
            value
                .get("artists")
                .and_then(|v| v.as_array())
                .and_then(|artists| artists.first())
                .and_then(|first| first.get("name"))
                .and_then(|name| name.as_str())
                .map(ToString::to_string)
        })?;

    let name = value
        .get("title")
        .and_then(|v| v.as_str())
        .map(ToString::to_string)
        .or_else(|| {
            value
                .get("name")
                .and_then(|v| v.as_str())
                .map(ToString::to_string)
        })?;

    let album = value
        .get("album")
        .and_then(|v| {
            if let Some(s) = v.as_str() {
                Some(s.to_string())
            } else {
                v.get("name")
                    .and_then(|n| n.as_str())
                    .map(ToString::to_string)
            }
        });

    let duration_ms = value
        .get("duration_ms")
        .and_then(|v| v.as_i64())
        .or_else(|| value.get("duration").and_then(|v| v.as_i64()));

    let uri = value
        .get("uri")
        .and_then(|v| v.as_str())
        .map(ToString::to_string)
        .or_else(|| {
            value
                .get("id")
                .and_then(|v| v.as_str())
                .map(|id| format!("spotify:track:{id}"))
        });

    Some(SpotifyTrack {
        artist,
        name,
        album,
        duration_ms,
        uri,
        raw_payload: value,
    })
}

async fn ensure_desired_tracks_table(db_pool: &sqlx::SqlitePool) -> Result<()> {
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
    .execute(db_pool)
    .await?;

    let columns = sqlx::query("PRAGMA table_info(desired_tracks)")
        .fetch_all(db_pool)
        .await?;

    let has_column = |name: &str| {
        columns.iter().any(|row| {
            row.try_get::<String, _>("name")
                .map(|column| column == name)
                .unwrap_or(false)
        })
    };

    if !has_column("source_album_id") {
        let _ = sqlx::query("ALTER TABLE desired_tracks ADD COLUMN source_album_id TEXT")
            .execute(db_pool)
            .await;
    }
    if !has_column("source_artist_id") {
        let _ = sqlx::query("ALTER TABLE desired_tracks ADD COLUMN source_artist_id TEXT")
            .execute(db_pool)
            .await;
    }
    if !has_column("raw_payload_json") {
        let _ = sqlx::query("ALTER TABLE desired_tracks ADD COLUMN raw_payload_json TEXT")
            .execute(db_pool)
            .await;
    }
    if !has_column("track_number") {
        let _ = sqlx::query("ALTER TABLE desired_tracks ADD COLUMN track_number INTEGER")
            .execute(db_pool)
            .await;
    }
    if !has_column("disc_number") {
        let _ = sqlx::query("ALTER TABLE desired_tracks ADD COLUMN disc_number INTEGER")
            .execute(db_pool)
            .await;
    }
    if !has_column("isrc") {
        let _ = sqlx::query("ALTER TABLE desired_tracks ADD COLUMN isrc TEXT")
            .execute(db_pool)
            .await;
    }
    if !has_column("imported_at") {
        let _ = sqlx::query("ALTER TABLE desired_tracks ADD COLUMN imported_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP")
            .execute(db_pool)
            .await;
    }

    Ok(())
}
