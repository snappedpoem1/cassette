use thiserror::Error;

#[derive(Error, Debug)]
pub enum GatekeeperError {
    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Filesystem error: {0}")]
    FilesystemError(#[from] std::io::Error),

    #[error("Payload validation failed: {0}")]
    PayloadValidationFailed(String),

    #[error("Decode probe failed: {0}")]
    DecodeFailed(String),

    #[error("Fingerprint computation failed: {0}")]
    FingerprintFailed(String),

    #[error("Identity proof failed: {0}")]
    IdentityProofFailed(String),

    #[error("Quality assessment failed: {0}")]
    QualityAssessmentFailed(String),

    #[error("Junk filter triggered: {0}")]
    JunkFilterTriggered(String),

    #[error("Duplicate detected: {0}")]
    DuplicateDetected(String),

    #[error("Collision handling failed: {0}")]
    CollisionError(String),

    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),

    #[error("Verification failed: {0}")]
    VerificationFailed(String),

    #[error("Quarantine failed: {0}")]
    QuarantineError(String),

    #[error("Admission failed: {0}")]
    AdmissionError(String),

    #[error("Timeout during {operation}: {duration_ms}ms")]
    Timeout { operation: String, duration_ms: u64 },

    #[error("Unsupported format: {0}")]
    UnsupportedFormat(String),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, GatekeeperError>;
