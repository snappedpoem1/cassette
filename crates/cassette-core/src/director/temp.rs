use crate::director::config::TempRecoveryPolicy;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct TempManager {
    root: PathBuf,
    policy: TempRecoveryPolicy,
}

#[derive(Debug, Clone)]
pub struct TaskTempContext {
    pub task_id: String,
    pub root: PathBuf,
    pub active_dir: PathBuf,
    pub quarantine_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TempMarker {
    task_id: String,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TempRecoverySummary {
    pub deleted_roots: Vec<PathBuf>,
    pub preserved_quarantine: Vec<PathBuf>,
}

impl TempManager {
    pub fn new(root: PathBuf, policy: TempRecoveryPolicy) -> Self {
        Self { root, policy }
    }

    pub async fn prepare_task(&self, task_id: &str) -> Result<TaskTempContext, std::io::Error> {
        let root = self.root.join(task_id);
        let active_dir = root.join("active");
        let quarantine_dir = root.join("quarantine");
        tokio::fs::create_dir_all(&active_dir).await?;
        tokio::fs::create_dir_all(&quarantine_dir).await?;

        let marker = TempMarker {
            task_id: task_id.to_string(),
            created_at: Utc::now(),
        };
        let marker_json = serde_json::to_vec_pretty(&marker)
            .map_err(|error| std::io::Error::other(error.to_string()))?;
        tokio::fs::write(root.join("active.json"), marker_json).await?;

        Ok(TaskTempContext {
            task_id: task_id.to_string(),
            root,
            active_dir,
            quarantine_dir,
        })
    }

    pub async fn cleanup_task(&self, context: &TaskTempContext) -> Result<(), std::io::Error> {
        if context.root.exists() {
            tokio::fs::remove_dir_all(&context.root).await?;
        }
        Ok(())
    }

    pub async fn quarantine_file(
        &self,
        context: &TaskTempContext,
        source: &Path,
    ) -> Result<PathBuf, std::io::Error> {
        let filename = source
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("candidate.bin");
        let destination = context.quarantine_dir.join(filename);
        tokio::fs::rename(source, &destination).await?;
        Ok(destination)
    }

    pub async fn recover_stale(&self) -> Result<TempRecoverySummary, std::io::Error> {
        tokio::fs::create_dir_all(&self.root).await?;
        let mut deleted_roots = Vec::<PathBuf>::new();
        let mut preserved_quarantine = Vec::<PathBuf>::new();
        let cutoff = Utc::now() - Duration::hours(self.policy.stale_after_hours as i64);

        let mut entries = tokio::fs::read_dir(&self.root).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let marker_path = path.join("active.json");
            let marker = match tokio::fs::read(&marker_path).await {
                Ok(bytes) => serde_json::from_slice::<TempMarker>(&bytes).ok(),
                Err(_) => None,
            };
            let is_stale = marker.as_ref().map(|value| value.created_at < cutoff).unwrap_or(true);
            if !is_stale {
                continue;
            }

            if self.policy.quarantine_failures && path.join("quarantine").exists() {
                preserved_quarantine.push(path.join("quarantine"));
            } else {
                tokio::fs::remove_dir_all(&path).await?;
                deleted_roots.push(path);
            }
        }

        Ok(TempRecoverySummary {
            deleted_roots,
            preserved_quarantine,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn stale_recovery_deletes_old_task_roots() {
        let dir = tempdir().expect("temp dir");
        let manager = TempManager::new(
            dir.path().to_path_buf(),
            TempRecoveryPolicy {
                stale_after_hours: 0,
                quarantine_failures: false,
            },
        );
        let context = manager.prepare_task("task-1").await.expect("prepare task");
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;

        let summary = manager.recover_stale().await.expect("recover stale");
        assert!(summary.deleted_roots.contains(&context.root));
    }
}
