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
    pub file_mtime_ms: Option<i64>,
    pub content_hash: Option<String>,
    pub acoustid_fingerprint: Option<String>,
    #[sqlx(default)]
    pub fingerprint_attempted_at: Option<String>,
    #[sqlx(default)]
    pub fingerprint_error: Option<String>,
    #[sqlx(default)]
    pub fingerprint_source_mtime_ms: Option<i64>,
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
    pub file_mtime_ms: Option<i64>,
    pub content_hash: Option<String>,
    pub acoustid_fingerprint: Option<String>,
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
    pub skipped_files: u64,
    pub unreadable_files: u64,
    pub suspicious_files: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ScanCheckpoint {
    pub id: i64,
    pub root_path: String,
    pub last_run_id: Option<String>,
    pub last_scanned_path: Option<String>,
    pub status: String,
    pub files_seen: i64,
    pub files_indexed: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LocalFileScanState {
    pub file_path: String,
    pub file_size: i64,
    pub file_mtime_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CanonicalArtist {
    pub id: i64,
    pub name: String,
    pub normalized_name: String,
    pub musicbrainz_id: Option<String>,
    pub spotify_id: Option<String>,
    pub discogs_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CanonicalRelease {
    pub id: i64,
    pub canonical_artist_id: i64,
    pub title: String,
    pub normalized_title: String,
    pub release_group_mbid: Option<String>,
    pub release_mbid: Option<String>,
    pub spotify_id: Option<String>,
    pub discogs_id: Option<String>,
    pub release_type: Option<String>,
    pub year: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CanonicalRecording {
    pub id: i64,
    pub canonical_artist_id: Option<i64>,
    pub canonical_release_id: Option<i64>,
    pub title: String,
    pub normalized_title: String,
    pub musicbrainz_recording_id: Option<String>,
    pub isrc: Option<String>,
    pub track_number: Option<i64>,
    pub disc_number: Option<i64>,
    pub duration_ms: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AcquisitionRequestRow {
    pub id: i64,
    pub scope: String,
    pub source_name: String,
    pub source_track_id: Option<String>,
    pub source_album_id: Option<String>,
    pub source_artist_id: Option<String>,
    pub artist: String,
    pub album: Option<String>,
    pub title: String,
    pub normalized_artist: String,
    pub normalized_album: Option<String>,
    pub normalized_title: String,
    pub track_number: Option<i64>,
    pub disc_number: Option<i64>,
    pub year: Option<i64>,
    pub duration_secs: Option<f64>,
    pub isrc: Option<String>,
    pub musicbrainz_recording_id: Option<String>,
    pub musicbrainz_release_group_id: Option<String>,
    pub musicbrainz_release_id: Option<String>,
    pub canonical_artist_id: Option<i64>,
    pub canonical_release_id: Option<i64>,
    pub strategy: String,
    pub quality_policy: Option<String>,
    pub excluded_providers_json: Option<String>,
    pub edition_policy: Option<String>,
    pub confirmation_policy: String,
    pub desired_track_id: Option<i64>,
    pub source_operation_id: Option<String>,
    pub task_id: Option<String>,
    pub request_signature: String,
    pub status: String,
    pub raw_payload_json: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AcquisitionRequestEvent {
    pub id: i64,
    pub request_id: i64,
    pub task_id: Option<String>,
    pub event_type: String,
    pub status: String,
    pub message: Option<String>,
    pub payload_json: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchOutcome {
    pub status: ReconciliationStatus,
    pub matched_track_id: Option<i64>,
    pub matched_local_file_id: Option<i64>,
    pub confidence: f32,
    pub reason: String,
}
