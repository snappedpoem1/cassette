use crate::librarian::models::DesiredTrack;
use crate::orchestrator::config::ReconciliationConfig;
use crate::orchestrator::error::Result;
use crate::orchestrator::reconciliation::exact_match::row_to_match;
use crate::orchestrator::reconciliation::normalization::normalize_name;
use crate::orchestrator::types::{LocalFileMatch, MatchMethod};

pub async fn match_by_fuzzy(
    pool: &sqlx::SqlitePool,
    desired: &DesiredTrack,
    config: &ReconciliationConfig,
) -> Result<Option<(LocalFileMatch, f32)>> {
    let desired_artist = normalize_name(&desired.artist_name);
    let desired_title = normalize_name(&desired.track_title);

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
        LIMIT 2000
        "#,
    )
    .fetch_all(pool)
    .await?;

    let mut best: Option<(LocalFileMatch, f32)> = None;
    for row in rows {
        let mut candidate = row_to_match(&row, MatchMethod::WeakFuzzy)?;
        let artist_similarity = strsim::normalized_levenshtein(
            &desired_artist,
            &normalize_name(&candidate.artist_name),
        ) as f32;
        let title_similarity =
            strsim::normalized_levenshtein(&desired_title, &normalize_name(&candidate.title))
                as f32;
        let confidence = (artist_similarity * 0.4) + (title_similarity * 0.6);

        if confidence >= config.fuzzy_match_floor {
            candidate.matched_via = MatchMethod::WeakFuzzy;
            if best.as_ref().map(|(_, c)| *c).unwrap_or(0.0) < confidence {
                best = Some((candidate, confidence));
            }
        }
    }

    Ok(best)
}
