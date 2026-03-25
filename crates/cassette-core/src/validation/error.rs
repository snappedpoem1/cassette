use thiserror::Error;

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Database error: {0}")]
    Sqlx(#[from] sqlx::Error),

    #[error("Library manager error: {0}")]
    Manager(#[from] crate::library::ManagerError),

    #[error("Orchestrator error: {0}")]
    Orchestrator(#[from] crate::orchestrator::OrchestratorError),

    #[error("Director error: {0}")]
    Director(#[from] crate::director::DirectorError),

    #[error("Gatekeeper error: {0}")]
    Gatekeeper(#[from] crate::gatekeeper::GatekeeperError),

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("No operations logged for {0}")]
    NoOperationsLogged(String),

    #[error("Operation has no events: {0}")]
    OperationHasNoEvents(String),

    #[error("Detected stalled operations: {0}")]
    StalledOperations(usize),

    #[error("Detected orphaned successful operations: {0}")]
    OrphanedOperations(usize),
}

pub type Result<T> = std::result::Result<T, ValidationError>;
