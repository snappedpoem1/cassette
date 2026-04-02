use crate::gatekeeper::error::Result;
use crate::gatekeeper::mod_types::{IdentityProof, QualityAssessment};
use std::path::Path;

pub async fn upsert_local_file(
    db_pool: &sqlx::SqlitePool,
    file_path: &Path,
    identity: &IdentityProof,
    quality: &QualityAssessment,
    track_id: Option<i64>,
) -> Result<i64> {
    let path_str = file_path.to_string_lossy().to_string();
    let file_name = file_path
        .file_name()
        .and_then(|x| x.to_str())
        .unwrap_or("unknown")
        .to_string();
    let extension = file_path
        .extension()
        .and_then(|x| x.to_str())
        .unwrap_or("")
        .to_string();
    let file_mtime_ms = std::fs::metadata(file_path)
        .ok()
        .and_then(|metadata| metadata.modified().ok())
        .and_then(|value| value.duration_since(std::time::UNIX_EPOCH).ok())
        .and_then(|value| i64::try_from(value.as_millis()).ok());

    sqlx::query(
        "INSERT INTO local_files (
            track_id, file_path, file_name, extension,
            codec, bitrate, sample_rate, bit_depth, channels,
            duration_ms, file_size, file_mtime_ms, content_hash, acoustid_fingerprint,
            fingerprint_attempted_at, fingerprint_error, fingerprint_source_mtime_ms,
            integrity_status, quality_tier
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, CURRENT_TIMESTAMP, NULL, ?15, ?16, ?17)
        ON CONFLICT(file_path) DO UPDATE SET
            track_id = excluded.track_id,
            codec = excluded.codec,
            bitrate = excluded.bitrate,
            sample_rate = excluded.sample_rate,
            bit_depth = excluded.bit_depth,
            channels = excluded.channels,
            duration_ms = excluded.duration_ms,
            file_size = excluded.file_size,
            file_mtime_ms = excluded.file_mtime_ms,
            content_hash = excluded.content_hash,
            acoustid_fingerprint = excluded.acoustid_fingerprint,
            fingerprint_attempted_at = CURRENT_TIMESTAMP,
            fingerprint_error = NULL,
            fingerprint_source_mtime_ms = excluded.fingerprint_source_mtime_ms,
            integrity_status = excluded.integrity_status,
            quality_tier = excluded.quality_tier,
            updated_at = CURRENT_TIMESTAMP",
    )
    .bind(track_id)
    .bind(&path_str)
    .bind(&file_name)
    .bind(&extension)
    .bind(&identity.codec)
    .bind(identity.bitrate as i64)
    .bind(identity.sample_rate as i64)
    .bind(identity.bit_depth as i64)
    .bind(identity.channels as i64)
    .bind(identity.duration_ms as i64)
    .bind(identity.file_size as i64)
    .bind(file_mtime_ms)
    .bind(&identity.content_hash)
    .bind(&identity.acoustid_fingerprint)
    .bind(file_mtime_ms)
    .bind("verified")
    .bind(format!("{:?}", quality.quality_tier))
    .execute(db_pool)
    .await?;

    let row: (i64,) = sqlx::query_as("SELECT id FROM local_files WHERE file_path = ?1")
        .bind(&path_str)
        .fetch_one(db_pool)
        .await?;
    Ok(row.0)
}
