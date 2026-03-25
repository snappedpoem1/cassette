use crate::gatekeeper::error::Result;
use crate::gatekeeper::mod_types::IngressOutcome;
use std::path::Path;
use tokio::io::AsyncWriteExt;

pub async fn append_jsonl(outcome: &IngressOutcome, file_path: &Path) -> Result<()> {
    if let Some(parent) = file_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(file_path)
        .await?;

    let line = serde_json::to_string(outcome)?;
    file.write_all(line.as_bytes()).await?;
    file.write_all(b"\n").await?;
    Ok(())
}
