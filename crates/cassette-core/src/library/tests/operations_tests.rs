use crate::library::tests::test_manager;
use crate::library::{Module, OperationStatus};

#[tokio::test]
async fn operation_lifecycle_persists_status() {
    let manager = test_manager().await.expect("manager");
    let operation_id = manager
        .start_operation(Module::Librarian, "scan")
        .await
        .expect("start");

    manager
        .complete_operation(&operation_id, OperationStatus::Success)
        .await
        .expect("complete");

    let details = manager
        .get_operation_details(&operation_id)
        .await
        .expect("details");
    assert_eq!(details.operation.status, "success");
}

#[tokio::test]
async fn logs_operation_event() {
    let manager = test_manager().await.expect("manager");
    let operation_id = manager
        .start_operation(Module::Gatekeeper, "ingest")
        .await
        .expect("start");

    manager
        .log_event(
            &operation_id,
            "file_admitted",
            None,
            None,
            None,
            None,
            &serde_json::json!({"ok": true}),
        )
        .await
        .expect("log event");

    let events = manager
        .get_events_for_operation(&operation_id)
        .await
        .expect("events");
    assert_eq!(events.len(), 1);
}
