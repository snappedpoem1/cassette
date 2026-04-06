use crate::custodian::collision::policies::DuplicatePolicy;
use crate::custodian::error::{CustodianError, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DuplicateDecision {
    KeepBestDiscardWorse,
    KeepAllMarkDuplicates,
    MarkForManualReview,
    QuarantineAsDuplicate,
}

pub fn classify_duplicate(
    policy: DuplicatePolicy,
    incoming_score: i64,
    existing_score: i64,
) -> DuplicateDecision {
    match policy {
        DuplicatePolicy::KeepBest => {
            if incoming_score >= existing_score {
                DuplicateDecision::KeepBestDiscardWorse
            } else {
                DuplicateDecision::MarkForManualReview
            }
        }
        DuplicatePolicy::KeepAll => DuplicateDecision::KeepAllMarkDuplicates,
        DuplicatePolicy::ManualReview => DuplicateDecision::MarkForManualReview,
        DuplicatePolicy::QuarantineAll => DuplicateDecision::QuarantineAsDuplicate,
    }
}

pub fn assert_supported_hash(hash: &Option<String>) -> Result<()> {
    if hash.as_deref().is_some_and(|h| h.is_empty()) {
        return Err(CustodianError::DuplicatePolicyError(
            "empty hash cannot be used for duplicate policy".to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_policy_keep_all_marks_duplicates() {
        let decision = classify_duplicate(DuplicatePolicy::KeepAll, 120, 140);
        assert_eq!(decision, DuplicateDecision::KeepAllMarkDuplicates);
    }
}
