use crate::library::error::{ManagerError, Result};
use crate::library::manager::LibraryManager;
use crate::orchestrator::{run_full_library_sync_with_manager, OrchestratorConfig};

#[derive(Debug, Clone)]
pub struct OrchestratorOutcome {
    pub root_operation_id: String,
    pub delta_queue: Vec<crate::orchestrator::DeltaQueueEntry>,
    pub summary: String,
}

pub async fn run_orchestrator_with_manager(
    manager: &LibraryManager,
    config: &OrchestratorConfig,
) -> Result<OrchestratorOutcome> {
    match run_full_library_sync_with_manager(manager, config).await {
        Ok(outcome) => Ok(OrchestratorOutcome {
            root_operation_id: outcome.root_operation_id.clone(),
            delta_queue: outcome.delta_queue.clone(),
            summary: format!(
                "Sync complete: {} files scanned, {} cleaned, {} deltas generated",
                outcome.scan_outcome.files_scanned,
                outcome.cleanup_outcome.files_sorted,
                outcome.delta_queue.len(),
            ),
        }),
        Err(error) => Err(ManagerError::OrchestrationFailed(error.to_string())),
    }
}
