use crate::library::tests::test_manager;
use crate::library::Module;
use chrono::{Duration, Utc};
use std::path::PathBuf;

#[tokio::test]
async fn detects_wait_cycle_risk() {
    let manager = test_manager().await.expect("manager");
    let op_a = manager
        .start_operation(Module::Gatekeeper, "a")
        .await
        .expect("op a");
    let op_b = manager
        .start_operation(Module::Custodian, "b")
        .await
        .expect("op b");

    let file_a = PathBuf::from("C:/tmp/deadlock_a.flac");
    let file_b = PathBuf::from("C:/tmp/deadlock_b.flac");

    let _g1 = manager
        .acquire_lock_for_file(&file_a, Module::Gatekeeper, &op_a, 500)
        .await
        .expect("lock a");
    let _g2 = manager
        .acquire_lock_for_file(&file_b, Module::Custodian, &op_b, 500)
        .await
        .expect("lock b");

    manager.mark_operation_waiting(&op_a, Some(file_b.clone())).await;
    manager.mark_operation_waiting(&op_b, Some(file_a.clone())).await;

    let report = manager.detect_deadlock_risk().await.expect("report");
    assert!(report.is_some());
}

#[tokio::test]
async fn cleanup_stalled_marks_failed() {
    let manager = test_manager().await.expect("manager");
    let op = manager
        .start_operation(Module::Librarian, "stall")
        .await
        .expect("op");

    {
        let mut active = manager.active_operations.write().await;
        if let Some(ctx) = active.get_mut(&op) {
            ctx.started_at = Utc::now() - Duration::seconds(7_200);
        }
    }

    let stalled = manager.cleanup_stalled_operations().await.expect("cleanup");
    assert_eq!(stalled.len(), 1);
}
