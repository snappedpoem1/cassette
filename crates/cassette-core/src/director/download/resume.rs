use crate::director::config::DirectorConfig;
use crate::director::error::DirectorError;
use crate::director::resilience::backoff::director_backoff;
use crate::director::resilience::range_request::with_optional_range;
use backoff::backoff::Backoff;
use futures_util::StreamExt;
use std::path::Path;
use std::time::Duration;
use tokio::io::AsyncWriteExt;

pub async fn download_with_resume(
    url: &str,
    dest_path: &Path,
    config: &DirectorConfig,
    operation_id: &str,
) -> Result<(), DirectorError> {
    if let Some(local_path) = url.strip_prefix("file://") {
        return copy_local_file(local_path, dest_path, config).await;
    }

    let client = reqwest::Client::new();
    let mut backoff = director_backoff(config.max_download_time_secs);
    let mut attempts = 0u32;

    loop {
        attempts += 1;
        match download_attempt(&client, url, dest_path, config).await {
            Ok(_) => return Ok(()),
            Err(error) if is_retryable(&error) && attempts < config.retry_max_attempts => {
                let wait = backoff.next_backoff().ok_or(DirectorError::Timeout)?;
                tracing::warn!(
                    operation_id = operation_id,
                    attempt = attempts,
                    error = %error,
                    retry_after_ms = wait.as_millis(),
                    "Download failed; retrying"
                );
                tokio::time::sleep(wait).await;
            }
            Err(error) => return Err(error),
        }
    }
}

async fn copy_local_file(
    source: &str,
    dest_path: &Path,
    config: &DirectorConfig,
) -> Result<(), DirectorError> {
    let source_path = std::path::Path::new(source);
    let metadata = tokio::fs::metadata(source_path)
        .await
        .map_err(|e| DirectorError::StagingError(e.to_string()))?;
    if metadata.len() as usize > config.max_file_size_bytes {
        return Err(DirectorError::FileTooLarge {
            size: metadata.len() as usize,
            max: config.max_file_size_bytes,
        });
    }
    tokio::fs::copy(source_path, dest_path)
        .await
        .map_err(|e| DirectorError::StagingError(e.to_string()))?;
    Ok(())
}

pub async fn download_attempt(
    client: &reqwest::Client,
    url: &str,
    dest_path: &Path,
    config: &DirectorConfig,
) -> Result<(), DirectorError> {
    let existing_size = if dest_path.exists() {
        tokio::fs::metadata(dest_path)
            .await
            .ok()
            .map(|m| m.len())
            .unwrap_or(0)
    } else {
        0
    };

    let request = client
        .get(url)
        .timeout(Duration::from_secs(config.request_timeout_secs.max(1)));
    let request = with_optional_range(request, existing_size);

    let response = request.send().await.map_err(|e| {
        if e.is_timeout() {
            DirectorError::Timeout
        } else {
            DirectorError::NetworkError(e.to_string())
        }
    })?;

    if !response.status().is_success() {
        return Err(DirectorError::HttpError(response.status().as_u16()));
    }

    let append = existing_size > 0 && response.status() == reqwest::StatusCode::PARTIAL_CONTENT;
    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .append(append)
        .truncate(!append)
        .write(true)
        .open(dest_path)
        .await
        .map_err(|e| DirectorError::StagingError(e.to_string()))?;

    let mut stream = response.bytes_stream();
    let mut written = if append { existing_size as usize } else { 0usize };

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| DirectorError::NetworkError(e.to_string()))?;
        file.write_all(&chunk)
            .await
            .map_err(|e| DirectorError::StagingError(e.to_string()))?;

        written += chunk.len();
        if written > config.max_file_size_bytes {
            return Err(DirectorError::FileTooLarge {
                size: written,
                max: config.max_file_size_bytes,
            });
        }
    }

    file.flush()
        .await
        .map_err(|e| DirectorError::StagingError(e.to_string()))?;

    Ok(())
}

pub fn is_retryable(error: &DirectorError) -> bool {
    matches!(
        error,
        DirectorError::NetworkError(_)
            | DirectorError::Timeout
            | DirectorError::HttpError(408 | 429 | 500 | 502 | 503 | 504)
    )
}
