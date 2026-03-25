use crate::gatekeeper::config::GatekeeperConfig;
use crate::gatekeeper::error::Result;
use crate::gatekeeper::mod_types::{DuplicateConflict, DuplicatePolicyOutcome, QualityTier};
use std::path::PathBuf;

pub async fn detect_duplicates(
    fingerprint: &str,
    content_hash: &str,
    incoming_quality: QualityTier,
    db_pool: &sqlx::SqlitePool,
    config: &GatekeeperConfig,
) -> Result<Option<DuplicateConflict>> {
    if let Some(row) = sqlx::query(
        "SELECT id, file_path, quality_tier
         FROM local_files
         WHERE content_hash = ?1 OR acoustid_fingerprint = ?2
         LIMIT 1",
    )
    .bind(content_hash)
    .bind(fingerprint)
    .fetch_optional(db_pool)
    .await?
    {
        use sqlx::Row;
        let id: i64 = row.try_get("id")?;
        let path: String = row.try_get("file_path")?;
        let existing_quality = match row
            .try_get::<Option<String>, _>("quality_tier")?
            .unwrap_or_else(|| "LossyAcceptable".to_string())
            .to_ascii_lowercase()
            .as_str()
        {
            "lossless" => QualityTier::Lossless,
            "lossyhi" => QualityTier::LossyHi,
            "belowfloor" => QualityTier::BelowFloor,
            _ => QualityTier::LossyAcceptable,
        };

        let incoming_is_better = tier_rank(incoming_quality) > tier_rank(existing_quality);
        let policy_decision = if config.on_duplicate_require_review {
            DuplicatePolicyOutcome::ManualReview
        } else if config.on_duplicate_allow_both {
            DuplicatePolicyOutcome::MarkBothKeepBest
        } else if config.on_duplicate_keep_best {
            if incoming_is_better {
                DuplicatePolicyOutcome::ReplaceExisting
            } else {
                DuplicatePolicyOutcome::KeepExisting
            }
        } else {
            DuplicatePolicyOutcome::KeepExisting
        };

        return Ok(Some(DuplicateConflict {
            existing_local_file_id: id,
            existing_file_path: PathBuf::from(path),
            existing_quality,
            incoming_quality,
            incoming_is_better,
            policy_decision,
        }));
    }

    Ok(None)
}

fn tier_rank(tier: QualityTier) -> i32 {
    match tier {
        QualityTier::BelowFloor => 0,
        QualityTier::LossyAcceptable => 1,
        QualityTier::LossyHi => 2,
        QualityTier::Lossless => 3,
    }
}
