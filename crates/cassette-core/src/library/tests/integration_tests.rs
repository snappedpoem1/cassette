use crate::library::tests::test_manager;

#[tokio::test]
async fn librarian_bind_runs_lifecycle() {
    let manager = test_manager().await.expect("manager");

    let value = manager
        .run_librarian_operation("scan", |_op_id| async move {
            Ok::<_, crate::library::ManagerError>(42usize)
        })
        .await
        .expect("run");

    assert_eq!(value, 42usize);
}
