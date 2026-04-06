use crate::librarian::models::{
    DeltaActionType, LocalFile, MatchOutcome, NewDeltaQueueItem, NewReconciliationResult,
    ReconciliationStatus,
};

pub fn quality_assessment(file: Option<&LocalFile>) -> Option<String> {
    file.and_then(|f| f.quality_tier.clone())
}

pub fn classify_delta(
    outcome: &MatchOutcome,
    matched_file: Option<&LocalFile>,
) -> (NewReconciliationResult, NewDeltaQueueItem) {
    let quality = quality_assessment(matched_file);

    let action = match outcome.status {
        ReconciliationStatus::Missing => DeltaActionType::MissingDownload,
        ReconciliationStatus::WeakMatch | ReconciliationStatus::ManualReview => {
            DeltaActionType::ManualReview
        }
        ReconciliationStatus::Duplicate => DeltaActionType::DuplicateReview,
        ReconciliationStatus::ExactMatch | ReconciliationStatus::StrongMatch => {
            if quality.as_deref() == Some("upgrade_candidate") {
                DeltaActionType::UpgradeQuality
            } else {
                DeltaActionType::NoAction
            }
        }
        ReconciliationStatus::UpgradeNeeded => DeltaActionType::UpgradeQuality,
    };

    let priority = match action {
        DeltaActionType::MissingDownload => 100,
        DeltaActionType::UpgradeQuality => 80,
        DeltaActionType::DuplicateReview => 60,
        DeltaActionType::ManualReview => 40,
        DeltaActionType::RelinkMetadata => 30,
        DeltaActionType::NoAction => 0,
    };

    let result = NewReconciliationResult {
        desired_track_id: 0,
        matched_track_id: outcome.matched_track_id,
        matched_local_file_id: outcome.matched_local_file_id,
        reconciliation_status: outcome.status,
        quality_assessment: quality.clone(),
        reason: outcome.reason.clone(),
    };

    let delta = NewDeltaQueueItem {
        desired_track_id: 0,
        action_type: action,
        priority,
        reason: outcome.reason.clone(),
        target_quality: if matches!(
            action,
            DeltaActionType::UpgradeQuality | DeltaActionType::MissingDownload
        ) {
            Some("lossless_preferred".to_string())
        } else {
            None
        },
    };

    (result, delta)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upgrade_candidate_maps_to_upgrade_action() {
        let file = LocalFile {
            id: 1,
            track_id: Some(1),
            file_path: "x.mp3".to_string(),
            file_name: "x.mp3".to_string(),
            extension: "mp3".to_string(),
            codec: Some("mp3".to_string()),
            bitrate: Some(96),
            sample_rate: Some(44100),
            bit_depth: None,
            channels: Some(2),
            duration_ms: Some(200000),
            file_size: 100,
            file_mtime_ms: None,
            content_hash: None,
            acoustid_fingerprint: None,
            fingerprint_attempted_at: None,
            fingerprint_error: None,
            fingerprint_source_mtime_ms: None,
            integrity_status: "readable".to_string(),
            quality_tier: Some("upgrade_candidate".to_string()),
            last_scanned_at: None,
            created_at: String::new(),
            updated_at: String::new(),
        };
        let outcome = MatchOutcome {
            status: ReconciliationStatus::StrongMatch,
            matched_track_id: Some(1),
            matched_local_file_id: Some(1),
            confidence: 0.9,
            reason: "strong metadata".to_string(),
        };

        let (_, delta) = classify_delta(&outcome, Some(&file));
        assert_eq!(delta.action_type, DeltaActionType::UpgradeQuality);
    }

    #[test]
    fn missing_match_maps_to_missing_download() {
        let outcome = MatchOutcome {
            status: ReconciliationStatus::Missing,
            matched_track_id: None,
            matched_local_file_id: None,
            confidence: 0.0,
            reason: "no local match".to_string(),
        };

        let (_, delta) = classify_delta(&outcome, None);
        assert_eq!(delta.action_type, DeltaActionType::MissingDownload);
        assert_eq!(delta.priority, 100);
    }
}
