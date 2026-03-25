use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Artist {
    pub id: i64,
    pub canonical_name: String,
    pub normalized_name: String,
    pub spotify_id: Option<String>,
    pub discogs_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Album {
    pub id: i64,
    pub artist_id: i64,
    pub title: String,
    pub normalized_title: String,
    pub release_date: Option<String>,
    pub spotify_id: Option<String>,
    pub discogs_id: Option<String>,
    pub cover_art_path: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Track {
    pub id: i64,
    pub album_id: Option<i64>,
    pub artist_id: i64,
    pub title: String,
    pub normalized_title: String,
    pub track_number: Option<i64>,
    pub disc_number: Option<i64>,
    pub duration_ms: Option<i64>,
    pub isrc: Option<String>,
    pub spotify_id: Option<String>,
    pub discogs_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum IntegrityStatus {
    Readable,
    Unreadable,
    PartialMetadata,
    UnknownFormat,
    Suspicious,
}

impl IntegrityStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Readable => "readable",
            Self::Unreadable => "unreadable",
            Self::PartialMetadata => "partial_metadata",
            Self::UnknownFormat => "unknown_format",
            Self::Suspicious => "suspicious",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum QualityTier {
    LosslessPreferred,
    LossyAcceptable,
    UpgradeCandidate,
    DuplicateWorse,
    DuplicateEquivalent,
}

impl QualityTier {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LosslessPreferred => "lossless_preferred",
            Self::LossyAcceptable => "lossy_acceptable",
            Self::UpgradeCandidate => "upgrade_candidate",
            Self::DuplicateWorse => "duplicate_worse",
            Self::DuplicateEquivalent => "duplicate_equivalent",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LocalFile {
    pub id: i64,
    pub track_id: Option<i64>,
    pub file_path: String,
    pub file_name: String,
    pub extension: String,
    pub codec: Option<String>,
    pub bitrate: Option<i64>,
    pub sample_rate: Option<i64>,
    pub bit_depth: Option<i64>,
    pub channels: Option<i64>,
    pub duration_ms: Option<i64>,
    pub file_size: i64,
    pub content_hash: Option<String>,
    pub integrity_status: String,
    pub quality_tier: Option<String>,
    pub last_scanned_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewLocalFile {
    pub track_id: Option<i64>,
    pub file_path: String,
    pub file_name: String,
    pub extension: String,
    pub codec: Option<String>,
    pub bitrate: Option<i64>,
    pub sample_rate: Option<i64>,
    pub bit_depth: Option<i64>,
    pub channels: Option<i64>,
    pub duration_ms: Option<i64>,
    pub file_size: i64,
    pub content_hash: Option<String>,
    pub integrity_status: IntegrityStatus,
    pub quality_tier: Option<QualityTier>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DesiredTrack {
    pub id: i64,
    pub source_name: String,
    pub source_track_id: Option<String>,
    pub source_album_id: Option<String>,
    pub source_artist_id: Option<String>,
    pub artist_name: String,
    pub album_title: Option<String>,
    pub track_title: String,
    pub track_number: Option<i64>,
    pub disc_number: Option<i64>,
    pub duration_ms: Option<i64>,
    pub isrc: Option<String>,
    pub raw_payload_json: Option<String>,
    pub imported_at: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ReconciliationStatus {
    ExactMatch,
    StrongMatch,
    WeakMatch,
    Missing,
    UpgradeNeeded,
    Duplicate,
    ManualReview,
}

impl ReconciliationStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ExactMatch => "exact_match",
            Self::StrongMatch => "strong_match",
            Self::WeakMatch => "weak_match",
            Self::Missing => "missing",
            Self::UpgradeNeeded => "upgrade_needed",
            Self::Duplicate => "duplicate",
            Self::ManualReview => "manual_review",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewReconciliationResult {
    pub desired_track_id: i64,
    pub matched_track_id: Option<i64>,
    pub matched_local_file_id: Option<i64>,
    pub reconciliation_status: ReconciliationStatus,
    pub quality_assessment: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum DeltaActionType {
    MissingDownload,
    UpgradeQuality,
    RelinkMetadata,
    ManualReview,
    DuplicateReview,
    NoAction,
}

impl DeltaActionType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MissingDownload => "missing_download",
            Self::UpgradeQuality => "upgrade_quality",
            Self::RelinkMetadata => "relink_metadata",
            Self::ManualReview => "manual_review",
            Self::DuplicateReview => "duplicate_review",
            Self::NoAction => "no_action",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewDeltaQueueItem {
    pub desired_track_id: i64,
    pub action_type: DeltaActionType,
    pub priority: i64,
    pub reason: String,
    pub target_quality: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScanStats {
    pub discovered_files: u64,
    pub scanned_files: u64,
    pub unreadable_files: u64,
    pub suspicious_files: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchOutcome {
    pub status: ReconciliationStatus,
    pub matched_track_id: Option<i64>,
    pub matched_local_file_id: Option<i64>,
    pub confidence: f32,
    pub reason: String,
}
