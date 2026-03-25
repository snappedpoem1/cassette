use crate::library::manager::LibraryManager;
use crate::library::state::Module;
use std::path::PathBuf;

pub struct FileLockGuard {
    pub(crate) manager: LibraryManager,
    pub(crate) file_path: PathBuf,
    pub(crate) module: Module,
}

impl FileLockGuard {
    pub fn file_path(&self) -> &std::path::Path {
        &self.file_path
    }

    pub fn module(&self) -> Module {
        self.module
    }
}

impl Drop for FileLockGuard {
    fn drop(&mut self) {
        let manager = self.manager.clone();
        let file_path = self.file_path.clone();

        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                if let Err(error) = manager.release_lock_for_file(&file_path).await {
                    tracing::warn!(file = %file_path.display(), error = %error, "failed to release lock during guard drop");
                }
            });
            return;
        }

        if let Ok(mut locks) = self.manager.file_locks.try_write() {
            locks.remove(&self.file_path);
        }
        tracing::warn!(
            file = %self.file_path.display(),
            "dropped lock without async runtime; DB lock row cleanup deferred to operation completion"
        );
    }
}
