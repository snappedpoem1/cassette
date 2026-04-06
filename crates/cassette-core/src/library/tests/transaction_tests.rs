use crate::library::tests::test_manager;
use crate::library::Module;

#[tokio::test]
async fn execute_atomic_rolls_back_on_error() {
    let manager = test_manager().await.expect("manager");
    let op = manager
        .start_operation(Module::Gatekeeper, "atomic")
        .await
        .expect("op");

    let result = manager
        .execute_atomic::<i64, _>(&op, |tx| {
            Box::pin(async move {
                sqlx::query("INSERT INTO operation_log(operation_id, module, phase, status, started_at) VALUES('x1','gatekeeper','test','in_progress',CURRENT_TIMESTAMP)")
                    .execute(tx)
                    .await?;
                Err(sqlx::Error::Protocol("boom".to_string()))
            })
        })
        .await;

    assert!(result.is_err());

    let count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM operation_log WHERE operation_id = 'x1'",
    )
    .fetch_one(manager.db_pool())
    .await
    .expect("count");
    assert_eq!(count, 0);
}
