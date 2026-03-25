use crate::library::error::{ManagerError, Result};
use crate::library::locking::file_lock::FileLockGuard;
use crate::library::manager::LibraryManager;
use crate::library::state::{FileLock, Module};
use chrono::{Duration as ChronoDuration, Utc};
use std::path::Path;
use std::time::{Duration, Instant};

impl LibraryManager {
    pub async fn acquire_lock_for_file(
        &self,
        file_path: &Path,
        module: Module,
        operation_id: &str,
        timeout_ms: u64,
    ) -> Result<FileLockGuard> {
        let timeout = Duration::from_millis(timeout_ms.min(self.config.max_lock_timeout_ms));
        let start = Instant::now();

        loop {
            let existing = {
                let locks = self.file_locks.read().await;
                locks.get(file_path).cloned()
            };

            if let Some(existing_lock) = existing {
                self.mark_operation_waiting(operation_id, Some(file_path.to_path_buf()))
                    .await;
                if start.elapsed() > timeout {
                    return Err(ManagerError::LockAcquisitionTimeout {
                        file: file_path.to_path_buf(),
                        locked_by: existing_lock.locked_by,
                        waited_ms: timeout_ms,
                    });
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
                continue;
            }

            {
                let mut locks = self.file_locks.write().await;
                let lock = FileLock {
                    file_path: file_path.to_path_buf(),
                    locked_by: module,
                    acquired_at: Utc::now(),
                    operation_id: operation_id.to_string(),
                };
                locks.insert(file_path.to_path_buf(), lock);
            }

            if let Err(error) = self
                .persist_lock_to_db(file_path, module, operation_id, timeout_ms)
                .await
            {
                let mut locks = self.file_locks.write().await;
                locks.remove(file_path);
                return Err(error);
            }

            self.mark_operation_waiting(operation_id, None).await;
            self.track_operation_file(operation_id, file_path).await;

            tracing::info!(
                operation_id = operation_id,
                module = %module,
                file = %file_path.display(),
                "File lock acquired"
            );

            return Ok(FileLockGuard {
                manager: self.clone(),
                file_path: file_path.to_path_buf(),
                module,
            });
        }
    }

    pub async fn release_lock_for_file(&self, file_path: &Path) -> Result<()> {
        {
            let mut locks = self.file_locks.write().await;
            locks.remove(file_path);
        }

        sqlx::query("DELETE FROM file_locks WHERE file_path = ?1")
            .bind(file_path.to_string_lossy().to_string())
            .execute(&self.db_pool)
            .await?;

        tracing::debug!(file = %file_path.display(), "File lock released");
        Ok(())
    }

    pub async fn is_file_locked(&self, file_path: &Path) -> bool {
        self.file_locks.read().await.contains_key(file_path)
    }

    pub async fn get_active_locks(&self) -> Vec<FileLock> {
        self.file_locks.read().await.values().cloned().collect()
    }

    pub(crate) async fn persist_lock_to_db(
        &self,
        file_path: &Path,
        module: Module,
        operation_id: &str,
        timeout_ms: u64,
    ) -> Result<()> {
        let acquired_at = Utc::now();
        let timeout_at = acquired_at + ChronoDuration::milliseconds(timeout_ms as i64);

        sqlx::query(
            r#"
            INSERT INTO file_locks (file_path, locked_by, operation_id, acquired_at, timeout_at)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(file_path) DO UPDATE SET
              locked_by = excluded.locked_by,
              operation_id = excluded.operation_id,
              acquired_at = excluded.acquired_at,
              timeout_at = excluded.timeout_at
            "#,
        )
        .bind(file_path.to_string_lossy().to_string())
        .bind(module.as_str())
        .bind(operation_id)
        .bind(acquired_at.to_rfc3339())
        .bind(timeout_at.to_rfc3339())
        .execute(&self.db_pool)
        .await?;

        Ok(())
    }
}
