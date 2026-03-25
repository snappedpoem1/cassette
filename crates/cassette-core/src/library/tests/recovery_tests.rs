use crate::library::tests::test_manager;
use crate::library::{Module, OperationStatus};

#[tokio::test]
async fn resume_only_allows_in_progress_operation() {
    let manager = test_manager().await.expect("manager");
    let op = manager
        .start_operation(Module::Director, "resume")
        .await
        .expect("start");

    manager.resume_operation(&op).await.expect("resume in progress");

    manager
        .complete_operation(&op, OperationStatus::Success)
        .await
        .expect("complete");

    let resumed = manager.resume_operation(&op).await;
    assert!(resumed.is_err());
}

#[tokio::test]
async fn rollback_marks_operation_rolled_back() {
    let manager = test_manager().await.expect("manager");
    let op = manager
        .start_operation(Module::Gatekeeper, "rollback")
        .await
        .expect("start");

    manager
        .log_event(
            &op,
            "noop",
            None,
            None,
            None,
            None,
            &serde_json::json!({"note": "test"}),
        )
        .await
        .expect("event");

    let report = manager.rollback_operation(&op).await.expect("rollback");
    assert_eq!(report.operation_id, op);

    let details = manager
        .get_operation_details(&report.operation_id)
        .await
        .expect("details");
    assert_eq!(details.operation.status, "rolled_back");
}
