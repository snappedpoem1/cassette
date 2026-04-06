use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DirectorError {
    #[error("configuration error: {0}")]
    Config(String),
    #[error("queue error: {0}")]
    Queue(String),
    #[error("provider exhausted for task {task_id}")]
    ProviderExhausted { task_id: String },
    #[error("provider error: {0}")]
    Provider(#[from] ProviderError),
    #[error("validation error: {0}")]
    Validation(#[from] ValidationError),
    #[error("metadata error: {0}")]
    Metadata(#[from] MetadataError),
    #[error("filesystem error: {0}")]
    Finalization(#[from] FinalizationError),
    #[error("task cancelled")]
    TaskCancelled,
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("task join error: {0}")]
    Join(#[from] tokio::task::JoinError),
    #[error("no available sources: {0}")]
    NoAvailableSources(String),
    #[error("source error ({provider}): {error}")]
    SourceError { provider: String, error: String },
    #[error("network error: {0}")]
    NetworkError(String),
    #[error("http error: {0}")]
    HttpError(u16),
    #[error("http error {status} with retry-after {retry_after_secs}s")]
    HttpRetryAfter { status: u16, retry_after_secs: u64 },
    #[error("download timeout")]
    Timeout,
    #[error("file too large: {size} bytes (max: {max})")]
    FileTooLarge { size: usize, max: usize },
    #[error("staging error: {0}")]
    StagingError(String),
    #[error("lock acquisition failed: {0}")]
    LockError(String),
    #[error("database error: {0}")]
    DatabaseError(String),
    #[error("verification failed: {0}")]
    VerificationError(String),
}

#[derive(Debug, Error, Clone)]
pub enum ProviderError {
    #[error("provider {provider_id} auth failed")]
    AuthFailed { provider_id: String },
    #[error("provider {provider_id} rate limited")]
    RateLimited { provider_id: String },
    #[error("provider {provider_id} timed out")]
    TimedOut { provider_id: String },
    #[error("provider {provider_id} not found")]
    NotFound { provider_id: String },
    #[error("provider {provider_id} network error: {message}")]
    Network {
        provider_id: String,
        message: String,
    },
    #[error("provider {provider_id} unsupported content: {message}")]
    UnsupportedContent {
        provider_id: String,
        message: String,
    },
    #[error("provider {provider_id} metadata mismatch: {message}")]
    MetadataMismatch {
        provider_id: String,
        message: String,
    },
    #[error("provider {provider_id} invalid audio: {message}")]
    InvalidAudio {
        provider_id: String,
        message: String,
    },
    #[error("provider {provider_id} temporary outage: {message}")]
    TemporaryOutage {
        provider_id: String,
        message: String,
    },
    #[error("provider {provider_id} other failure: {message}")]
    Other {
        provider_id: String,
        message: String,
    },
    #[error("provider {provider_id} busy (semaphore full)")]
    ProviderBusy { provider_id: String },
}

impl ProviderError {
    pub fn retryable(&self) -> bool {
        matches!(
            self,
            Self::RateLimited { .. }
                | Self::TimedOut { .. }
                | Self::Network { .. }
                | Self::TemporaryOutage { .. }
        )
    }

    pub fn is_busy(&self) -> bool {
        matches!(self, Self::ProviderBusy { .. })
    }
}

#[derive(Debug, Error, Clone)]
pub enum ValidationError {
    #[error("candidate is html or text payload, not audio")]
    HtmlPayload,
    #[error("candidate file is empty")]
    EmptyFile,
    #[error("audio container unreadable: {message}")]
    UnreadableContainer { message: String },
    #[error("audio duration implausible: {message}")]
    ImplausibleDuration { message: String },
    #[error("extension mismatch: expected {expected}, actual {actual}")]
    ExtensionMismatch { expected: String, actual: String },
    #[error("codec mismatch: expected {expected}, actual {actual}")]
    CodecMismatch { expected: String, actual: String },
    #[error("candidate rejected: {message}")]
    Rejected { message: String },
}

#[derive(Debug, Error, Clone)]
pub enum MetadataError {
    #[error("metadata tagging failed for {path:?}: {message}")]
    TagWrite { path: PathBuf, message: String },
}

#[derive(Debug, Error, Clone)]
pub enum FinalizationError {
    #[error("destination already exists: {path:?}")]
    DestinationExists { path: PathBuf },
    #[error("replacement candidate is not better than existing file: {path:?}")]
    ReplacementRejected { path: PathBuf },
    #[error("atomic move failed from {from:?} to {to:?}: {message}")]
    MoveFailed {
        from: PathBuf,
        to: PathBuf,
        message: String,
    },
}
