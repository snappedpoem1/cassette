use crate::librarian::models::DesiredTrack;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::orchestrator::delta::action_types::DeltaActionType;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MatchMethod {
    IsrcExact,
    MbidExact,
    SpotifyUriExact,
    FingerprintExact,
    StrongMetadata,
    WeakFuzzy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalFileMatch {
    pub file_id: u64,
    pub track_id: Option<u64>,
    pub file_path: PathBuf,
    pub file_name: String,
    pub codec: String,
    pub bitrate: u32,
    pub quality_tier: String,
    pub artist_name: String,
    pub album_title: String,
    pub title: String,
    pub duration_ms: u64,
    pub content_hash: Option<String>,
    pub acoustid_fingerprint: Option<String>,
    pub matched_via: MatchMethod,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ReconciliationStatus {
    ExactMatch,
    HighConfidenceMatch,
    FuzzyMatch,
    DuplicateFound { better_candidate: u64 },
    Missing,
    ManualReviewNeeded,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackReconciliation {
    pub desired_track_id: u64,
    pub desired_track: DesiredTrack,
    pub matched_local_file: Option<LocalFileMatch>,
    pub status: ReconciliationStatus,
    pub confidence: f32,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ReconciliationResult {
    pub total_desired: usize,
    pub matched_count: usize,
    pub missing_count: usize,
    pub upgrade_count: usize,
    pub duplicate_count: usize,
    pub manual_review_count: usize,
    pub reconciliations: Vec<TrackReconciliation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeltaQueueEntry {
    pub id: u64,
    pub desired_track_id: u64,
    pub action_type: DeltaActionType,
    pub priority: i32,
    pub reason: String,
    pub target_quality: Option<String>,
    pub source_reconciliation_id: u64,
    pub source_operation_id: String,
    pub created_at: DateTime<Utc>,
    pub processed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LibrarianPhaseOutcome {
    pub files_scanned: usize,
    pub files_upserted: usize,
    pub files_quarantined: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CustodianPhaseOutcome {
    pub files_sorted: usize,
    pub files_quarantined: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullSyncOutcome {
    pub root_operation_id: String,
    pub scan_outcome: LibrarianPhaseOutcome,
    pub cleanup_outcome: CustodianPhaseOutcome,
    pub reconciliation: ReconciliationResult,
    pub delta_queue: Vec<DeltaQueueEntry>,
}
