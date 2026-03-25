use crate::custodian::custody_log::manifest::CustodianManifest;
use crate::custodian::error::Result;
use std::path::{Path, PathBuf};

pub async fn persist_manifest(root: &Path, manifest: &CustodianManifest) -> Result<PathBuf> {
    tokio::fs::create_dir_all(root).await?;

    let filename = format!(
        "custodian-run-{}-{}.json",
        chrono::Utc::now().format("%Y%m%dT%H%M%SZ"),
        manifest.operation_id
    );
    let path = root.join(filename);
    let payload = serde_json::to_string_pretty(manifest)?;
    tokio::fs::write(&path, payload).await?;
    Ok(path)
}
