use crate::gatekeeper::config::GatekeeperConfig;
use crate::gatekeeper::database::ensure_schema as ensure_gatekeeper_schema;
use crate::gatekeeper::mod_types::{AdmissionDecision, BatchIngestOutcome, IngressOutcome};
use crate::gatekeeper::orchestrator::ingest_single_file;
use crate::librarian::models::DesiredTrack;
use crate::library::error::{ManagerError, Result};
use crate::library::manager::LibraryManager;
use crate::library::state::{Module, OperationStatus};
use std::path::Path;

impl LibraryManager {
    pub async fn run_gatekeeper_with_manager(
        &self,
        entries: &[(&Path, Option<&DesiredTrack>)],
        config: &GatekeeperConfig,
    ) -> Result<BatchIngestOutcome> {
        ensure_gatekeeper_schema(&self.db_pool).await.map_err(|e| {
            ManagerError::ModuleFailure(format!("gatekeeper schema init failed: {e}"))
        })?;

        let operation_id = self
            .start_operation(Module::Gatekeeper, "batch_ingest")
            .await?;

        let mut audit_entries: Vec<IngressOutcome> = Vec::new();
        let mut admitted = 0usize;
        let mut quarantined = 0usize;
        let mut rejected = 0usize;
        let duplicates_detected = 0usize;

        for (file_path, desired_track) in entries {
            let _lock = self
                .acquire_lock_for_file(file_path, Module::Gatekeeper, &operation_id, 30_000)
                .await?;

            match ingest_single_file(&self.db_pool, config, file_path, *desired_track).await {
                Ok(outcome) => {
                    let decision_type = match &outcome.decision {
                        AdmissionDecision::Admitted { .. } => {
                            admitted += 1;
                            "admitted"
                        }
                        AdmissionDecision::Quarantined { .. } => {
                            quarantined += 1;
                            "quarantined"
                        }
                        AdmissionDecision::Rejected { .. } => {
                            rejected += 1;
                            "rejected"
                        }
                    };

                    self.log_event(
                        &operation_id,
                        &format!("gatekeeper_{}", decision_type),
                        outcome.audit_log.matched_local_file_id.map(|id| id as u64),
                        None,
                        None,
                        None,
                        &serde_json::json!({
                            "file_path": file_path.to_string_lossy(),
                            "desired_track_id": outcome.audit_log.desired_track_id,
                            "decision": format!("{:?}", outcome.decision),
                            "duration_ms": outcome.audit_log.duration_ms,
                        }),
                    )
                    .await?;

                    audit_entries.push(outcome);
                }
                Err(error) => {
                    rejected += 1;
                    self.log_event(
                        &operation_id,
                        "gatekeeper_ingest_failed",
                        None,
                        None,
                        None,
                        None,
                        &serde_json::json!({
                            "desired_track_id": desired_track.map(|d| d.id),
                            "error": error.to_string(),
                        }),
                    )
                    .await?;
                }
            }
        }

        self.log_event(
            &operation_id,
            "batch_ingest_complete",
            None,
            None,
            None,
            None,
            &serde_json::json!({
                "total_files": entries.len(),
                "admitted": admitted,
                "quarantined": quarantined,
                "rejected": rejected,
            }),
        )
        .await?;

        self.complete_operation(&operation_id, OperationStatus::Success)
            .await?;

        if duplicates_detected > entries.len() {
            return Err(ManagerError::ModuleFailure(
                "duplicate tracking invariant exceeded total entries".to_string(),
            ));
        }

        Ok(BatchIngestOutcome {
            total_files: entries.len(),
            admitted,
            quarantined,
            rejected,
            duplicates_detected,
            audit_entries,
        })
    }
}
