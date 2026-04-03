use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedTrack {
    pub spotify_track_id: Option<String>,
    #[serde(default)]
    pub source_album_id: Option<String>,
    #[serde(default)]
    pub source_artist_id: Option<String>,
    pub source_playlist: Option<String>,
    pub artist: String,
    pub album_artist: Option<String>,
    pub title: String,
    pub album: Option<String>,
    pub track_number: Option<u32>,
    pub disc_number: Option<u32>,
    pub year: Option<i32>,
    pub duration_secs: Option<f64>,
    pub isrc: Option<String>,
    #[serde(default)]
    pub musicbrainz_recording_id: Option<String>,
    #[serde(default)]
    pub musicbrainz_release_id: Option<String>,
    #[serde(default)]
    pub canonical_artist_id: Option<i64>,
    #[serde(default)]
    pub canonical_release_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackTask {
    pub task_id: String,
    pub source: TrackTaskSource,
    #[serde(default)]
    pub desired_track_id: Option<i64>,
    #[serde(default)]
    pub source_operation_id: Option<String>,
    pub target: NormalizedTrack,
    pub strategy: AcquisitionStrategy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrackTaskSource {
    SpotifyLibrary,
    SpotifyHistory,
    SpotifyPlaylist { playlist_id: String },
    Manual,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AcquisitionStrategy {
    Standard,
    HighQualityOnly,
    ObscureFallbackHeavy,
    DiscographyBatch,
    SingleTrackPriority,
    MetadataRepairOnly,
    RedownloadReplaceIfBetter,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderDescriptor {
    pub id: String,
    pub display_name: String,
    pub trust_rank: i32,
    pub capabilities: ProviderCapabilities,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProviderCapabilities {
    pub supports_search: bool,
    pub supports_download: bool,
    pub supports_lossless: bool,
    pub supports_batch: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderSearchCandidate {
    pub provider_id: String,
    pub provider_candidate_id: String,
    pub artist: String,
    pub title: String,
    pub album: Option<String>,
    pub duration_secs: Option<f64>,
    pub extension_hint: Option<String>,
    pub bitrate_kbps: Option<u32>,
    pub cover_art_url: Option<String>,
    pub metadata_confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateAcquisition {
    pub provider_id: String,
    pub provider_candidate_id: String,
    pub temp_path: PathBuf,
    pub file_size: u64,
    pub extension_hint: Option<String>,
    #[serde(default)]
    pub resolved_metadata: Option<NormalizedTrack>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum CandidateQuality {
    Lossless,
    Lossy,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationIssue {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationReport {
    pub is_valid: bool,
    pub format_name: Option<String>,
    pub duration_secs: Option<f64>,
    pub audio_readable: bool,
    pub header_readable: bool,
    pub extension_ok: bool,
    pub file_size: u64,
    pub quality: CandidateQuality,
    pub issues: Vec<ValidationIssue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateScore {
    pub total: i32,
    pub metadata_match_points: i32,
    pub duration_points: i32,
    pub codec_points: i32,
    pub provider_points: i32,
    pub validation_points: i32,
    pub size_points: i32,
    pub bitrate_points: i32,
    pub format_points: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectionReason {
    pub summary: String,
    pub details: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateSelection {
    pub provider_id: String,
    pub provider_candidate_id: String,
    pub temp_path: PathBuf,
    pub score: CandidateScore,
    pub reason: SelectionReason,
    pub validation: ValidationReport,
    pub cover_art_url: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum CandidateSelectionMode {
    FirstValidWins,
    CompareAllCandidates,
    /// Race the top N providers in parallel, pick the best candidate from responders.
    CompareTopN(usize),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderAttemptRecord {
    pub provider_id: String,
    pub attempt: u32,
    pub outcome: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvenanceRecord {
    pub task_id: String,
    pub source_metadata: NormalizedTrack,
    pub selected_provider: String,
    pub selected_provider_candidate_id: Option<String>,
    pub score_reason: SelectionReason,
    pub validation_summary: ValidationReport,
    pub final_path: PathBuf,
    pub acquired_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinalizedTrack {
    pub path: PathBuf,
    pub replaced_existing: bool,
    pub provenance: ProvenanceRecord,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum FinalizedTrackDisposition {
    Finalized,
    AlreadyPresent,
    MetadataOnly,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectorTaskResult {
    pub task_id: String,
    pub disposition: FinalizedTrackDisposition,
    pub finalized: Option<FinalizedTrack>,
    pub attempts: Vec<ProviderAttemptRecord>,
    pub error: Option<String>,
    #[serde(default)]
    pub candidate_records: Vec<CandidateRecord>,
    #[serde(default)]
    pub provider_searches: Vec<ProviderSearchRecord>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum DirectorProgress {
    Queued,
    InProgress,
    ProviderAttempt,
    Validating,
    Tagging,
    Finalizing,
    Finalized,
    Cancelled,
    Failed,
    Exhausted,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectorEvent {
    pub task_id: String,
    pub progress: DirectorProgress,
    pub provider_id: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProviderHealthStatus {
    Unknown,
    Healthy,
    Down,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderHealthState {
    pub provider_id: String,
    pub status: ProviderHealthStatus,
    pub checked_at: DateTime<Utc>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateDisposition {
    pub candidate: ProviderSearchCandidate,
    pub acquisition: CandidateAcquisition,
    pub validation: ValidationReport,
    pub score: CandidateScore,
    pub score_reason: SelectionReason,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateRecord {
    pub provider_id: String,
    pub provider_display_name: String,
    pub provider_trust_rank: i32,
    pub provider_order_index: usize,
    pub search_rank: usize,
    pub candidate: ProviderSearchCandidate,
    pub acquisition_temp_path: Option<PathBuf>,
    pub validation: Option<ValidationReport>,
    pub score: Option<CandidateScore>,
    pub score_reason: Option<SelectionReason>,
    pub outcome: String,
    pub rejection_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderSearchRecord {
    pub provider_id: String,
    pub provider_display_name: String,
    pub provider_trust_rank: i32,
    pub provider_order_index: usize,
    pub outcome: String,
    pub candidate_count: usize,
    pub error: Option<String>,
    pub retryable: bool,
}
