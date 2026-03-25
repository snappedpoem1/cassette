use crate::library::manager::LibraryManager;
use std::path::{Path, PathBuf};

impl LibraryManager {
    pub async fn track_operation_file(&self, operation_id: &str, file_path: &Path) {
        let mut active = self.active_operations.write().await;
        if let Some(ctx) = active.get_mut(operation_id) {
            if !ctx.affected_files.iter().any(|p| p == file_path) {
                ctx.affected_files.push(file_path.to_path_buf());
            }
        }
    }

    pub async fn track_operation_track_id(&self, operation_id: &str, track_id: u64) {
        let mut active = self.active_operations.write().await;
        if let Some(ctx) = active.get_mut(operation_id) {
            if !ctx.affected_tracks.contains(&track_id) {
                ctx.affected_tracks.push(track_id);
            }
        }
    }

    pub async fn mark_operation_waiting(&self, operation_id: &str, file_path: Option<PathBuf>) {
        let mut active = self.active_operations.write().await;
        if let Some(ctx) = active.get_mut(operation_id) {
            ctx.waiting_on_file = file_path;
        }
    }
}
