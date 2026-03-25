use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ManifestSummary {
    pub total_files_processed: usize,
    pub files_valid: usize,
    pub files_sorted: usize,
    pub files_quarantined: usize,
    pub files_skipped: usize,
    pub duplicates_detected: usize,
    pub collisions_resolved: usize,
    pub errors: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestAction {
    pub source_path: String,
    pub action: String,
    pub destination_path: Option<String>,
    pub validation_status: String,
    pub codec: Option<String>,
    pub bitrate: Option<u32>,
    pub checksum: Option<String>,
    pub db_record_updated: bool,
    pub timestamp: String,
    pub success: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestCollision {
    pub destination_path: String,
    pub existing_file_quality: i64,
    pub incoming_file_quality: i64,
    pub policy: String,
    pub outcome: String,
    pub db_decision_recorded: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustodianManifest {
    pub operation_id: String,
    pub run_timestamp: String,
    pub mode: String,
    pub summary: ManifestSummary,
    pub actions: Vec<ManifestAction>,
    pub collisions: Vec<ManifestCollision>,
}
