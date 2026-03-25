use crate::library::tests::test_manager;
use crate::library::Module;
use std::path::Path;

#[tokio::test]
async fn acquires_and_releases_lock() {
    let manager = test_manager().await.expect("manager");
    let op = manager
        .start_operation(Module::Gatekeeper, "lock_test")
        .await
        .expect("operation");

    let path = Path::new("C:/tmp/file_a.flac");
    let _guard = manager
        .acquire_lock_for_file(path, Module::Gatekeeper, &op, 500)
        .await
        .expect("lock");

    assert!(manager.is_file_locked(path).await);
    manager
        .release_lock_for_file(path)
        .await
        .expect("release lock");
    assert!(!manager.is_file_locked(path).await);
}

#[tokio::test]
async fn lock_timeout_when_contended() {
    let manager = test_manager().await.expect("manager");
    let op_a = manager
        .start_operation(Module::Gatekeeper, "lock_a")
        .await
        .expect("operation a");
    let op_b = manager
        .start_operation(Module::Custodian, "lock_b")
        .await
        .expect("operation b");

    let path = Path::new("C:/tmp/file_b.flac");
    let _guard = manager
        .acquire_lock_for_file(path, Module::Gatekeeper, &op_a, 500)
        .await
        .expect("lock a");

    let result = manager
        .acquire_lock_for_file(path, Module::Custodian, &op_b, 150)
        .await;
    assert!(result.is_err());
}
