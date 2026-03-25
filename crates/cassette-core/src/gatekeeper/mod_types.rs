use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngressOutcome {
    pub file_path: PathBuf,
    pub decision: AdmissionDecision,
    pub identity_proof: Option<IdentityProof>,
    pub quality_assessment: QualityAssessment,
    pub junk_flags: Vec<JunkFlag>,
    pub audit_log: AuditLogEntry,
    pub next_action: NextAction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AdmissionDecision {
    Admitted { canonical_path: PathBuf, confidence: f32 },
    Quarantined { reason: QuarantineReason, manual_review_required: bool },
    Rejected { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityProof {
    pub codec: String,
    pub bitrate: u32,
    pub sample_rate: u32,
    pub bit_depth: u8,
    pub channels: u8,
    pub duration_ms: u64,
    pub file_size: u64,
    pub content_hash: String,
    pub acoustid_fingerprint: String,
    pub acoustid_confidence: f32,
    pub matched_desired_track: bool,
    pub matched_isrc: Option<bool>,
    pub matched_mbid: Option<bool>,
    pub validation_timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityAssessment {
    pub passes_bitrate_floor: bool,
    pub passes_sample_rate_floor: bool,
    pub is_lossless: bool,
    pub quality_tier: QualityTier,
    pub upgrade_from_local: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum QualityTier {
    Lossless,
    LossyHi,
    LossyAcceptable,
    BelowFloor,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum JunkFlag {
    IsKaraoke,
    IsLiveVersion,
    IsInstrumental,
    IsRemix,
    IsDemo,
    IsUnofficial,
    IsAltVersion,
    IsInterlude,
    IsSkitOrSpeech,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum QuarantineReason {
    PayloadCorrupted,
    QualityBelowFloor,
    JunkFilterTriggered,
    IdentityMismatch,
    DuplicateDetected,
    AmbiguousIdentity,
    UnsupportedFormat,
    FileSizeImprobable,
    ManualReview,
}

impl QuarantineReason {
    pub fn as_dir(self) -> &'static str {
        match self {
            Self::PayloadCorrupted => "decode_failed",
            Self::QualityBelowFloor => "below_quality_floor",
            Self::JunkFilterTriggered => "junk_detected",
            Self::IdentityMismatch => "identity_mismatch",
            Self::DuplicateDetected => "duplicate_review",
            Self::AmbiguousIdentity => "manual_review",
            Self::UnsupportedFormat => "unsupported_format",
            Self::FileSizeImprobable => "file_size_improbable",
            Self::ManualReview => "manual_review",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NextAction {
    None,
    TriggerEnrichment { local_file_id: i64, track_id: Option<i64> },
    RetryLater { reason: String },
    ManualReview { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogEntry {
    pub operation_id: String,
    pub timestamp: DateTime<Utc>,
    pub file_path: PathBuf,
    pub decision: AdmissionDecision,
    pub identity_proof: Option<IdentityProof>,
    pub quality_assessment: QualityAssessment,
    pub junk_flags: Vec<JunkFlag>,
    pub desired_track_id: Option<i64>,
    pub matched_local_file_id: Option<i64>,
    pub duration_ms: u64,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchIngestOutcome {
    pub total_files: usize,
    pub admitted: usize,
    pub quarantined: usize,
    pub rejected: usize,
    pub duplicates_detected: usize,
    pub audit_entries: Vec<IngressOutcome>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayloadProbe {
    pub codec: String,
    pub bitrate: u32,
    pub sample_rate: u32,
    pub bit_depth: u8,
    pub channels: u8,
    pub duration_ms: u64,
    pub file_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioTags {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub isrc: Option<String>,
    pub track_number: Option<u32>,
    pub disc_number: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateConflict {
    pub existing_local_file_id: i64,
    pub existing_file_path: PathBuf,
    pub existing_quality: QualityTier,
    pub incoming_quality: QualityTier,
    pub incoming_is_better: bool,
    pub policy_decision: DuplicatePolicyOutcome,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum DuplicatePolicyOutcome {
    KeepExisting,
    ReplaceExisting,
    MarkBothKeepBest,
    ManualReview,
}
