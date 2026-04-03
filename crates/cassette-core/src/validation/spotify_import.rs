use crate::validation::error::{Result, ValidationError};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::collections::HashSet;
use std::path::Path;
use crate::librarian::import::spotify::parse_spotify_payload;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportSummary {
    pub total_input: usize,
    pub total_imported: usize,
    pub duplicates_skipped: usize,
    pub db_total_spotify_tracks: usize,
    pub timestamp: chrono::DateTime<Utc>,
}

pub async fn import_spotify_export(
    db_pool: &sqlx::SqlitePool,
    spotify_json_path: &Path,
) -> Result<ImportSummary> {
    ensure_desired_tracks_table(db_pool).await?;

    let json_text = tokio::fs::read_to_string(spotify_json_path).await?;
    let payload = parse_spotify_payload(&json_text)?;
    let source_name = payload
        .source_name
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("spotify");
    let tracks = payload.tracks;

    let mut imported = 0usize;
    let mut duplicates = 0usize;
    let mut seen_in_file: HashSet<String> = HashSet::new();

    for track in &tracks {
        let source_track_id = track.track_id.clone();
        let unique_key = source_track_id
            .clone()
            .unwrap_or_else(|| format!("{}::{}", track.artist_name, track.track_title));

        if !seen_in_file.insert(unique_key) {
            duplicates += 1;
            continue;
        }

        if let Some(uri) = source_track_id.as_deref() {
            let existing = sqlx::query_scalar::<_, i64>(
                "SELECT id FROM desired_tracks WHERE source_track_id = ?1 AND source_name = ?2 LIMIT 1",
            )
            .bind(uri)
            .bind(source_name)
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
                WHERE source_name = ?1
                  AND lower(artist_name) = lower(?2)
                  AND lower(track_title) = lower(?3)
                LIMIT 1
                "#,
            )
            .bind(source_name)
            .bind(&track.artist_name)
            .bind(&track.track_title)
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
                            (source_name, source_track_id, source_album_id, source_artist_id, artist_name, album_title, track_title, track_number, disc_number, duration_ms, isrc, raw_payload_json)
                        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
            "#,
        )
        .bind(source_name)
        .bind(source_track_id)
        .bind(track.album_id.as_deref())
        .bind(track.artist_id.as_deref())
        .bind(&track.artist_name)
        .bind(track.album_title.clone())
        .bind(&track.track_title)
        .bind(track.track_number)
        .bind(track.disc_number)
        .bind(track.duration_ms)
        .bind(track.isrc.as_deref())
        .bind(
            track
                .raw_payload
                .as_ref()
                .map(serde_json::Value::to_string)
                .or_else(|| serde_json::to_string(track).ok()),
        )
        .execute(db_pool)
        .await?;

        imported += 1;
    }

    let db_total_spotify_tracks = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM desired_tracks WHERE source_name = ?1",
    )
    .bind(source_name)
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
