use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

use crate::director::types::HashAlgorithm;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectorConfig {
    pub library_root: PathBuf,
    pub temp_root: PathBuf,
    pub local_search_roots: Vec<PathBuf>,
    pub worker_concurrency: usize,
    pub provider_timeout_secs: u64,
    pub retry_policy: RetryPolicy,
    pub quality_policy: QualityPolicy,
    pub duplicate_policy: DuplicatePolicy,
    pub temp_recovery: TempRecoveryPolicy,
    pub provider_policies: Vec<ProviderPolicy>,
    pub staging_root: PathBuf,
    pub max_file_size_bytes: usize,
    pub max_concurrent_downloads: usize,
    pub lock_timeout_ms: u64,
    pub request_timeout_secs: u64,
    pub max_download_time_secs: u64,
    pub retry_max_attempts: u32,
    pub enabled_sources: Vec<String>,
    pub source_priority: Vec<String>,
    pub verify_after_download: bool,
    pub verify_hash_algorithm: HashAlgorithm,
    pub log_downloads: bool,
    pub tracing_level: String,
}

impl DirectorConfig {
    pub fn provider_timeout(&self) -> Duration {
        Duration::from_secs(self.provider_timeout_secs.max(1))
    }

    pub fn provider_policy(&self, provider_id: &str) -> Option<&ProviderPolicy> {
        self.provider_policies
            .iter()
            .find(|policy| policy.provider_id == provider_id)
    }
}

impl Default for DirectorConfig {
    fn default() -> Self {
        Self {
            library_root: PathBuf::from("Library"),
            temp_root: PathBuf::from("staging/director"),
            local_search_roots: Vec::new(),
            worker_concurrency: 12,
            provider_timeout_secs: 45,
            retry_policy: RetryPolicy::default(),
            quality_policy: QualityPolicy::default(),
            duplicate_policy: DuplicatePolicy::KeepExisting,
            temp_recovery: TempRecoveryPolicy::default(),
            provider_policies: Vec::new(),
            staging_root: PathBuf::from("staging/gatekeeper"),
            max_file_size_bytes: 2 * 1024 * 1024 * 1024,
            max_concurrent_downloads: 16,
            lock_timeout_ms: 600_000,
            request_timeout_secs: 120,
            max_download_time_secs: 3_600,
            retry_max_attempts: 5,
            enabled_sources: vec!["http".to_string(), "local_cache".to_string()],
            source_priority: vec!["local_cache".to_string(), "http".to_string()],
            verify_after_download: true,
            verify_hash_algorithm: HashAlgorithm::Blake3,
            log_downloads: true,
            tracing_level: "info".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    pub max_attempts_per_provider: u32,
    pub base_backoff_millis: u64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts_per_provider: 2,
            base_backoff_millis: 500,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityPolicy {
    pub minimum_duration_secs: f64,
    pub max_duration_delta_secs: Option<f64>,
    pub preferred_extensions: Vec<String>,
}

impl Default for QualityPolicy {
    fn default() -> Self {
        Self {
            minimum_duration_secs: 30.0,
            max_duration_delta_secs: Some(8.0),
            preferred_extensions: vec!["flac".to_string(), "wav".to_string(), "m4a".to_string()],
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum DuplicatePolicy {
    KeepExisting,
    ReplaceIfBetter,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TempRecoveryPolicy {
    pub stale_after_hours: u64,
    pub quarantine_failures: bool,
}

impl Default for TempRecoveryPolicy {
    fn default() -> Self {
        Self {
            stale_after_hours: 24,
            quarantine_failures: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderPolicy {
    pub provider_id: String,
    pub max_concurrency: usize,
}
