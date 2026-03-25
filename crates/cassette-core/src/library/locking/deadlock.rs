use crate::library::error::{ManagerError, Result};
use crate::library::manager::LibraryManager;
use crate::library::state::OperationStatus;
use crate::library::types::{DeadlockEdge, DeadlockReport};
use chrono::Utc;
use std::collections::{HashMap, HashSet};
use std::time::Duration;

pub struct DeadlockDetector;

impl LibraryManager {
    pub async fn detect_deadlock_risk(&self) -> Result<Option<DeadlockReport>> {
        let locks = self.file_locks.read().await;
        let operations = self.active_operations.read().await;

        let mut wait_graph: HashMap<String, Vec<String>> = HashMap::new();
        let mut edges = Vec::new();

        for (op_id, ctx) in operations.iter() {
            let Some(waiting_path) = &ctx.waiting_on_file else {
                continue;
            };

            if let Some(lock) = locks.get(waiting_path) {
                if &lock.operation_id != op_id {
                    wait_graph
                        .entry(op_id.clone())
                        .or_default()
                        .push(lock.operation_id.clone());
                    edges.push(DeadlockEdge {
                        waiting_operation_id: op_id.clone(),
                        blocking_operation_id: lock.operation_id.clone(),
                        file_path: waiting_path.clone(),
                    });
                }
            }
        }

        let cycle = find_cycle(&wait_graph);
        if let Some(cycle_operations) = cycle {
            return Ok(Some(DeadlockReport {
                detected_at: Utc::now(),
                cycle_operations,
                edges,
            }));
        }

        Ok(None)
    }

    pub async fn cleanup_stalled_operations(&self) -> Result<Vec<String>> {
        let max_duration = Duration::from_secs(self.config.stalled_operation_timeout_secs);
        let now = Utc::now();

        let op_ids: Vec<String> = {
            let active = self.active_operations.read().await;
            active
                .iter()
                .filter_map(|(op_id, ctx)| {
                    let elapsed = now.signed_duration_since(ctx.started_at);
                    let Ok(elapsed_std) = elapsed.to_std() else {
                        return None;
                    };
                    if elapsed_std > max_duration {
                        Some(op_id.clone())
                    } else {
                        None
                    }
                })
                .collect()
        };

        for op_id in &op_ids {
            tracing::warn!(operation_id = op_id, "Operation stalled; marking as failed");
            self.complete_operation(
                op_id,
                OperationStatus::FailedAt("Timeout: operation exceeded max duration".to_string()),
            )
            .await?;
        }

        Ok(op_ids)
    }

    pub async fn fail_on_deadlock_risk(&self) -> Result<()> {
        if let Some(report) = self.detect_deadlock_risk().await? {
            return Err(ManagerError::DeadlockDetected(format!(
                "cycle detected among operations: {}",
                report.cycle_operations.join(" -> ")
            )));
        }
        Ok(())
    }
}

fn find_cycle(graph: &HashMap<String, Vec<String>>) -> Option<Vec<String>> {
    let mut visited = HashSet::new();
    let mut stack = Vec::new();
    let mut in_stack = HashSet::new();

    for node in graph.keys() {
        if !visited.contains(node)
            && dfs_cycle(node, graph, &mut visited, &mut stack, &mut in_stack)
        {
            return Some(stack.clone());
        }
    }
    None
}

fn dfs_cycle(
    node: &str,
    graph: &HashMap<String, Vec<String>>,
    visited: &mut HashSet<String>,
    stack: &mut Vec<String>,
    in_stack: &mut HashSet<String>,
) -> bool {
    visited.insert(node.to_string());
    stack.push(node.to_string());
    in_stack.insert(node.to_string());

    if let Some(neighbors) = graph.get(node) {
        for neighbor in neighbors {
            if !visited.contains(neighbor)
                && dfs_cycle(neighbor, graph, visited, stack, in_stack)
            {
                return true;
            }
            if in_stack.contains(neighbor) {
                stack.push(neighbor.clone());
                return true;
            }
        }
    }

    stack.pop();
    in_stack.remove(node);
    false
}
