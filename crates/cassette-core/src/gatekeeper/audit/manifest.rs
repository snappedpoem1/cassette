use crate::gatekeeper::error::Result;
use crate::gatekeeper::mod_types::BatchIngestOutcome;
use chrono::Utc;
use std::path::{Path, PathBuf};

pub async fn write_manifest(base_dir: &Path, summary: &BatchIngestOutcome) -> Result<PathBuf> {
    tokio::fs::create_dir_all(base_dir).await?;
    let path = base_dir.join(format!("gatekeeper_manifest_{}.json", Utc::now().format("%Y%m%d_%H%M%S")));
    let payload = serde_json::to_vec_pretty(summary)?;
    tokio::fs::write(&path, payload).await?;
    Ok(path)
}
