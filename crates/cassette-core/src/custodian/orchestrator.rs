use crate::custodian::collision::policies::CollisionOutcome;
use crate::custodian::collision::resolution::resolve_collision;
use crate::custodian::config::CustodianConfig;
use crate::custodian::custody_log::manifest::{
    CustodianManifest, ManifestAction, ManifestCollision, ManifestSummary,
};
use crate::custodian::custody_log::persistence::persist_manifest;
use crate::custodian::error::Result;
use crate::custodian::quality::codec_info::quality_score;
use crate::custodian::quality::duplicates::{assert_supported_hash, classify_duplicate};
use crate::custodian::quarantine::directory_structure::build_quarantine_path;
use crate::custodian::quarantine::reason_codes::{quarantine_reason_for_status, QuarantineReason};
use crate::custodian::sort::canonical_path::{
    build_canonical_path, canonical_metadata_from_report,
};
use crate::custodian::staging::copy_verify_delete::staged_copy_verify;
use crate::custodian::staging::verification::compute_hash;
use crate::custodian::sync::delta_queue_sync::mark_quarantine_delta;
use crate::custodian::sync::local_files_update::{
    ensure_custodian_columns, load_candidates, update_local_file_after_action,
};
use crate::custodian::validation::{deep_validate_audio, ValidationStatus};
use chrono::Utc;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use uuid::Uuid;

#[derive(Debug, Clone, Default)]
pub struct CustodianSummary {
    pub total_files_processed: usize,
    pub files_valid: usize,
    pub files_sorted: usize,
    pub files_quarantined: usize,
    pub files_skipped: usize,
    pub duplicates_detected: usize,
    pub collisions_resolved: usize,
    pub total_errors: usize,
}

#[derive(Debug, Clone)]
pub struct CustodianOutcome {
    pub operation_id: String,
    pub mode: String,
    pub summary: CustodianSummary,
    pub manifest: serde_json::Value,
    pub errors: Vec<(String, String)>,
}

pub async fn run_custodian_cleanup(
    db_pool: &sqlx::SqlitePool,
    config: &CustodianConfig,
    dry_run: bool,
) -> Result<CustodianOutcome> {
    ensure_custodian_columns(db_pool).await?;
    let candidates = load_candidates(db_pool, &config.source_roots).await?;

    let operation_id = Uuid::new_v4().to_string();
    let mode = if dry_run { "dry_run" } else { "execute" }.to_string();
    let mut summary = CustodianSummary::default();
    let errors = Vec::<(String, String)>::new();
    let mut actions = Vec::<ManifestAction>::new();
    let mut collisions = Vec::<ManifestCollision>::new();

    let mut seen_hashes = HashMap::<String, PathBuf>::new();
    let run_started = Instant::now();
    let mut validate_time = Duration::ZERO;
    let mut collision_hash_time = Duration::ZERO;
    let mut transfer_time = Duration::ZERO;
    let mut db_time = Duration::ZERO;
    let mut progress_checkpoint = Instant::now();
    const PROGRESS_EVERY_FILES: usize = 250;

    let log_progress = |summary: &CustodianSummary,
                        validate_time: Duration,
                        collision_hash_time: Duration,
                        transfer_time: Duration,
                        db_time: Duration,
                        run_started: Instant,
                        progress_checkpoint: &mut Instant| {
        let now = Instant::now();
        let elapsed = run_started.elapsed().as_secs_f64();
        let checkpoint = now.duration_since(*progress_checkpoint).as_secs_f64();
        *progress_checkpoint = now;
        let processed = summary.total_files_processed.max(1) as f64;
        let rate = summary.total_files_processed as f64 / elapsed.max(0.001);
        println!(
            "[custodian] progress processed={} valid={} sorted={} quarantined={} skipped={} dupes={} collisions={} elapsed={:.1}s rate={:.2} files/s checkpoint={:.1}s stage_totals(validate={:.1}s collision_hash={:.1}s transfer={:.1}s db={:.1}s)",
            summary.total_files_processed,
            summary.files_valid,
            summary.files_sorted,
            summary.files_quarantined,
            summary.files_skipped,
            summary.duplicates_detected,
            summary.collisions_resolved,
            elapsed,
            rate,
            checkpoint,
            validate_time.as_secs_f64(),
            collision_hash_time.as_secs_f64(),
            transfer_time.as_secs_f64(),
            db_time.as_secs_f64(),
        );
        println!(
            "[custodian] stage_avg_ms validate={:.1} collision_hash={:.1} transfer={:.1} db={:.1}",
            (validate_time.as_secs_f64() * 1000.0) / processed,
            (collision_hash_time.as_secs_f64() * 1000.0) / processed,
            (transfer_time.as_secs_f64() * 1000.0) / processed,
            (db_time.as_secs_f64() * 1000.0) / processed,
        );
    };

    for candidate in candidates {
        summary.total_files_processed += 1;
        let source = candidate.file_path.clone();

        let validate_started = Instant::now();
        let report = deep_validate_audio(
            &source,
            &config.allowed_formats,
            config.suspicious_size_tolerance,
            true, // Required for correct duplicate/collision decisions.
        );
        validate_time += validate_started.elapsed();

        if matches!(report.status, ValidationStatus::Valid) {
            summary.files_valid += 1;

            let canonical = canonical_metadata_from_report(&source, &report);
            let destination = build_canonical_path(&config.sorted_target, &canonical);
            let hash = report.content_hash.clone();
            assert_supported_hash(&hash)?;

            if let Some(hash) = hash.clone() {
                if let Some(existing_path) = seen_hashes.get(&hash) {
                    summary.duplicates_detected += 1;
                    let incoming_score =
                        quality_score(report.codec.as_deref(), report.bitrate, report.bit_depth);
                    let existing_score = incoming_score;
                    let decision =
                        classify_duplicate(config.duplicate_policy, incoming_score, existing_score);

                    actions.push(ManifestAction {
                        source_path: source.to_string_lossy().to_string(),
                        action: "duplicate".to_string(),
                        destination_path: Some(existing_path.to_string_lossy().to_string()),
                        validation_status: format!("{:?}", report.status),
                        codec: report.codec.clone(),
                        bitrate: report.bitrate,
                        checksum: Some(hash),
                        db_record_updated: false,
                        timestamp: Utc::now().to_rfc3339(),
                        success: true,
                        reason: Some(format!("duplicate policy decision: {:?}", decision)),
                    });
                    summary.files_skipped += 1;
                    continue;
                }
            }

            let mut db_updated = false;
            if destination.exists() {
                summary.collisions_resolved += 1;
                let incoming_score =
                    quality_score(report.codec.as_deref(), report.bitrate, report.bit_depth);

                let existing_hash = if destination.exists() {
                    let hash_started = Instant::now();
                    let value = compute_hash(&destination).await.ok();
                    collision_hash_time += hash_started.elapsed();
                    value
                } else {
                    None
                };
                let existing_quality = if existing_hash.is_some() {
                    incoming_score
                } else {
                    0
                };
                let outcome = resolve_collision(
                    config.collision_policy,
                    incoming_score,
                    existing_quality,
                    existing_hash == report.content_hash,
                );

                collisions.push(ManifestCollision {
                    destination_path: destination.to_string_lossy().to_string(),
                    existing_file_quality: existing_quality,
                    incoming_file_quality: incoming_score,
                    policy: format!("{:?}", config.collision_policy),
                    outcome: format!("{:?}", outcome),
                    db_decision_recorded: !dry_run,
                });

                match outcome {
                    CollisionOutcome::NoActionFileAlreadySorted
                    | CollisionOutcome::KeepExistingMarkIncomingDuplicate => {
                        summary.files_skipped += 1;
                        actions.push(ManifestAction {
                            source_path: source.to_string_lossy().to_string(),
                            action: "skip_collision".to_string(),
                            destination_path: Some(destination.to_string_lossy().to_string()),
                            validation_status: format!("{:?}", report.status),
                            codec: report.codec.clone(),
                            bitrate: report.bitrate,
                            checksum: report.content_hash.clone(),
                            db_record_updated: false,
                            timestamp: Utc::now().to_rfc3339(),
                            success: true,
                            reason: Some(format!("collision outcome: {:?}", outcome)),
                        });
                        continue;
                    }
                    CollisionOutcome::ManualReviewRequired
                    | CollisionOutcome::QuarantineIncomingAsDuplicate
                    | CollisionOutcome::RenameIncomingWithSuffixForReview => {
                        let reason = if matches!(outcome, CollisionOutcome::ManualReviewRequired) {
                            QuarantineReason::CollisionReview
                        } else {
                            QuarantineReason::DuplicateReview
                        };
                        let quarantine =
                            build_quarantine_path(&config.quarantine_root, reason, &source);

                        if !dry_run {
                            if let Some(parent) = quarantine.parent() {
                                tokio::fs::create_dir_all(parent).await?;
                            }
                            let transfer_started = Instant::now();
                            staged_copy_verify(
                                &source,
                                &config.staging_root,
                                &quarantine,
                                config.verify_copy,
                                config.delete_source_after_verify,
                                config.same_volume_move,
                            )
                            .await?;
                            transfer_time += transfer_started.elapsed();

                            let db_started = Instant::now();
                            let mut tx = db_pool.begin().await?;
                            update_local_file_after_action(
                                &mut tx,
                                candidate.id,
                                &quarantine.to_string_lossy(),
                                reason.as_dir(),
                                &format!("collision outcome: {:?}", outcome),
                                None,
                            )
                            .await?;
                            mark_quarantine_delta(
                                &mut tx,
                                candidate.id,
                                candidate.track_id,
                                reason.as_dir(),
                            )
                            .await?;
                            tx.commit().await?;
                            db_time += db_started.elapsed();
                            db_updated = true;
                        }

                        summary.files_quarantined += 1;
                        actions.push(ManifestAction {
                            source_path: source.to_string_lossy().to_string(),
                            action: "quarantine_collision".to_string(),
                            destination_path: Some(quarantine.to_string_lossy().to_string()),
                            validation_status: format!("{:?}", report.status),
                            codec: report.codec.clone(),
                            bitrate: report.bitrate,
                            checksum: report.content_hash.clone(),
                            db_record_updated: db_updated,
                            timestamp: Utc::now().to_rfc3339(),
                            success: true,
                            reason: Some(format!("collision outcome: {:?}", outcome)),
                        });
                        continue;
                    }
                    CollisionOutcome::ReplaceExistingIfIncomingBetter => {}
                }
            }

            if !dry_run {
                if let Some(parent) = destination.parent() {
                    tokio::fs::create_dir_all(parent).await?;
                }
                let transfer_started = Instant::now();
                staged_copy_verify(
                    &source,
                    &config.staging_root,
                    &destination,
                    config.verify_copy,
                    config.delete_source_after_verify,
                    config.same_volume_move,
                )
                .await?;
                transfer_time += transfer_started.elapsed();

                let db_started = Instant::now();
                let mut tx = db_pool.begin().await?;
                update_local_file_after_action(
                    &mut tx,
                    candidate.id,
                    &destination.to_string_lossy(),
                    "valid",
                    "sorted into canonical path",
                    None,
                )
                .await?;
                tx.commit().await?;
                db_time += db_started.elapsed();
                db_updated = true;
            }

            if let Some(hash) = report.content_hash.clone() {
                seen_hashes.insert(hash, destination.clone());
            }

            summary.files_sorted += 1;
            actions.push(ManifestAction {
                source_path: source.to_string_lossy().to_string(),
                action: "sort".to_string(),
                destination_path: Some(destination.to_string_lossy().to_string()),
                validation_status: format!("{:?}", report.status),
                codec: report.codec.clone(),
                bitrate: report.bitrate,
                checksum: report.content_hash.clone(),
                db_record_updated: db_updated,
                timestamp: Utc::now().to_rfc3339(),
                success: true,
                reason: None,
            });
        } else {
            let reason = quarantine_reason_for_status(report.status.clone())
                .unwrap_or(QuarantineReason::DecodeFailed);
            let quarantine = build_quarantine_path(&config.quarantine_root, reason, &source);

            let mut db_updated = false;
            if !dry_run && !matches!(report.status, ValidationStatus::MissingOnDisk) {
                if let Some(parent) = quarantine.parent() {
                    tokio::fs::create_dir_all(parent).await?;
                }
                let transfer_started = Instant::now();
                staged_copy_verify(
                    &source,
                    &config.staging_root,
                    &quarantine,
                    config.verify_copy,
                    config.delete_source_after_verify,
                    config.same_volume_move,
                )
                .await?;
                transfer_time += transfer_started.elapsed();

                let db_started = Instant::now();
                let mut tx = db_pool.begin().await?;
                update_local_file_after_action(
                    &mut tx,
                    candidate.id,
                    &quarantine.to_string_lossy(),
                    reason.as_dir(),
                    &report.reasons.join("; "),
                    None,
                )
                .await?;
                mark_quarantine_delta(&mut tx, candidate.id, candidate.track_id, reason.as_dir())
                    .await?;
                tx.commit().await?;
                db_time += db_started.elapsed();
                db_updated = true;
            }

            summary.files_quarantined += 1;
            actions.push(ManifestAction {
                source_path: source.to_string_lossy().to_string(),
                action: "quarantine".to_string(),
                destination_path: Some(quarantine.to_string_lossy().to_string()),
                validation_status: format!("{:?}", report.status),
                codec: report.codec.clone(),
                bitrate: report.bitrate,
                checksum: report.content_hash.clone(),
                db_record_updated: db_updated,
                timestamp: Utc::now().to_rfc3339(),
                success: true,
                reason: Some(report.reasons.join("; ")),
            });
        }

        if summary.total_files_processed % PROGRESS_EVERY_FILES == 0 {
            log_progress(
                &summary,
                validate_time,
                collision_hash_time,
                transfer_time,
                db_time,
                run_started,
                &mut progress_checkpoint,
            );
        }
    }

    log_progress(
        &summary,
        validate_time,
        collision_hash_time,
        transfer_time,
        db_time,
        run_started,
        &mut progress_checkpoint,
    );

    summary.total_errors = errors.len();
    let manifest = CustodianManifest {
        operation_id: operation_id.clone(),
        run_timestamp: Utc::now().to_rfc3339(),
        mode: mode.clone(),
        summary: ManifestSummary {
            total_files_processed: summary.total_files_processed,
            files_valid: summary.files_valid,
            files_sorted: summary.files_sorted,
            files_quarantined: summary.files_quarantined,
            files_skipped: summary.files_skipped,
            duplicates_detected: summary.duplicates_detected,
            collisions_resolved: summary.collisions_resolved,
            errors: summary.total_errors,
        },
        actions,
        collisions,
    };

    let manifest_path = persist_manifest(&config.manifest_dir, &manifest).await?;
    let manifest_json = serde_json::to_value(&manifest)?;
    tracing::info!(manifest_path = %manifest_path.display(), "custodian manifest persisted");

    Ok(CustodianOutcome {
        operation_id,
        mode,
        summary,
        manifest: manifest_json,
        errors,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn setup_pool() -> sqlx::SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("pool");

        for sql in crate::librarian::db::migrations::MIGRATIONS {
            sqlx::query(sql).execute(&pool).await.expect("migrate");
        }

        pool
    }

    #[tokio::test]
    async fn dry_run_does_not_mutate_files() {
        let pool = setup_pool().await;
        let root = tempfile::tempdir().expect("root");
        let file = root.path().join("song.mp3");
        std::fs::write(&file, b"not-audio").expect("write");

        sqlx::query(
            "INSERT INTO local_files (file_path, file_name, extension, integrity_status)
             VALUES (?1, ?2, ?3, ?4)",
        )
        .bind(file.to_string_lossy().to_string())
        .bind("song.mp3")
        .bind("mp3")
        .bind("unknown")
        .execute(&pool)
        .await
        .expect("insert local file");

        let cfg = CustodianConfig {
            source_roots: vec![root.path().to_path_buf()],
            sorted_target: root.path().join("sorted"),
            staging_root: root.path().join("staging"),
            quarantine_root: root.path().join("quarantine"),
            dry_run: true,
            verify_copy: true,
            cross_volume_copy: true,
            same_volume_move: false,
            delete_source_after_verify: false,
            duplicate_policy: crate::custodian::collision::policies::DuplicatePolicy::ManualReview,
            collision_policy:
                crate::custodian::collision::policies::CollisionPolicy::RenameIncoming,
            suspicious_size_tolerance: 1.5,
            allowed_formats: vec!["mp3".to_string(), "flac".to_string()],
            logging_level: "info".to_string(),
            manifest_dir: root.path().join("manifests"),
        };

        let outcome = run_custodian_cleanup(&pool, &cfg, true)
            .await
            .expect("custodian");
        assert_eq!(outcome.mode, "dry_run");
        assert!(file.exists());
        assert!(!cfg.staging_root.exists() || std::fs::read_dir(&cfg.staging_root).is_ok());
    }
}
