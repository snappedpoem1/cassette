use crate::director::config::DirectorConfig;
use crate::director::download::resume::download_with_resume;
use crate::director::download::staging::{check_existing_staged_file, compute_staging_path};
use crate::director::error::DirectorError;
use crate::director::sources::SourceProvider;
use crate::director::types::{HashAlgorithm, StagedFile};
use crate::librarian::models::DesiredTrack;
use crate::library::{LibraryManager, Module};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

/// Map of provider name -> per-provider concurrency semaphore.
pub type ProviderSemaphores = Arc<HashMap<String, Arc<tokio::sync::Semaphore>>>;

pub async fn download_file(
    manager: &LibraryManager,
    desired_track: &DesiredTrack,
    sources: &[Arc<dyn SourceProvider>],
    config: &DirectorConfig,
    operation_id: &str,
    provider_semaphores: &ProviderSemaphores,
) -> Result<StagedFile, DirectorError> {
    if let Some(existing) = check_existing_staged_file(manager, desired_track, config).await? {
        tracing::info!(
            operation_id = operation_id,
            track_id = desired_track.id,
            "File already staged; skipping download"
        );
        return Ok(existing);
    }

    tokio::fs::create_dir_all(&config.staging_root)
        .await
        .map_err(|e| DirectorError::StagingError(e.to_string()))?;

    let staging_path = compute_staging_path(&config.staging_root, desired_track);
    let _lock = manager
        .acquire_lock_for_file(
            &staging_path,
            Module::Director,
            operation_id,
            config.lock_timeout_ms,
        )
        .await
        .map_err(|e| DirectorError::LockError(e.to_string()))?;

    let mut last_error: Option<DirectorError> = None;
    let mut attempted_sources: Vec<String> = Vec::new();
    let mut source_errors: Vec<serde_json::Value> = Vec::new();

    for source in sources {
        if !source.can_handle(desired_track) {
            continue;
        }

        // Acquire per-provider permit so we don't overwhelm any single provider.
        let _provider_permit = if let Some(sem) = provider_semaphores.get(source.name()) {
            Some(
                sem.acquire()
                    .await
                    .map_err(|_| DirectorError::SourceError {
                        provider: source.name().to_string(),
                        error: "provider semaphore closed".to_string(),
                    })?,
            )
        } else {
            None
        };

        attempted_sources.push(source.name().to_string());

        match download_from_source(
            manager,
            desired_track,
            source.as_ref(),
            &staging_path,
            config,
            operation_id,
        )
        .await
        {
            Ok(staged_file) => {
                let _ = manager
                    .log_event(
                        operation_id,
                        "download_complete",
                        None,
                        None,
                        None,
                        None,
                        &serde_json::json!({
                            "desired_track_id": desired_track.id,
                            "source": source.name(),
                            "sources_tried": attempted_sources,
                            "file_size": staged_file.file_size,
                            "staging_path": staged_file.path.display().to_string(),
                            "content_hash": staged_file.content_hash,
                            "codec": staged_file.codec,
                            "bitrate": staged_file.bitrate,
                        }),
                    )
                    .await;
                return Ok(staged_file);
            }
            Err(error) => {
                tracing::warn!(
                    operation_id = operation_id,
                    track_id = desired_track.id,
                    source = source.name(),
                    error = %error,
                    "Download from source failed; trying next"
                );
                source_errors.push(serde_json::json!({
                    "source": source.name(),
                    "error": error.to_string(),
                }));
                last_error = Some(error);
            }
        }
    }

    let error = last_error.unwrap_or_else(|| {
        DirectorError::NoAvailableSources(format!(
            "No sources for {} - {}",
            desired_track.artist_name, desired_track.track_title
        ))
    });

    let _ = manager
        .log_event(
            operation_id,
            "download_failed",
            None,
            None,
            None,
            None,
            &serde_json::json!({
                "desired_track_id": desired_track.id,
                "error": error.to_string(),
                "sources_attempted": attempted_sources,
                "source_errors": source_errors,
                "tried_all_sources": true,
            }),
        )
        .await;

    Err(error)
}

async fn download_from_source(
    _manager: &LibraryManager,
    desired_track: &DesiredTrack,
    source: &dyn SourceProvider,
    staging_path: &Path,
    config: &DirectorConfig,
    operation_id: &str,
) -> Result<StagedFile, DirectorError> {
    let resolved = source
        .resolve_download_url(desired_track)
        .await
        .map_err(|e| DirectorError::SourceError {
            provider: source.name().to_string(),
            error: e.to_string(),
        })?;

    let temp_path = staging_path.with_extension("tmp");
    download_with_resume(&resolved.download_url, &temp_path, config, operation_id).await?;

    let content_hash = compute_hash(&temp_path, config.verify_hash_algorithm).await?;
    let final_staging_path = resolved
        .expected_codec
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(|codec| staging_path.with_extension(codec))
        .unwrap_or_else(|| staging_path.to_path_buf());

    if final_staging_path.exists() {
        tokio::fs::remove_file(&final_staging_path)
            .await
            .map_err(|e| DirectorError::StagingError(e.to_string()))?;
    }

    tokio::fs::rename(&temp_path, &final_staging_path)
        .await
        .map_err(|e| DirectorError::StagingError(e.to_string()))?;

    let metadata = tokio::fs::metadata(&final_staging_path)
        .await
        .map_err(|e| DirectorError::StagingError(e.to_string()))?;

    Ok(StagedFile {
        path: final_staging_path,
        file_size: metadata.len(),
        content_hash,
        codec: resolved.expected_codec,
        bitrate: resolved.expected_bitrate,
        source: source.name().to_string(),
        metadata: resolved.metadata,
    })
}

async fn compute_hash(path: &Path, algorithm: HashAlgorithm) -> Result<String, DirectorError> {
    let bytes = tokio::fs::read(path)
        .await
        .map_err(|e| DirectorError::VerificationError(e.to_string()))?;

    let hash = match algorithm {
        HashAlgorithm::Blake3 => blake3::hash(&bytes).to_hex().to_string(),
        HashAlgorithm::Sha256 => {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(&bytes);
            format!("{:x}", hasher.finalize())
        }
    };
    Ok(hash)
}
