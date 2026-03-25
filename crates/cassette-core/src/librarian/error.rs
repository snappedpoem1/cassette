use thiserror::Error;

#[derive(Error, Debug)]
pub enum LibrarianError {
    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Scan error: {0}")]
    ScanError(String),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),

    #[error("Matching error: {0}")]
    MatchingError(String),

    #[error("Reconciliation error: {0}")]
    ReconciliationError(String),

    #[error("Integrity error: {0}")]
    IntegrityError(String),

    #[error("Quality assessment error: {0}")]
    QualityError(String),

    #[error("Import error: {0}")]
    ImportError(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, LibrarianError>;
