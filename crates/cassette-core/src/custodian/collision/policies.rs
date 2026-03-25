use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum DuplicatePolicy {
    KeepBest,
    KeepAll,
    ManualReview,
    QuarantineAll,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum CollisionPolicy {
    RenameIncoming,
    QuarantineIncoming,
    CompareAndReplace,
    ManualReview,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum CollisionOutcome {
    KeepExistingMarkIncomingDuplicate,
    ReplaceExistingIfIncomingBetter,
    RenameIncomingWithSuffixForReview,
    QuarantineIncomingAsDuplicate,
    ManualReviewRequired,
    NoActionFileAlreadySorted,
}
