use cassette_core::db::{PendingDirectorTask, TerminalDirectorTaskUpdate};
use std::collections::HashMap;

pub struct PendingRecoveryPlan {
    pub resumable_tasks: Vec<PendingDirectorTask>,
    pub stale_task_ids: Vec<String>,
}

pub fn build_pending_recovery_plan(
    pending: Vec<PendingDirectorTask>,
    terminal_updates: &HashMap<String, TerminalDirectorTaskUpdate>,
) -> PendingRecoveryPlan {
    let mut resumable_tasks = Vec::new();
    let mut stale_task_ids = Vec::new();

    for pending_task in pending {
        let is_terminal_progress = matches!(
            pending_task.progress.as_str(),
            "Finalized" | "Cancelled" | "Failed" | "Exhausted" | "Skipped"
        );
        let has_newer_terminal_result = terminal_updates
            .get(&pending_task.task.task_id)
            .map(|update| update.updated_at >= pending_task.updated_at)
            .unwrap_or(false);

        if is_terminal_progress || has_newer_terminal_result {
            stale_task_ids.push(pending_task.task.task_id);
            continue;
        }

        resumable_tasks.push(pending_task);
    }

    PendingRecoveryPlan {
        resumable_tasks,
        stale_task_ids,
    }
}
