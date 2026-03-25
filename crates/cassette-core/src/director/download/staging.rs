use crate::director::config::DirectorConfig;
use crate::director::error::DirectorError;
use crate::director::types::StagedFile;
use crate::library::LibraryManager;
use crate::librarian::models::DesiredTrack;
use std::path::{Path, PathBuf};

pub fn compute_staging_path(staging_root: &Path, desired_track: &DesiredTrack) -> PathBuf {
    let filename = format!(
        "{}-{}-{}.bin",
        desired_track.id,
        sanitize_component(&desired_track.artist_name),
        sanitize_component(&desired_track.track_title)
    );
    staging_root.join(filename)
}

pub async fn list_staged_files(staging_root: &Path) -> Result<Vec<StagedFile>, DirectorError> {
    let mut entries = tokio::fs::read_dir(staging_root)
        .await
        .map_err(|e| DirectorError::StagingError(e.to_string()))?;
    let mut files = Vec::new();

    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|e| DirectorError::StagingError(e.to_string()))?
    {
        let path = entry.path();
        if path.is_file() && !path.to_string_lossy().ends_with(".tmp") {
            let metadata = entry
                .metadata()
                .await
                .map_err(|e| DirectorError::StagingError(e.to_string()))?;
            let bytes = tokio::fs::read(&path)
                .await
                .map_err(|e| DirectorError::StagingError(e.to_string()))?;
            let content_hash = blake3::hash(&bytes).to_hex().to_string();
            files.push(StagedFile {
                path,
                file_size: metadata.len(),
                content_hash,
                codec: None,
                bitrate: None,
                source: "unknown".to_string(),
                metadata: serde_json::json!({}),
            });
        }
    }

    Ok(files)
}

pub async fn check_existing_staged_file(
    manager: &LibraryManager,
    desired_track: &DesiredTrack,
    config: &DirectorConfig,
) -> Result<Option<StagedFile>, DirectorError> {
    let path = compute_staging_path(&config.staging_root, desired_track);
    if path.exists() {
        let metadata = tokio::fs::metadata(&path)
            .await
            .map_err(|e| DirectorError::StagingError(e.to_string()))?;
        let bytes = tokio::fs::read(&path)
            .await
            .map_err(|e| DirectorError::StagingError(e.to_string()))?;
        let hash = blake3::hash(&bytes).to_hex().to_string();
        return Ok(Some(StagedFile {
            path,
            file_size: metadata.len(),
            content_hash: hash,
            codec: None,
            bitrate: None,
            source: "staging_cache".to_string(),
            metadata: serde_json::json!({ "source": "filesystem" }),
        }));
    }

        let row = sqlx::query_scalar::<_, String>(
        r#"
        SELECT event_data
        FROM operation_events
        WHERE event_type = 'download_complete'
                    AND (
                                target_track_id = ?1
                                OR json_extract(event_data, '$.desired_track_id') = ?1
                            )
          AND timestamp > datetime('now', '-24 hours')
        ORDER BY event_id DESC
        LIMIT 1
        "#,
    )
    .bind(desired_track.id)
        .fetch_optional(manager.db_pool())
    .await
    .map_err(|e| DirectorError::DatabaseError(e.to_string()))?;

        let Some(event_data) = row else {
        return Ok(None);
    };

    let value: serde_json::Value = serde_json::from_str(&event_data)
        .map_err(|e| DirectorError::DatabaseError(e.to_string()))?;
    let Some(staging_path) = value.get("staging_path").and_then(|v| v.as_str()) else {
        return Ok(None);
    };

    let path = PathBuf::from(staging_path);
    if !path.exists() {
        return Ok(None);
    }

    let metadata = tokio::fs::metadata(&path)
        .await
        .map_err(|e| DirectorError::StagingError(e.to_string()))?;
    Ok(Some(StagedFile {
        path,
        file_size: metadata.len(),
        content_hash: value
            .get("content_hash")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string(),
        codec: value.get("codec").and_then(|v| v.as_str()).map(ToString::to_string),
        bitrate: value.get("bitrate").and_then(|v| v.as_u64()).map(|v| v as u32),
        source: value
            .get("source")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string(),
        metadata: value,
    }))
}

pub async fn cleanup_orphan_temporary_files(staging_root: &Path) -> Result<usize, DirectorError> {
    let mut entries = tokio::fs::read_dir(staging_root)
        .await
        .map_err(|e| DirectorError::StagingError(e.to_string()))?;
    let mut removed = 0usize;
    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|e| DirectorError::StagingError(e.to_string()))?
    {
        let path = entry.path();
        if path.is_file() && path.to_string_lossy().ends_with(".tmp") {
            tokio::fs::remove_file(&path)
                .await
                .map_err(|e| DirectorError::StagingError(e.to_string()))?;
            removed += 1;
        }
    }
    Ok(removed)
}

fn sanitize_component(value: &str) -> String {
    value
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string()
}
