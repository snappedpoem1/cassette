mod deadlock_tests;
mod integration_tests;
mod locking_tests;
mod operations_tests;
mod recovery_tests;
mod transaction_tests;

use crate::library::{LibraryManager, ManagerConfig, ManagerResult};
use tempfile::NamedTempFile;

pub async fn test_manager() -> ManagerResult<LibraryManager> {
    let file = NamedTempFile::new().expect("temp db file");
    let url = format!("sqlite://{}", file.path().to_string_lossy());
    let manager = LibraryManager::connect(&url, ManagerConfig::default()).await?;
    Ok(manager)
}
