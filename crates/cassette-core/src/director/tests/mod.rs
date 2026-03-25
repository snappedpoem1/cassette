mod batch_tests;
mod download_tests;
mod resilience_tests;
mod source_tests;

use crate::director::config::DirectorConfig;
use crate::library::{LibraryManager, ManagerConfig};
use tempfile::NamedTempFile;

async fn test_manager() -> LibraryManager {
    let file = NamedTempFile::new().expect("temp db file");
    let url = format!("sqlite://{}", file.path().to_string_lossy());
    LibraryManager::connect(&url, ManagerConfig::default())
        .await
        .expect("manager")
}

fn test_director_config(staging_root: std::path::PathBuf) -> DirectorConfig {
    DirectorConfig {
        staging_root,
        request_timeout_secs: 5,
        retry_max_attempts: 2,
        max_download_time_secs: 10,
        max_concurrent_downloads: 2,
        ..DirectorConfig::default()
    }
}
