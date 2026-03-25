use crate::custodian::collision::policies::{CollisionPolicy, DuplicatePolicy};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustodianConfig {
    pub source_roots: Vec<PathBuf>,
    pub sorted_target: PathBuf,
    pub staging_root: PathBuf,
    pub quarantine_root: PathBuf,
    pub dry_run: bool,
    pub verify_copy: bool,
    pub cross_volume_copy: bool,
    pub same_volume_move: bool,
    pub delete_source_after_verify: bool,
    pub duplicate_policy: DuplicatePolicy,
    pub collision_policy: CollisionPolicy,
    pub suspicious_size_tolerance: f64,
    pub allowed_formats: Vec<String>,
    pub logging_level: String,
    pub manifest_dir: PathBuf,
}

impl Default for CustodianConfig {
    fn default() -> Self {
        Self {
            source_roots: vec![PathBuf::from("A:\\music")],
            sorted_target: PathBuf::from("A:\\music_sorted"),
            staging_root: PathBuf::from("A:\\music_staging"),
            quarantine_root: PathBuf::from("A:\\music_quarantine"),
            dry_run: true,
            verify_copy: true,
            cross_volume_copy: true,
            same_volume_move: false,
            delete_source_after_verify: false,
            duplicate_policy: DuplicatePolicy::ManualReview,
            collision_policy: CollisionPolicy::RenameIncoming,
            suspicious_size_tolerance: 1.5,
            allowed_formats: vec![
                "flac".to_string(),
                "mp3".to_string(),
                "m4a".to_string(),
                "aac".to_string(),
                "ogg".to_string(),
                "opus".to_string(),
                "wav".to_string(),
                "aiff".to_string(),
            ],
            logging_level: "info".to_string(),
            manifest_dir: PathBuf::from("A:\\music_admin\\custodian_runs"),
        }
    }
}
