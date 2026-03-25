use crate::custodian::collision::policies::{CollisionOutcome, CollisionPolicy};

pub fn resolve_collision(
    policy: CollisionPolicy,
    incoming_quality_score: i64,
    existing_quality_score: i64,
    hashes_equal: bool,
) -> CollisionOutcome {
    if hashes_equal {
        return CollisionOutcome::NoActionFileAlreadySorted;
    }

    match policy {
        CollisionPolicy::RenameIncoming => CollisionOutcome::RenameIncomingWithSuffixForReview,
        CollisionPolicy::QuarantineIncoming => CollisionOutcome::QuarantineIncomingAsDuplicate,
        CollisionPolicy::ManualReview => CollisionOutcome::ManualReviewRequired,
        CollisionPolicy::CompareAndReplace => {
            if incoming_quality_score > existing_quality_score {
                CollisionOutcome::ReplaceExistingIfIncomingBetter
            } else {
                CollisionOutcome::KeepExistingMarkIncomingDuplicate
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::custodian::collision::policies::CollisionPolicy;

    #[test]
    fn compare_and_replace_honors_quality() {
        let keep = resolve_collision(CollisionPolicy::CompareAndReplace, 100, 120, false);
        let replace = resolve_collision(CollisionPolicy::CompareAndReplace, 140, 120, false);
        assert!(matches!(keep, CollisionOutcome::KeepExistingMarkIncomingDuplicate));
        assert!(matches!(replace, CollisionOutcome::ReplaceExistingIfIncomingBetter));
    }
}
