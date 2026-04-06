pub mod config;
pub mod download;
pub mod engine;
pub mod error;
pub mod finalize;
pub mod metadata;
pub mod models;
pub mod provider;
pub mod providers;
pub mod resilience;
pub mod scoring;
pub mod sources;
pub mod strategy;
pub mod temp;
pub mod types;
pub mod validation;

#[cfg(test)]
mod tests;

pub use config::{
    DirectorConfig, DuplicatePolicy, ProviderPolicy, QualityPolicy, RetryPolicy, TempRecoveryPolicy,
};
pub use engine::{Director, DirectorHandle, DirectorSubmission};
pub use error::{DirectorError, FinalizationError, MetadataError, ProviderError, ValidationError};
pub use models::{
    AcquisitionStrategy, CandidateAcquisition, CandidateDisposition, CandidateQuality,
    CandidateScore, CandidateSelection, CandidateSelectionMode, DirectorEvent, DirectorProgress,
    DirectorTaskResult, FinalizedTrack, FinalizedTrackDisposition, NormalizedTrack,
    ProvenanceRecord, ProviderAttemptRecord, ProviderCapabilities, ProviderDescriptor,
    ProviderSearchCandidate, SelectionReason, TrackTask, TrackTaskSource, ValidationIssue,
    ValidationReport,
};
pub use provider::Provider;
pub use sources::{ResolvedTrack, SourceError, SourceProvider};
pub use strategy::{StrategyPlan, StrategyPlanner};
pub use types::{BatchDownloadOutcome, HashAlgorithm, StagedFile};
