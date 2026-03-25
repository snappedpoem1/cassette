#[derive(thiserror::Error, Debug)]
pub enum OrchestratorError {
    #[error("Librarian failed: {0}")]
    LibrarianFailed(String),

    #[error("Custodian failed: {0}")]
    CustodianFailed(String),

    #[error("Reconciliation failed: {0}")]
    ReconciliationFailed(String),

    #[error("Desired track not found: {0}")]
    DesiredTrackNotFound(u64),

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Manager error: {0}")]
    ManagerError(String),

    #[error("No match found for desired track {0}")]
    NoMatchFound(u64),

    #[error("Ambiguous match: {0}")]
    AmbiguousMatch(String),

    #[error("Serialization error: {0}")]
    Serialization(String),
}

pub type Result<T> = std::result::Result<T, OrchestratorError>;

impl From<sqlx::Error> for OrchestratorError {
    fn from(value: sqlx::Error) -> Self {
        Self::DatabaseError(value.to_string())
    }
}

impl From<serde_json::Error> for OrchestratorError {
    fn from(value: serde_json::Error) -> Self {
        Self::Serialization(value.to_string())
    }
}
