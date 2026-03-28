use crate::library::{LibraryManager, ManagerConfig};
use crate::sources::is_audio_path;
use crate::validation::error::{Result, ValidationError};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct TestLibrarySetup {
    pub source_library: Option<PathBuf>,
    pub test_library: PathBuf,
    pub test_staging: PathBuf,
    pub test_quarantine: PathBuf,
    pub test_db: PathBuf,
}

#[derive(Debug, Clone)]
pub struct TestLibraryConfig {
    pub source_library: Option<PathBuf>,
    pub test_library: PathBuf,
    pub test_staging: PathBuf,
    pub test_quarantine: PathBuf,
    pub test_db: PathBuf,
    pub copy_limit: usize,
}

impl Default for TestLibraryConfig {
    fn default() -> Self {
        Self {
            source_library: Some(PathBuf::from("A:\\music")),
            test_library: PathBuf::from("A:\\music_test"),
            test_staging: PathBuf::from("A:\\music_test_staging"),
            test_quarantine: PathBuf::from("A:\\music_test_quarantine"),
            test_db: PathBuf::from("cassette_test.db"),
            copy_limit: 1000,
        }
    }
}

impl TestLibrarySetup {
    pub async fn setup(config: &TestLibraryConfig) -> Result<Self> {
        tokio::fs::create_dir_all(&config.test_library).await?;
        tokio::fs::create_dir_all(&config.test_staging).await?;
        tokio::fs::create_dir_all(&config.test_quarantine).await?;

        if let Some(source_library) = &config.source_library {
            if source_library.exists() {
                println!(
                    "Preparing test library subset from {} (limit: {})...",
                    source_library.display(),
                    config.copy_limit
                );
                copy_library_subset(source_library, &config.test_library, config.copy_limit).await?;
            }
        }

        if let Some(parent) = config.test_db.parent() {
            if !parent.as_os_str().is_empty() {
                tokio::fs::create_dir_all(parent).await?;
            }
        }

        if !config.test_db.exists() {
            let _ = tokio::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .open(&config.test_db)
                .await?;
        }

        let db_url = sqlite_url_for_path(&config.test_db);
        let _manager = LibraryManager::connect(&db_url, ManagerConfig::default()).await?;

        Ok(Self {
            source_library: config.source_library.clone(),
            test_library: config.test_library.clone(),
            test_staging: config.test_staging.clone(),
            test_quarantine: config.test_quarantine.clone(),
            test_db: config.test_db.clone(),
        })
    }

    pub async fn cleanup(&self) -> Result<()> {
        remove_if_exists(&self.test_library).await?;
        remove_if_exists(&self.test_staging).await?;
        remove_if_exists(&self.test_quarantine).await?;

        if self.test_db.exists() {
            let mut removed = false;
            for _ in 0..10 {
                match tokio::fs::remove_file(&self.test_db).await {
                    Ok(_) => {
                        removed = true;
                        break;
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
                        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                    }
                    Err(error) => return Err(error.into()),
                }
            }

            if !removed && self.test_db.exists() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    format!("Failed to remove locked test database {}", self.test_db.display()),
                )
                .into());
            }
        }

        Ok(())
    }
}

pub async fn reset_validation_environment(config: &TestLibraryConfig) -> Result<()> {
    remove_if_exists(&config.test_library).await?;
    remove_if_exists(&config.test_staging).await?;
    remove_if_exists(&config.test_quarantine).await?;

    if config.test_db.exists() {
        tokio::fs::remove_file(&config.test_db).await?;
    }

    Ok(())
}

pub fn sqlite_url_for_path(path: &Path) -> String {
    format!("sqlite://{}", path.to_string_lossy())
}

async fn copy_library_subset(source: &Path, destination: &Path, limit: usize) -> Result<()> {
    let mut copied = 0usize;
    let mut linked = 0usize;

    for entry in WalkDir::new(source).follow_links(true).into_iter().filter_map(|e| e.ok()) {
        if copied >= limit {
            break;
        }

        if !entry.file_type().is_file() {
            continue;
        }

        let src_path = entry.path();
        if !is_audio_path(src_path) {
            continue;
        }

        let relative = src_path.strip_prefix(source).map_err(|error| {
            ValidationError::InvalidConfig(format!(
                "Failed to strip source prefix from {}: {error}",
                src_path.display()
            ))
        })?;

        let target = destination.join(relative);
        if let Some(parent) = target.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Hard-linking is much faster for local sandbox setup on the same volume.
        // Fall back to byte copy when links are not supported.
        match std::fs::hard_link(src_path, &target) {
            Ok(_) => {
                linked += 1;
            }
            Err(_) => {
                tokio::fs::copy(src_path, &target).await?;
            }
        }

        copied += 1;
        if copied % 500 == 0 {
            println!(
                "Test library prep progress: {copied}/{limit} files (hard-linked: {linked})"
            );
        }
    }

    println!(
        "Test library prep complete: {copied} files staged (hard-linked: {linked}, copied: {})",
        copied.saturating_sub(linked)
    );

    Ok(())
}


async fn remove_if_exists(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }

    let metadata = tokio::fs::metadata(path).await?;
    if metadata.is_dir() {
        tokio::fs::remove_dir_all(path).await?;
    } else {
        tokio::fs::remove_file(path).await?;
    }

    Ok(())
}
