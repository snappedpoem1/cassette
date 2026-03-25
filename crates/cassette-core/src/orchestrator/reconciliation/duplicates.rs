use crate::librarian::models::DesiredTrack;
use crate::orchestrator::config::ReconciliationConfig;
use crate::orchestrator::error::Result;
use crate::orchestrator::reconciliation::exact_match::row_to_match;
use crate::orchestrator::types::{LocalFileMatch, MatchMethod};
use sqlx::Row;

pub async fn find_duplicates(
    pool: &sqlx::SqlitePool,
    desired: &DesiredTrack,
    config: &ReconciliationConfig,
) -> Result<Vec<LocalFileMatch>> {
    let mut where_clauses = Vec::<String>::new();

    if config.detect_by_content_hash {
        if let Some(hash) = extract_payload_value(desired, "content_hash") {
            where_clauses.push(format!("lf.content_hash = '{}'", hash.replace('\'', "''")));
        }
    }

    if config.detect_by_fingerprint {
        if has_column(pool, "local_files", "acoustid_fingerprint").await? {
            if let Some(fingerprint) = extract_payload_value(desired, "acoustid_fingerprint") {
                where_clauses.push(format!("lf.acoustid_fingerprint = '{}'", fingerprint.replace('\'', "''")));
            }
        }
    }

    if where_clauses.is_empty() {
        return Ok(Vec::new());
    }

    let query = format!(
        "SELECT lf.id AS file_id, lf.track_id, lf.file_path, lf.file_name,\
             COALESCE(lf.codec, lf.extension, '') AS codec,\
             COALESCE(lf.bitrate, 0) AS bitrate,\
             COALESCE(lf.quality_tier, 'unknown') AS quality_tier,\
             a.canonical_name AS artist_name,\
             COALESCE(al.title, '') AS album_title,\
             t.title AS title,\
             COALESCE(lf.duration_ms, COALESCE(t.duration_ms, 0)) AS duration_ms,\
             lf.content_hash AS content_hash,\
             CASE WHEN instr((SELECT group_concat(name) FROM pragma_table_info('local_files')), 'acoustid_fingerprint') > 0 THEN lf.acoustid_fingerprint ELSE NULL END AS acoustid_fingerprint\
         FROM local_files lf\
         LEFT JOIN tracks t ON t.id = lf.track_id\
         LEFT JOIN artists a ON a.id = t.artist_id\
         LEFT JOIN albums al ON al.id = t.album_id\
         WHERE {}\
         ORDER BY COALESCE(lf.quality_tier, 'unknown') DESC",
        where_clauses.join(" OR ")
    );

    let rows = sqlx::query(&query).fetch_all(pool).await?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        out.push(row_to_match(&row, MatchMethod::FingerprintExact)?);
    }
    Ok(out)
}

pub fn should_upgrade_quality(
    current_tier: &str,
    config: &ReconciliationConfig,
) -> bool {
    if !config.prefer_lossless {
        return false;
    }
    quality_tier_rank("lossless") > quality_tier_rank(current_tier)
}

fn quality_tier_rank(tier: &str) -> u32 {
    match tier {
        "lossless" | "lossless_preferred" => 4,
        "lossy_hi" => 3,
        "lossy_acceptable" => 2,
        "below_floor" => 1,
        _ => 0,
    }
}

fn extract_payload_value(desired: &DesiredTrack, key: &str) -> Option<String> {
    let payload = desired.raw_payload_json.as_ref()?;
    let value: serde_json::Value = serde_json::from_str(payload).ok()?;
    value.get(key).and_then(|v| v.as_str()).map(ToString::to_string)
}

async fn has_column(pool: &sqlx::SqlitePool, table: &str, column: &str) -> Result<bool> {
    let rows = sqlx::query(&format!("PRAGMA table_info({table})"))
        .fetch_all(pool)
        .await?;
    Ok(rows.iter().any(|r| r.try_get::<String, _>("name").map(|n| n == column).unwrap_or(false)))
}
