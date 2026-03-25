use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LibraryState {
    Idle,
    ScanningLibrarian,
    CleaningCustodian,
    IngestionGatekeeper,
    EnrichmentBackground,
    ReconciliationOrchestrator,
    Recovering,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Module {
    Librarian,
    Custodian,
    Orchestrator,
    Director,
    Gatekeeper,
    Enricher,
}

impl Module {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Librarian => "librarian",
            Self::Custodian => "custodian",
            Self::Orchestrator => "orchestrator",
            Self::Director => "director",
            Self::Gatekeeper => "gatekeeper",
            Self::Enricher => "enricher",
        }
    }

    pub fn from_db(value: &str) -> Option<Self> {
        match value {
            "librarian" => Some(Self::Librarian),
            "custodian" => Some(Self::Custodian),
            "orchestrator" => Some(Self::Orchestrator),
            "director" => Some(Self::Director),
            "gatekeeper" => Some(Self::Gatekeeper),
            "enricher" => Some(Self::Enricher),
            _ => None,
        }
    }
}

impl Display for Module {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OperationStatus {
    InProgress,
    Success,
    PartialSuccess { completed: usize, failed: usize },
    FailedAt(String),
    RolledBack,
}

impl OperationStatus {
    pub fn as_db_status(&self) -> &'static str {
        match self {
            Self::InProgress => "in_progress",
            Self::Success => "success",
            Self::PartialSuccess { .. } => "partial_success",
            Self::FailedAt(_) => "failed",
            Self::RolledBack => "rolled_back",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileLock {
    pub file_path: PathBuf,
    pub locked_by: Module,
    pub acquired_at: DateTime<Utc>,
    pub operation_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationContext {
    pub operation_id: String,
    pub module: Module,
    pub phase: String,
    pub started_at: DateTime<Utc>,
    pub affected_files: Vec<PathBuf>,
    pub affected_tracks: Vec<u64>,
    pub waiting_on_file: Option<PathBuf>,
    pub status: OperationStatus,
}
