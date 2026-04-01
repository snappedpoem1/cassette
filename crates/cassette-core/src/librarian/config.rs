use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ScanMode {
    Full,
    Resume,
    DeltaOnly,
}

impl ScanMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Resume => "resume",
            Self::DeltaOnly => "delta-only",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DuplicatePolicy {
    Ignore,
    Flag,
    PreferLossless,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityConfig {
    pub preferred_lossy_floor_kbps: u32,
    pub absolute_lossy_floor_kbps: u32,
}

impl Default for QualityConfig {
    fn default() -> Self {
        Self {
            preferred_lossy_floor_kbps: 320,
            absolute_lossy_floor_kbps: 128,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanBehavior {
    pub follow_symlinks: bool,
    pub ignore_hidden_files: bool,
}

impl Default for ScanBehavior {
    fn default() -> Self {
        Self {
            follow_symlinks: false,
            ignore_hidden_files: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibrarianConfig {
    pub library_roots: Vec<PathBuf>,
    pub sqlite_path: PathBuf,
    pub desired_state_path: Option<PathBuf>,
    pub enable_content_hashing: bool,
    pub skip_scan: bool,
    pub duplicate_policy: DuplicatePolicy,
    pub quality: QualityConfig,
    pub scan_behavior: ScanBehavior,
    pub scan_mode: ScanMode,
    pub tracing_filter: String,
}

impl Default for LibrarianConfig {
    fn default() -> Self {
        Self {
            library_roots: vec![PathBuf::from("A:\\music")],
            sqlite_path: PathBuf::from("staging/librarian.sqlite"),
            desired_state_path: None,
            enable_content_hashing: true,
            skip_scan: false,
            duplicate_policy: DuplicatePolicy::Flag,
            quality: QualityConfig::default(),
            scan_behavior: ScanBehavior::default(),
            scan_mode: ScanMode::Full,
            tracing_filter: "info,cassette_core::librarian=debug".to_string(),
        }
    }
}
