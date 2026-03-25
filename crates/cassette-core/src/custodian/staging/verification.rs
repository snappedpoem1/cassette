use crate::custodian::error::{CustodianError, Result};

pub async fn compute_hash(path: &std::path::Path) -> Result<String> {
    let bytes = tokio::fs::read(path).await?;
    Ok(blake3::hash(&bytes).to_hex().to_string())
}

pub async fn verify_identical(source: &std::path::Path, copied: &std::path::Path) -> Result<()> {
    let source_hash = compute_hash(source).await?;
    let copied_hash = compute_hash(copied).await?;
    if source_hash != copied_hash {
        return Err(CustodianError::StagingError(
            "copy verification failed: checksum mismatch".to_string(),
        ));
    }
    Ok(())
}
