use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum HashAlgorithm {
    Blake3,
    Sha256,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StagedFile {
    pub path: PathBuf,
    pub file_size: u64,
    pub content_hash: String,
    pub codec: Option<String>,
    pub bitrate: Option<u32>,
    pub source: String,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchDownloadOutcome {
    pub operation_id: String,
    pub total_requested: usize,
    pub already_staged: usize,
    pub successfully_downloaded: usize,
    pub failed_downloads: Vec<(u64, String)>,
    pub errors: Vec<String>,
}
