use crate::librarian::models::DesiredTrack;
use crate::orchestrator::config::ReconciliationConfig;
use crate::orchestrator::error::Result;
use crate::orchestrator::reconciliation::exact_match::row_to_match;
use crate::orchestrator::reconciliation::normalization::normalize_name;
use crate::orchestrator::reconciliation::scoring::compute_metadata_confidence;
use crate::orchestrator::types::{LocalFileMatch, MatchMethod};

pub async fn match_by_strong_metadata(
    pool: &sqlx::SqlitePool,
    desired: &DesiredTrack,
    config: &ReconciliationConfig,
) -> Result<Option<(LocalFileMatch, f32)>> {
    let artist = normalize_name(&desired.artist_name);
    let title = normalize_name(&desired.track_title);
    let album = desired.album_title.as_ref().map(|v| normalize_name(v));

    let rows = sqlx::query(
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
        FROM local_files lf
        JOIN tracks t ON t.id = lf.track_id
        JOIN artists a ON a.id = t.artist_id
        LEFT JOIN albums al ON al.id = t.album_id
        WHERE lf.integrity_status IN ('readable', 'valid', 'partial_metadata')
        "#,
    )
    .fetch_all(pool)
    .await?;

    let mut best: Option<(LocalFileMatch, f32)> = None;
    for row in rows {
        let mut candidate = row_to_match(&row, MatchMethod::StrongMetadata)?;
        if normalize_name(&candidate.artist_name) != artist {
            continue;
        }

        if normalize_name(&candidate.title) != title {
            continue;
        }

        if let Some(expected_album) = album.as_ref() {
            if !expected_album.is_empty()
                && normalize_name(&candidate.album_title) != *expected_album
            {
                continue;
            }
        }

        let confidence =
            compute_metadata_confidence(desired, &candidate, config.duration_tolerance_ms);
        if confidence >= config.strong_match_floor {
            candidate.matched_via = MatchMethod::StrongMetadata;
            if best.as_ref().map(|(_, c)| *c).unwrap_or(0.0) < confidence {
                best = Some((candidate, confidence));
            }
        }
    }

    Ok(best)
}
