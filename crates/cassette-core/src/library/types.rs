use crate::library::state::Module;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaVersion {
    pub current: u32,
    pub expected: u32,
    pub is_compatible: bool,
    pub pending_migrations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct OperationRecord {
    pub operation_id: String,
    pub module: String,
    pub phase: String,
    pub status: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub duration_ms: Option<i64>,
    pub error_message: Option<String>,
    pub error_context_json: Option<String>,
    pub files_affected: i64,
    pub tracks_affected: i64,
    pub metadata_json: Option<String>,
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct OperationEvent {
    pub event_id: i64,
    pub operation_id: String,
    pub event_type: String,
    pub target_file_id: Option<i64>,
    pub target_track_id: Option<i64>,
    pub before_state_json: Option<String>,
    pub after_state_json: Option<String>,
    pub event_data: Option<String>,
    pub timestamp: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationDetails {
    pub operation: OperationRecord,
    pub events: Vec<OperationEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct FileLineageEvent {
    pub event_id: i64,
    pub operation_id: String,
    pub module: String,
    pub phase: String,
    pub event_type: String,
    pub timestamp: Option<String>,
    pub event_data: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileLineage {
    pub file_path: PathBuf,
    pub events: Vec<FileLineageEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct InvariantViolation {
    pub violation_id: i64,
    pub invariant_name: String,
    pub violating_record_json: String,
    pub attempted_operation_id: Option<String>,
    pub detected_at: Option<String>,
    pub resolved: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackReport {
    pub operation_id: String,
    pub events_rolled_back: usize,
    pub failed_rollbacks: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeadlockEdge {
    pub waiting_operation_id: String,
    pub blocking_operation_id: String,
    pub file_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeadlockReport {
    pub detected_at: DateTime<Utc>,
    pub cycle_operations: Vec<String>,
    pub edges: Vec<DeadlockEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveLockView {
    pub file_path: PathBuf,
    pub module: Module,
    pub operation_id: String,
    pub acquired_at: DateTime<Utc>,
}
