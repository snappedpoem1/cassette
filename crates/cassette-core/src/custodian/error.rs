use thiserror::Error;

#[derive(Error, Debug)]
pub enum CustodianError {
    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Filesystem error: {0}")]
    FilesystemError(#[from] std::io::Error),

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Decode error: {0}")]
    DecodeError(String),

    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),

    #[error("Collision error: {0}")]
    CollisionError(String),

    #[error("Duplicate policy error: {0}")]
    DuplicatePolicyError(String),

    #[error("Quarantine error: {0}")]
    QuarantineError(String),

    #[error("Sync error: {0}")]
    SyncError(String),

    #[error("Staging error: {0}")]
    StagingError(String),

    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, CustodianError>;
