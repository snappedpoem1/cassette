use crate::library::state::Module;
use std::path::PathBuf;

#[derive(thiserror::Error, Debug)]
pub enum ManagerError {
    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),

    #[error("Lock acquisition timeout for {file}: held by {locked_by}, waited {waited_ms}ms")]
    LockAcquisitionTimeout {
        file: PathBuf,
        locked_by: Module,
        waited_ms: u64,
    },

    #[error("Deadlock detected: {0}")]
    DeadlockDetected(String),

    #[error("Operation not found: {0}")]
    OperationNotFound(String),

    #[error("Operation not pending (cannot resume): {0}")]
    OperationNotPending(String),

    #[error("Transaction failed: {0}")]
    TransactionFailed(String),

    #[error("Schema mismatch: expected {expected}, got {current}")]
    SchemaMismatch { expected: u32, current: u32 },

    #[error("Invariant violation: {invariant}: {detail}")]
    InvariantViolation { invariant: String, detail: String },

    #[error("File already exists: {0}")]
    FileExists(PathBuf),

    #[error("File not found: {0}")]
    FileNotFound(PathBuf),

    #[error("Operation completed with unsupported status transition")]
    InvalidOperationStatus,

    #[error("Operation context is missing: {0}")]
    OperationContextMissing(String),

    #[error("Module operation failed: {0}")]
    ModuleFailure(String),

    #[error("Download failed: {0}")]
    DownloadFailed(String),

    #[error("Orchestration failed: {0}")]
    OrchestrationFailed(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Time conversion error: {0}")]
    TimeConversion(String),
}

pub type Result<T> = std::result::Result<T, ManagerError>;
