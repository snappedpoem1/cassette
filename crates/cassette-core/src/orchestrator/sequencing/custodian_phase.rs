use crate::custodian::run_custodian_cleanup;
use crate::orchestrator::config::OrchestratorConfig;
use crate::orchestrator::error::{OrchestratorError, Result};
use crate::orchestrator::types::CustodianPhaseOutcome;
use std::time::Duration;

pub async fn run_custodian_phase_managed(
    manager: &crate::library::LibraryManager,
    parent_op_id: &str,
    config: &OrchestratorConfig,
) -> Result<CustodianPhaseOutcome> {
    let op_id = manager
        .start_operation(crate::library::Module::Custodian, "cleanup")
        .await
        .map_err(|e| OrchestratorError::ManagerError(e.to_string()))?;

    manager
        .log_event(
            &op_id,
            "cleanup_started",
            None,
            None,
            None,
            None,
            &serde_json::json!({
                "dry_run": config.custodian.dry_run,
            }),
        )
        .await
        .map_err(|e| OrchestratorError::ManagerError(e.to_string()))?;

    let db_pool = manager.db_pool().clone();
    let custodian_config = config.custodian.clone();
    let dry_run = config.custodian.dry_run;

    let mut cleanup_task = tokio::spawn(async move {
        run_custodian_cleanup(&db_pool, &custodian_config, dry_run).await
    });

    let started = std::time::Instant::now();
    let mut heartbeat = tokio::time::interval(Duration::from_secs(30));
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    let cleanup_result = loop {
        tokio::select! {
            _ = heartbeat.tick() => {
                let elapsed = started.elapsed().as_secs();
                println!("Custodian cleanup still running... {}s elapsed", elapsed);
            }
            result = &mut cleanup_task => {
                break result;
            }
        }
    };

    match cleanup_result {
        Ok(result) => match result {
        Ok(outcome) => {
            let phase = CustodianPhaseOutcome {
                files_sorted: outcome.summary.files_sorted,
                files_quarantined: outcome.summary.files_quarantined,
            };

            manager
                .log_event(
                    &op_id,
                    "cleanup_completed",
                    None,
                    None,
                    None,
                    None,
                    &serde_json::json!({
                        "files_processed": outcome.summary.total_files_processed,
                        "files_valid": outcome.summary.files_valid,
                        "files_sorted": outcome.summary.files_sorted,
                        "files_quarantined": outcome.summary.files_quarantined,
                        "files_skipped": outcome.summary.files_skipped,
                        "duplicates_detected": outcome.summary.duplicates_detected,
                        "collisions_detected": outcome.summary.collisions_resolved,
                    }),
                )
                .await
                .map_err(|e| OrchestratorError::ManagerError(e.to_string()))?;

            if outcome.summary.files_quarantined > 0 {
                manager
                    .log_event(
                        &op_id,
                        "files_quarantined",
                        None,
                        None,
                        None,
                        None,
                        &serde_json::json!({
                            "count": outcome.summary.files_quarantined,
                        }),
                    )
                    .await
                    .map_err(|e| OrchestratorError::ManagerError(e.to_string()))?;
            }

            if !outcome.errors.is_empty() {
                for (file_path, error_msg) in &outcome.errors {
                    manager
                        .log_event(
                            &op_id,
                            "cleanup_error",
                            None,
                            None,
                            None,
                            None,
                            &serde_json::json!({
                                "file": file_path,
                                "error": error_msg,
                            }),
                        )
                        .await
                        .map_err(|e| OrchestratorError::ManagerError(e.to_string()))?;
                }
            }

            manager
                .complete_operation(&op_id, crate::library::OperationStatus::Success)
                .await
                .map_err(|e| OrchestratorError::ManagerError(e.to_string()))?;

            manager
                .log_event(
                    parent_op_id,
                    "custodian_cleanup_complete",
                    None,
                    None,
                    None,
                    None,
                    &serde_json::json!({
                        "sorted": phase.files_sorted,
                        "quarantined": phase.files_quarantined,
                    }),
                )
                .await
                .map_err(|e| OrchestratorError::ManagerError(e.to_string()))?;

            Ok(phase)
        }
        Err(error) => {
            manager
                .complete_operation(
                    &op_id,
                    crate::library::OperationStatus::FailedAt(error.to_string()),
                )
                .await
                .map_err(|e| OrchestratorError::ManagerError(e.to_string()))?;
            Err(OrchestratorError::CustodianFailed(error.to_string()))
        }
    },
        Err(error) => {
            manager
                .complete_operation(
                    &op_id,
                    crate::library::OperationStatus::FailedAt(error.to_string()),
                )
                .await
                .map_err(|e| OrchestratorError::ManagerError(e.to_string()))?;
            Err(OrchestratorError::CustodianFailed(format!("custodian task join error: {error}")))
        }
    }
}
