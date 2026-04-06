use crate::custodian::error::{CustodianError, Result};
use crate::custodian::staging::verification::verify_identical;
use std::path::{Path, PathBuf};

pub async fn copy_to_staging(staging_root: &Path, source: &Path) -> Result<PathBuf> {
    tokio::fs::create_dir_all(staging_root).await?;

    let file_name = source
        .file_name()
        .and_then(|v| v.to_str())
        .ok_or_else(|| CustodianError::StagingError("invalid source filename".to_string()))?;

    let staged = staging_root.join(format!("{}-{}", uuid::Uuid::new_v4(), file_name));
    tokio::fs::copy(source, &staged).await?;
    Ok(staged)
}

pub async fn staged_copy_verify(
    source: &Path,
    staging_root: &Path,
    destination: &Path,
    verify_copy: bool,
    delete_source_after_verify: bool,
    same_volume_move: bool,
) -> Result<()> {
    if let Some(parent) = destination.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    if same_volume_move {
        match tokio::fs::rename(source, destination).await {
            Ok(()) => return Ok(()),
            Err(_) => {
                // Fall back to copy-based transfer when an in-place move is not possible.
            }
        }
    }

    let staged = copy_to_staging(staging_root, source).await?;
    if verify_copy {
        verify_identical(source, &staged).await?;
    }

    tokio::fs::rename(&staged, destination).await?;

    if verify_copy {
        verify_identical(source, destination).await?;
    }

    if delete_source_after_verify {
        tokio::fs::remove_file(source).await?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn copy_verify_delete_keeps_source_when_configured() {
        let dir = tempfile::tempdir().expect("tempdir");
        let source = dir.path().join("source.mp3");
        let staging = dir.path().join("staging");
        let destination = dir.path().join("sorted").join("dest.mp3");

        tokio::fs::write(&source, b"audio-bytes")
            .await
            .expect("write");
        staged_copy_verify(&source, &staging, &destination, true, false, false)
            .await
            .expect("stage copy");

        assert!(source.exists());
        assert!(destination.exists());
    }
}
