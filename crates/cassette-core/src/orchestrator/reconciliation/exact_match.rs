use crate::librarian::models::DesiredTrack;
use crate::orchestrator::error::Result;
use crate::orchestrator::types::{LocalFileMatch, MatchMethod};
use sqlx::Row;

pub async fn match_by_exact_ids(
    pool: &sqlx::SqlitePool,
    desired: &DesiredTrack,
) -> Result<Option<LocalFileMatch>> {
    if let Some(isrc) = desired.isrc.as_ref() {
        let query = sqlx::query(
            r#"
            SELECT lf.id AS file_id, lf.track_id, lf.file_path, lf.file_name,
                   COALESCE(lf.codec, lf.extension, '') AS codec,
                   COALESCE(lf.bitrate, 0) AS bitrate,
                   COALESCE(lf.quality_tier, 'unknown') AS quality_tier,
                   a.canonical_name AS artist_name,
                   COALESCE(al.title, '') AS album_title,
                   t.title AS title,
                   COALESCE(lf.duration_ms, COALESCE(t.duration_ms, 0)) AS duration_ms,
                   lf.content_hash AS content_hash,
                   CAST(NULL AS TEXT) AS acoustid_fingerprint
            FROM tracks t
            JOIN artists a ON a.id = t.artist_id
            LEFT JOIN albums al ON al.id = t.album_id
            JOIN local_files lf ON lf.track_id = t.id
            WHERE t.isrc = ?1
            LIMIT 1
            "#,
        )
        .bind(isrc)
        .fetch_optional(pool)
        .await?;

        if let Some(row) = query {
            return Ok(Some(row_to_match(&row, MatchMethod::IsrcExact)?));
        }
    }

    if let Some(spotify_id) = desired.source_track_id.as_ref() {
        let query = sqlx::query(
            r#"
            SELECT lf.id AS file_id, lf.track_id, lf.file_path, lf.file_name,
                   COALESCE(lf.codec, lf.extension, '') AS codec,
                   COALESCE(lf.bitrate, 0) AS bitrate,
                   COALESCE(lf.quality_tier, 'unknown') AS quality_tier,
                   a.canonical_name AS artist_name,
                   COALESCE(al.title, '') AS album_title,
                   t.title AS title,
                   COALESCE(lf.duration_ms, COALESCE(t.duration_ms, 0)) AS duration_ms,
                   lf.content_hash AS content_hash,
                   CAST(NULL AS TEXT) AS acoustid_fingerprint
            FROM tracks t
            JOIN artists a ON a.id = t.artist_id
            LEFT JOIN albums al ON al.id = t.album_id
            JOIN local_files lf ON lf.track_id = t.id
            WHERE t.spotify_id = ?1
            LIMIT 1
            "#,
        )
        .bind(spotify_id)
        .fetch_optional(pool)
        .await?;

        if let Some(row) = query {
            return Ok(Some(row_to_match(&row, MatchMethod::SpotifyUriExact)?));
        }
    }

    Ok(None)
}

pub(crate) fn row_to_match(
    row: &sqlx::sqlite::SqliteRow,
    matched_via: MatchMethod,
) -> Result<LocalFileMatch> {
    Ok(LocalFileMatch {
        file_id: row.try_get::<i64, _>("file_id")? as u64,
        track_id: row.try_get::<Option<i64>, _>("track_id")?.map(|v| v as u64),
        file_path: std::path::PathBuf::from(row.try_get::<String, _>("file_path")?),
        file_name: row.try_get::<String, _>("file_name")?,
        codec: row.try_get::<String, _>("codec")?,
        bitrate: row.try_get::<i64, _>("bitrate")?.max(0) as u32,
        quality_tier: row.try_get::<String, _>("quality_tier")?,
        artist_name: row.try_get::<String, _>("artist_name")?,
        album_title: row.try_get::<String, _>("album_title")?,
        title: row.try_get::<String, _>("title")?,
        duration_ms: row.try_get::<i64, _>("duration_ms")?.max(0) as u64,
        content_hash: row.try_get::<Option<String>, _>("content_hash")?,
        acoustid_fingerprint: row.try_get::<Option<String>, _>("acoustid_fingerprint")?,
        matched_via,
    })
}
