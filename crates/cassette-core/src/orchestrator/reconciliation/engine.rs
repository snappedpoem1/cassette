use crate::librarian::models::DesiredTrack;
use crate::library::LibraryManager;
use crate::orchestrator::config::ReconciliationConfig;
use crate::orchestrator::error::{OrchestratorError, Result};
use crate::orchestrator::reconciliation::duplicates::{find_duplicates, should_upgrade_quality};
use crate::orchestrator::reconciliation::exact_match::match_by_exact_ids;
use crate::orchestrator::reconciliation::fuzzy_match::match_by_fuzzy;
use crate::orchestrator::reconciliation::strong_match::match_by_strong_metadata;
use crate::orchestrator::types::{ReconciliationResult, ReconciliationStatus, TrackReconciliation};

pub async fn reconcile_desired_against_local(
    manager: &LibraryManager,
    operation_id: &str,
    desired_tracks: &[DesiredTrack],
    config: &ReconciliationConfig,
) -> Result<ReconciliationResult> {
    let mut result = ReconciliationResult {
        total_desired: desired_tracks.len(),
        ..Default::default()
    };

    for desired in desired_tracks {
        if let Some(exact) = match_by_exact_ids(manager.db_pool(), desired).await? {
            result.matched_count += 1;
            result.reconciliations.push(TrackReconciliation {
                desired_track_id: desired.id as u64,
                desired_track: desired.clone(),
                matched_local_file: Some(exact),
                status: ReconciliationStatus::ExactMatch,
                confidence: config.exact_match_confidence,
                reason: "Matched by exact identifier".to_string(),
            });
            continue;
        }

        if let Some((strong, confidence)) =
            match_by_strong_metadata(manager.db_pool(), desired, config).await?
        {
            result.matched_count += 1;
            result.reconciliations.push(TrackReconciliation {
                desired_track_id: desired.id as u64,
                desired_track: desired.clone(),
                matched_local_file: Some(strong),
                status: ReconciliationStatus::HighConfidenceMatch,
                confidence,
                reason: format!("Strong metadata match ({:.0}%)", confidence * 100.0),
            });
            continue;
        }

        if let Some((fuzzy, confidence)) = match_by_fuzzy(manager.db_pool(), desired, config).await? {
            // Auto-accept fuzzy matches above 80% as high-confidence;
            // only flag truly ambiguous matches for manual review.
            let (status, label) = if confidence >= 0.80 {
                result.matched_count += 1;
                (ReconciliationStatus::HighConfidenceMatch, "Auto-accepted fuzzy match")
            } else {
                result.manual_review_count += 1;
                (ReconciliationStatus::ManualReviewNeeded, "Fuzzy match requires manual review")
            };
            result.reconciliations.push(TrackReconciliation {
                desired_track_id: desired.id as u64,
                desired_track: desired.clone(),
                matched_local_file: Some(fuzzy),
                status,
                confidence,
                reason: format!("{} ({:.0}%)", label, confidence * 100.0),
            });
            continue;
        }

        result.missing_count += 1;
        result.reconciliations.push(TrackReconciliation {
            desired_track_id: desired.id as u64,
            desired_track: desired.clone(),
            matched_local_file: None,
            status: ReconciliationStatus::Missing,
            confidence: 0.0,
            reason: "Not found in local library".to_string(),
        });

        let _ = manager
            .log_event(
                operation_id,
                "track_missing",
                None,
                None,
                None,
                None,
                &serde_json::json!({
                    "desired_track_id": desired.id,
                    "artist": desired.artist_name,
                    "album": desired.album_title,
                    "title": desired.track_title,
                }),
            )
            .await;
    }

    for reconcile in &mut result.reconciliations {
        let Some(local_match) = reconcile.matched_local_file.as_ref() else {
            continue;
        };

        let duplicates = find_duplicates(manager.db_pool(), &reconcile.desired_track, config).await?;
        if !duplicates.is_empty() {
            result.duplicate_count += duplicates.len();
            reconcile.status = ReconciliationStatus::DuplicateFound {
                better_candidate: duplicates[0].file_id,
            };
            let _ = manager
                .log_event(
                    operation_id,
                    "duplicate_detected",
                    Some(duplicates[0].file_id),
                    None,
                    None,
                    None,
                    &serde_json::json!({
                        "desired_track_id": reconcile.desired_track_id,
                        "duplicate_count": duplicates.len(),
                    }),
                )
                .await;
        }

        if should_upgrade_quality(&local_match.quality_tier, config) {
            result.upgrade_count += 1;
            let _ = manager
                .log_event(
                    operation_id,
                    "upgrade_candidate",
                    Some(local_match.file_id),
                    None,
                    None,
                    None,
                    &serde_json::json!({
                        "desired_track_id": reconcile.desired_track_id,
                        "current_quality": local_match.quality_tier,
                        "desired_quality": "lossless",
                    }),
                )
                .await;
        }
    }

    if result.total_desired == 0 {
        return Err(OrchestratorError::ReconciliationFailed(
            "desired track set is empty".to_string(),
        ));
    }

    Ok(result)
}
