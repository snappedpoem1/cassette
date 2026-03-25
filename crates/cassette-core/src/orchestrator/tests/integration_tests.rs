use super::test_manager;
use crate::library::integration::orchestrator_bind::run_orchestrator_with_manager;
use crate::orchestrator::OrchestratorConfig;

#[tokio::test]
async fn orchestrator_bind_runs_and_returns_summary() {
    let manager = test_manager().await;

    sqlx::query("INSERT INTO desired_tracks (id, source_name, artist_name, track_title, imported_at) VALUES (2, 'spotify', 'Nobody', 'Nowhere', datetime('now'))")
        .execute(manager.db_pool())
        .await
        .expect("desired");

    let cfg = OrchestratorConfig {
        run_librarian: false,
        run_custodian: false,
        run_reconciliation: true,
        ..OrchestratorConfig::default()
    };

    let outcome = run_orchestrator_with_manager(&manager, &cfg)
        .await
        .expect("orchestrator outcome");

    assert!(!outcome.root_operation_id.is_empty());
    assert!(outcome.summary.contains("deltas"));
}
