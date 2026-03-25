use crate::director::config::DirectorConfig;
use crate::director::download::batch_download;
use crate::director::sources::SourceProvider;
use crate::director::types::BatchDownloadOutcome;
use crate::library::error::{ManagerError, Result};
use crate::library::manager::LibraryManager;
use crate::library::state::{Module, OperationStatus};
use crate::librarian::models::DesiredTrack;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct DirectorOutcome {
    pub operation_id: String,
    pub staged_files: usize,
    pub failed_downloads: usize,
}

impl LibraryManager {
    pub async fn run_director_with_manager(
        &self,
        desired_tracks: &[DesiredTrack],
        sources: &[Arc<dyn SourceProvider>],
        config: &DirectorConfig,
    ) -> Result<DirectorOutcome> {
        let op_id = self.start_operation(Module::Director, "batch_download").await?;

        let result: std::result::Result<BatchDownloadOutcome, crate::director::DirectorError> =
            batch_download(self, desired_tracks, sources, config).await;

        match result {
            Ok(outcome) => {
                let _ = self
                    .log_event(
                        &op_id,
                        "director_batch_complete",
                        None,
                        None,
                        None,
                        None,
                        &serde_json::json!({
                            "successfully_downloaded": outcome.successfully_downloaded,
                            "failed": outcome.errors.len(),
                            "already_staged": outcome.already_staged,
                        }),
                    )
                    .await;

                self.complete_operation(
                    &op_id,
                    if outcome.errors.is_empty() {
                        OperationStatus::Success
                    } else {
                        OperationStatus::PartialSuccess {
                            completed: outcome.successfully_downloaded,
                            failed: outcome.errors.len(),
                        }
                    },
                )
                .await?;

                Ok(DirectorOutcome {
                    operation_id: op_id,
                    staged_files: outcome.successfully_downloaded,
                    failed_downloads: outcome.errors.len(),
                })
            }
            Err(error) => {
                self.complete_operation(&op_id, OperationStatus::FailedAt(error.to_string()))
                    .await?;
                Err(ManagerError::DownloadFailed(error.to_string()))
            }
        }
    }
}
