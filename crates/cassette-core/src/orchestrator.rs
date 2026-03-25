pub mod config;
pub mod delta;
pub mod error;
pub mod reconciliation;
pub mod sequencing;
pub mod types;

#[cfg(test)]
mod tests;

pub use config::{OrchestratorConfig, ReconciliationConfig};
pub use delta::action_types::DeltaActionType;
pub use delta::adapter::DeltaQueueAdapter;
pub use delta::generation::generate_delta_queue_managed;
pub use error::{OrchestratorError, Result};
pub use reconciliation::engine::reconcile_desired_against_local;
pub use sequencing::full_sync::run_full_library_sync_with_manager;
pub use types::{
	DeltaQueueEntry, FullSyncOutcome, LocalFileMatch, MatchMethod, ReconciliationResult,
	ReconciliationStatus, TrackReconciliation,
};
