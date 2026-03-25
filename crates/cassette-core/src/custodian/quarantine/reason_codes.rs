use crate::custodian::validation::ValidationStatus;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuarantineReason {
    ZeroByte,
    DecodeFailed,
    MissingMetadata,
    UnsupportedFormat,
    ProbableTruncation,
    HtmlOrText,
    CollisionReview,
    DuplicateReview,
    MissingOnDisk,
}

impl QuarantineReason {
    pub fn as_dir(self) -> &'static str {
        match self {
            Self::ZeroByte => "zero_byte",
            Self::DecodeFailed => "decode_failed",
            Self::MissingMetadata => "missing_metadata",
            Self::UnsupportedFormat => "unsupported_format",
            Self::ProbableTruncation => "probable_truncation",
            Self::HtmlOrText => "html_or_text",
            Self::CollisionReview => "collision_review",
            Self::DuplicateReview => "duplicate_review",
            Self::MissingOnDisk => "missing_on_disk",
        }
    }
}

pub fn quarantine_reason_for_status(status: ValidationStatus) -> Option<QuarantineReason> {
    match status {
        ValidationStatus::ZeroByte => Some(QuarantineReason::ZeroByte),
        ValidationStatus::DecodeFailed | ValidationStatus::UnreadableContainer => {
            Some(QuarantineReason::DecodeFailed)
        }
        ValidationStatus::IncompleteMetadata | ValidationStatus::MetadataOnlyNoAudioProof => {
            Some(QuarantineReason::MissingMetadata)
        }
        ValidationStatus::UnsupportedFormat => Some(QuarantineReason::UnsupportedFormat),
        ValidationStatus::ProbableTruncation
        | ValidationStatus::SuspiciousSmallForDuration
        | ValidationStatus::SuspiciousDurationMismatch => Some(QuarantineReason::ProbableTruncation),
        ValidationStatus::HtmlOrTextPayload => Some(QuarantineReason::HtmlOrText),
        ValidationStatus::MissingOnDisk => Some(QuarantineReason::MissingOnDisk),
        ValidationStatus::Valid => None,
    }
}
