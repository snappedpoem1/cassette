use crate::director::config::DirectorConfig;
use crate::director::download::single::{download_file, ProviderSemaphores};
use crate::director::download::staging::check_existing_staged_file;
use crate::director::error::DirectorError;
use crate::director::sources::SourceProvider;
use crate::director::types::BatchDownloadOutcome;
use crate::library::{LibraryManager, Module, OperationStatus};
use crate::librarian::models::DesiredTrack;
use std::collections::HashMap;
use std::sync::Arc;

const DEFAULT_PROVIDER_CONCURRENCY: usize = 6;

pub async fn batch_download(
    manager: &LibraryManager,
    desired_tracks: &[DesiredTrack],
    sources: &[Arc<dyn SourceProvider>],
    config: &DirectorConfig,
) -> Result<BatchDownloadOutcome, DirectorError> {
    let operation_id = manager
        .start_operation(Module::Director, "batch_download")
        .await
        .map_err(|e| DirectorError::DatabaseError(e.to_string()))?;

    let mut to_download = Vec::new();
    let mut already_staged = Vec::new();
    for track in desired_tracks {
        if check_existing_staged_file(manager, track, config)
            .await
            .ok()
            .flatten()
            .is_some()
        {
            already_staged.push(track.clone());
        } else {
            to_download.push(track.clone());
        }
    }

    // Build per-provider semaphores from config (or default to 2).
    let mut provider_map: HashMap<String, Arc<tokio::sync::Semaphore>> = HashMap::new();
    for source in sources {
        let name = source.name().to_string();
        if !provider_map.contains_key(&name) {
            let limit = config
                .provider_policy(&name)
                .map(|p| p.max_concurrency.max(1))
                .unwrap_or(DEFAULT_PROVIDER_CONCURRENCY);
            provider_map.insert(name, Arc::new(tokio::sync::Semaphore::new(limit)));
        }
    }
    let provider_semaphores: ProviderSemaphores = Arc::new(provider_map);

    let semaphore = Arc::new(tokio::sync::Semaphore::new(
        config.max_concurrent_downloads.max(1),
    ));
    let mut tasks = Vec::new();
    for track in to_download {
        let semaphore = Arc::clone(&semaphore);
        let provider_semaphores = Arc::clone(&provider_semaphores);
        let manager = manager.clone();
        let sources: Vec<Arc<dyn SourceProvider>> = sources.to_vec();
        let config = config.clone();
        let op_id = operation_id.clone();
        tasks.push(tokio::spawn(async move {
            let _permit = semaphore.acquire().await.ok()?;
            Some((track.id as u64, download_file(&manager, &track, &sources, &config, &op_id, &provider_semaphores).await))
        }));
    }

    let mut outcome = BatchDownloadOutcome {
        operation_id: operation_id.clone(),
        total_requested: desired_tracks.len(),
        already_staged: already_staged.len(),
        successfully_downloaded: 0,
        failed_downloads: Vec::new(),
        errors: Vec::new(),
    };

    for task in tasks {
        match task.await {
            Ok(Some((_track_id, Ok(_staged)))) => outcome.successfully_downloaded += 1,
            Ok(Some((track_id, Err(error)))) => {
                outcome
                    .failed_downloads
                    .push((track_id, error.to_string()));
                outcome.errors.push(error.to_string());
            }
            Ok(None) => outcome.errors.push("semaphore acquisition failed".to_string()),
            Err(error) => outcome.errors.push(format!("Task panicked: {error}")),
        }
    }

    let status = if outcome.errors.is_empty() {
        OperationStatus::Success
    } else {
        OperationStatus::PartialSuccess {
            completed: outcome.successfully_downloaded,
            failed: outcome.errors.len(),
        }
    };

    let _ = manager
        .log_event(
            &operation_id,
            "batch_download_complete",
            None,
            None,
            None,
            None,
            &serde_json::json!({
                "already_staged": outcome.already_staged,
                "successfully_downloaded": outcome.successfully_downloaded,
                "failed": outcome.errors.len(),
            }),
        )
        .await;

    manager
        .complete_operation(&operation_id, status)
        .await
        .map_err(|e| DirectorError::DatabaseError(e.to_string()))?;

    Ok(outcome)
}
