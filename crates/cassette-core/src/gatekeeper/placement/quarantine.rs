use crate::gatekeeper::error::Result;
use crate::gatekeeper::mod_types::AdmissionDecision;
use std::path::{Path, PathBuf};
use tokio::fs;

pub async fn move_to_quarantine(
    source: &Path,
    quarantine_root: &Path,
    decision: AdmissionDecision,
) -> Result<PathBuf> {
    let bucket = match decision {
        AdmissionDecision::Quarantined { reason, .. } => reason.as_dir(),
        AdmissionDecision::Rejected { .. } => "rejected",
        AdmissionDecision::Admitted { .. } => "other",
    };

    let name = source
        .file_name()
        .and_then(|x| x.to_str())
        .unwrap_or("unknown.bin");

    let dest = quarantine_root.join(bucket).join(name);
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).await?;
    }

    fs::copy(source, &dest).await?;
    Ok(dest)
}
