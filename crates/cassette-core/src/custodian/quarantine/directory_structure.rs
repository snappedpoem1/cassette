use crate::custodian::quarantine::reason_codes::QuarantineReason;
use crate::custodian::sort::sanitize::sanitize_component;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub fn build_quarantine_path(root: &Path, reason: QuarantineReason, source_path: &Path) -> PathBuf {
    let file_name = source_path
        .file_name()
        .and_then(|v| v.to_str())
        .map(sanitize_component)
        .unwrap_or_else(|| "unknown.bin".to_string());

    let unique = format!("{}-{}", Uuid::new_v4(), file_name);
    root.join(reason.as_dir()).join(unique)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quarantine_path_uses_reason_directory() {
        let path = build_quarantine_path(
            Path::new("A:/music_quarantine"),
            QuarantineReason::DecodeFailed,
            Path::new("A:/music/bad.flac"),
        );
        let text = path.to_string_lossy();
        assert!(text.contains("decode_failed"));
        assert!(text.contains("bad.flac"));
    }
}
