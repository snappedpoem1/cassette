use crate::gatekeeper::error::{GatekeeperError, Result};
use crate::gatekeeper::mod_types::{IdentityProof, PayloadProbe, QualityAssessment};
use crate::librarian::models::DesiredTrack;
use chrono::Utc;
use sqlx::Row;

pub async fn prove_identity(
    fingerprint: &str,
    probe: &PayloadProbe,
    quality: &QualityAssessment,
    content_hash: &str,
    expected_metadata: Option<&DesiredTrack>,
    db_pool: &sqlx::SqlitePool,
) -> Result<(IdentityProof, Option<i64>)> {
    let mut matched_local_file_id = None;
    let mut matched_desired_track = false;
    let mut matched_isrc = None;
    let matched_mbid = None;
    let mut confidence = 0.45_f32;

    if let Some(expected) = expected_metadata {
        if let Some(expected_isrc) = &expected.isrc {
            matched_isrc = Some(false);
            let row = sqlx::query("SELECT id, isrc FROM tracks WHERE isrc = ?1 LIMIT 1")
                .bind(expected_isrc)
                .fetch_optional(db_pool)
                .await?;
            if row.is_some() {
                matched_desired_track = true;
                matched_isrc = Some(true);
                confidence = 0.95;
            }
        }

        if !matched_desired_track {
            let artist = expected.artist_name.to_ascii_lowercase();
            let title = expected.track_title.to_ascii_lowercase();
            let row = sqlx::query(
                "SELECT t.id
                 FROM tracks t
                 JOIN artists a ON a.id = t.artist_id
                 WHERE a.normalized_name = ?1 AND t.normalized_title = ?2
                 LIMIT 1",
            )
            .bind(artist)
            .bind(title)
            .fetch_optional(db_pool)
            .await?;

            if row.is_some() {
                matched_desired_track = true;
                confidence = 0.82;
            }
        }
    }

    let duplicate = sqlx::query(
        "SELECT id FROM local_files
         WHERE content_hash = ?1 OR acoustid_fingerprint = ?2
         LIMIT 1",
    )
    .bind(content_hash)
    .bind(fingerprint)
    .fetch_optional(db_pool)
    .await?;

    if let Some(row) = duplicate {
        matched_local_file_id = Some(row.try_get::<i64, _>("id").map_err(|e| {
            GatekeeperError::IdentityProofFailed(format!("duplicate row decode failed: {e}"))
        })?);
        confidence = confidence.max(0.99);
    }

    let quality_factor = if quality.is_lossless { 0.03 } else { 0.0 };
    let acoustid_confidence = (confidence + quality_factor).clamp(0.0, 1.0);

    Ok((
        IdentityProof {
            codec: probe.codec.clone(),
            bitrate: probe.bitrate,
            sample_rate: probe.sample_rate,
            bit_depth: probe.bit_depth,
            channels: probe.channels,
            duration_ms: probe.duration_ms,
            file_size: probe.file_size,
            content_hash: content_hash.to_string(),
            acoustid_fingerprint: fingerprint.to_string(),
            acoustid_confidence,
            matched_desired_track,
            matched_isrc,
            matched_mbid,
            validation_timestamp: Utc::now(),
        },
        matched_local_file_id,
    ))
}
