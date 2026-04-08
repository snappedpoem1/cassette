pub mod error;
pub mod logging;
pub mod spotify_import;
pub mod test_library;

use crate::director::download::batch_download;
use crate::director::download::compute_staging_path;
use crate::director::models::{AcquisitionStrategy, NormalizedTrack, TrackTask, TrackTaskSource};
use crate::director::provider::Provider;
use crate::director::providers::{DeezerProvider, QobuzProvider, SlskdProvider};
use crate::director::sources::{BandcampSource, HttpSource, LocalCacheSource, SourceProvider, SpotifySource, YoutubeSource};
use crate::director::strategy::StrategyPlanner;
use crate::director::temp::TempManager;
use crate::director::DirectorConfig;
use crate::gatekeeper::GatekeeperConfig;
use crate::librarian::models::DesiredTrack;
use crate::library::{LibraryManager, ManagerConfig, Module, OperationStatus};
use crate::orchestrator::delta::adapter::DeltaQueueAdapter;
use crate::orchestrator::delta::generation::generate_delta_queue_managed;
use crate::orchestrator::reconciliation::engine::reconcile_desired_against_local;
use crate::orchestrator::sequencing::custodian_phase::run_custodian_phase_managed;
use crate::orchestrator::sequencing::librarian_phase::run_librarian_phase_managed;
use crate::orchestrator::OrchestratorConfig;
use crate::sources::{RemoteProviderConfig, SlskdConnectionConfig};
use crate::validation::error::{Result, ValidationError};
use crate::validation::logging::{
    explain_audit_trace, verify_complete_operation_log, verify_operation_log, LogVerification,
};
use crate::validation::spotify_import::{
    import_spotify_export, verify_spotify_import, ImportSummary,
};
use crate::validation::test_library::{
    reset_validation_environment, sqlite_url_for_path, TestLibraryConfig, TestLibrarySetup,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct ValidationConfig {
    pub test_library: TestLibraryConfig,
    pub cleanup_after_run: bool,
    pub run_director: bool,
    pub run_gatekeeper: bool,
    pub enabled_sources: Vec<String>,
    /// When true, Custodian sorts in-place (sorted_target == library root)
    /// and dry_run is disabled so files actually move.
    pub production: bool,
    /// Force Custodian dry_run even in production mode.
    pub dry_run: bool,
    /// Only run Librarian + Custodian, skip Director/Gatekeeper.
    pub organize_only: bool,
    /// Resume from download phase using existing delta_queue — skip import/scan/reconcile/delta.
    pub download_only: bool,
}

impl Default for ValidationConfig {
    fn default() -> Self {
        Self {
            test_library: TestLibraryConfig::default(),
            cleanup_after_run: true,
            run_director: true,
            run_gatekeeper: true,
            enabled_sources: vec![
                "local_cache".to_string(),
                "http".to_string(),
                "youtube".to_string(),
                "bandcamp".to_string(),
            ],
            production: false,
            dry_run: false,
            organize_only: false,
            download_only: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ValidationReport {
    pub setup_ok: bool,

    pub desired_tracks_imported: usize,

    pub librarian_scanned: usize,
    pub librarian_logged: bool,

    pub custodian_sorted: usize,
    pub custodian_logged: bool,

    pub reconciliation_matched: usize,
    pub reconciliation_missing: usize,
    pub orchestrator_logged: bool,

    pub delta_queue_entries: usize,
    pub delta_logged: bool,

    pub director_downloaded: usize,
    pub director_failed: usize,
    pub director_logged: bool,

    pub gatekeeper_admitted: usize,
    pub gatekeeper_logged: bool,
    pub audit_trace_ok: bool,

    pub total_operations: usize,
    pub total_events: usize,
    pub max_concurrent_locks: usize,
    pub stalled_operations: usize,
    pub orphaned_operations: usize,

    pub reconciliation_2_missing: usize,
    pub gaps_filled: usize,

    pub cleanup_ok: bool,
}

impl ValidationReport {
    pub fn summary(&self) -> String {
        format!(
            "\nCASSETTE VALIDATION REPORT\n\nIMPORT\n  Desired tracks: {}\n\nLIBRARIAN\n  Scanned: {} files\n  Logged: {}\n\nCUSTODIAN\n  Sorted: {} files\n  Logged: {}\n\nORCHESTRATOR\n  Matched: {} tracks\n  Missing: {} tracks -> {}\n  DeltaQueue: {} entries\n  Logged: {}\n\nDIRECTOR\n  Downloaded: {} files\n  Failed: {} files\n  Logged: {}\n\nGATEKEEPER\n  Admitted: {} files\n  Logged: {}\n  Audit trace: {}\n\nOPERATION LOG\n  Total operations: {}\n  Total events: {}\n  Max concurrent locks: {}\n  Stalled operations: {}\n  Orphaned successful operations: {}\n\nVALIDATION\n  Gaps filled: {}\n  Library consistent: {}\n  All logged: {}\n",
            self.desired_tracks_imported,
            self.librarian_scanned,
            self.librarian_logged,
            self.custodian_sorted,
            self.custodian_logged,
            self.reconciliation_matched,
            self.reconciliation_missing,
            self.reconciliation_2_missing,
            self.delta_queue_entries,
            self.orchestrator_logged,
            self.director_downloaded,
            self.director_failed,
            self.director_logged,
            self.gatekeeper_admitted,
            self.gatekeeper_logged,
            self.audit_trace_ok,
            self.total_operations,
            self.total_events,
            self.max_concurrent_locks,
            self.stalled_operations,
            self.orphaned_operations,
            self.gaps_filled,
            self.setup_ok && self.cleanup_ok,
            self.librarian_logged
                && self.custodian_logged
                && self.orchestrator_logged
                && self.delta_logged
                && self.director_logged
                && self.gatekeeper_logged,
        )
    }
}

pub async fn run_full_validation(
    spotify_export: &Path,
    config: &ValidationConfig,
) -> Result<ValidationReport> {
    println!("Setting up test library...");
    let setup = TestLibrarySetup::setup(&config.test_library).await?;

    let db_url = sqlite_url_for_path(&setup.test_db);
    let mut report = ValidationReport {
        setup_ok: true,
        ..Default::default()
    };

    let final_result = {
        let manager = LibraryManager::connect(&db_url, ManagerConfig::default()).await?;
        cleanup_stale_operations(&manager).await?;
        let root_op_id = manager
            .start_operation(Module::Orchestrator, "validation_run")
            .await?;

        let run_result = run_validation_internal(
            &manager,
            &setup,
            spotify_export,
            config,
            &root_op_id,
            &mut report,
        )
        .await;

        match &run_result {
            Ok(()) => {
                manager
                    .complete_operation(&root_op_id, OperationStatus::Success)
                    .await?;
            }
            Err(error) => {
                manager
                    .complete_operation(&root_op_id, OperationStatus::FailedAt(error.to_string()))
                    .await?;
            }
        }

        run_result
    };

    match final_result {
        Ok(()) => {
            if config.cleanup_after_run {
                println!("Cleaning up test environment...");
                match setup.cleanup().await {
                    Ok(()) => {
                        report.cleanup_ok = true;
                    }
                    Err(error) => {
                        eprintln!("Validation cleanup warning: {error}");
                        report.cleanup_ok = false;
                    }
                }
            } else {
                report.cleanup_ok = true;
            }

            Ok(report)
        }
        Err(error) => {
            if config.cleanup_after_run {
                let _ = setup.cleanup().await;
            }

            Err(error)
        }
    }
}

async fn cleanup_stale_operations(manager: &LibraryManager) -> Result<()> {
    let stale_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM operation_log WHERE status = 'in_progress'",
    )
    .fetch_one(manager.db_pool())
    .await?;

    if stale_count > 0 {
        println!("Recovering {stale_count} stale in-progress operations from previous runs...");
        sqlx::query(
            r#"
            UPDATE operation_log
            SET status = 'failed',
                ended_at = CURRENT_TIMESTAMP,
                duration_ms = COALESCE(duration_ms, 0),
                error_message = COALESCE(error_message, 'superseded by new validation run')
            WHERE status = 'in_progress'
            "#,
        )
        .execute(manager.db_pool())
        .await?;

        sqlx::query("DELETE FROM file_locks")
            .execute(manager.db_pool())
            .await?;
    }

    Ok(())
}

pub async fn reset_validation(config: &ValidationConfig) -> Result<()> {
    reset_validation_environment(&config.test_library).await
}

async fn run_validation_internal(
    manager: &LibraryManager,
    setup: &TestLibrarySetup,
    spotify_export: &Path,
    config: &ValidationConfig,
    root_op_id: &str,
    report: &mut ValidationReport,
) -> Result<()> {
    ensure_validation_pipeline_tables(manager.db_pool()).await?;

    if config.download_only {
        return run_download_only(manager, setup, config, root_op_id, report).await;
    }

    println!("Importing Spotify export...");
    let import_summary = import_spotify_export(manager.db_pool(), spotify_export).await?;
    verify_spotify_import(manager.db_pool(), import_summary.db_total_spotify_tracks).await?;
    report.desired_tracks_imported = import_summary.total_imported;
    print_import_summary(&import_summary);

    let mut orchestrator_config = OrchestratorConfig::default();
    orchestrator_config.librarian.library_roots = vec![setup.test_library.clone()];
    orchestrator_config.librarian.enable_content_hashing = false;
    orchestrator_config.library_roots = vec![setup.test_library.clone()];

    // Custodian: scan the library root, sort into canonical structure.
    orchestrator_config.custodian.source_roots = vec![setup.test_library.clone()];
    orchestrator_config.custodian.sorted_target = setup.test_library.clone();
    orchestrator_config.custodian.staging_root = setup.test_staging.join("custodian_staging");
    orchestrator_config.custodian.quarantine_root = setup.test_quarantine.clone();
    if config.production {
        orchestrator_config.custodian.same_volume_move = true;
        orchestrator_config.custodian.verify_copy = false;
        orchestrator_config.custodian.delete_source_after_verify = true;
    }
    if config.production && !config.dry_run {
        orchestrator_config.custodian.dry_run = false;
    } else {
        orchestrator_config.custodian.dry_run = true;
    }

    println!("Running Librarian scan...");
    let scan_outcome =
        run_librarian_phase_managed(manager, root_op_id, &orchestrator_config).await?;
    report.librarian_scanned = scan_outcome.files_scanned;
    report.librarian_logged = verify_operation_log(manager, Module::Librarian, "scan")
        .await
        .is_ok();

    println!("Running Custodian cleanup...");
    let cleanup_outcome =
        run_custodian_phase_managed(manager, root_op_id, &orchestrator_config).await?;
    report.custodian_sorted = cleanup_outcome.files_sorted;
    report.custodian_logged = verify_operation_log(manager, Module::Custodian, "cleanup")
        .await
        .is_ok();

    if config.organize_only {
        println!("Organize-only mode — skipping reconciliation, download, and gatekeeper.");
        return Ok(());
    }

    println!("Running Orchestrator reconciliation...");
    let desired_tracks = sqlx::query_as::<_, DesiredTrack>(
        "SELECT id, source_name, source_track_id, source_album_id, source_artist_id, artist_name, album_title, track_title, track_number, disc_number, duration_ms, isrc, raw_payload_json, imported_at FROM desired_tracks ORDER BY id",
    )
    .fetch_all(manager.db_pool())
    .await?;

    if desired_tracks.is_empty() {
        return Err(ValidationError::InvalidConfig(
            "No desired tracks present after Spotify import".to_string(),
        ));
    }

    let reconciliation_op = manager
        .start_operation(Module::Orchestrator, "reconciliation")
        .await?;

    let reconciliation = match reconcile_desired_against_local(
        manager,
        &reconciliation_op,
        &desired_tracks,
        &orchestrator_config.reconciliation,
    )
    .await
    {
        Ok(result) => {
            manager
                .complete_operation(&reconciliation_op, OperationStatus::Success)
                .await?;
            result
        }
        Err(error) => {
            manager
                .complete_operation(
                    &reconciliation_op,
                    OperationStatus::FailedAt(error.to_string()),
                )
                .await?;
            return Err(error.into());
        }
    };

    report.reconciliation_matched = reconciliation.matched_count;
    report.reconciliation_missing = reconciliation.missing_count;
    report.orchestrator_logged =
        verify_operation_log(manager, Module::Orchestrator, "reconciliation")
            .await
            .is_ok();

    println!("Generating DeltaQueue...");
    let delta_op = manager
        .start_operation(Module::Orchestrator, "delta")
        .await?;
    let delta_queue = match generate_delta_queue_managed(manager, &delta_op, &reconciliation).await
    {
        Ok(deltas) => {
            manager
                .complete_operation(&delta_op, OperationStatus::Success)
                .await?;
            deltas
        }
        Err(error) => {
            manager
                .complete_operation(&delta_op, OperationStatus::FailedAt(error.to_string()))
                .await?;
            return Err(error.into());
        }
    };

    report.delta_queue_entries = delta_queue.len();
    report.delta_logged = verify_operation_log(manager, Module::Orchestrator, "delta")
        .await
        .is_ok();

    let adapter = DeltaQueueAdapter::new(manager.db_pool().clone());
    let desired_for_download = adapter
        .extract_desired_tracks_for_download(&delta_queue)
        .await?;

    if config.run_director {
        println!("Downloading missing tracks...");
        let sources = build_sources(config, setup);
        let mut director_config = DirectorConfig::default();
        director_config.staging_root = setup.test_staging.clone();
        director_config.temp_root = setup.test_staging.join("tmp");
        director_config.local_search_roots = vec![setup.test_library.clone()];

        let download_outcome =
            batch_download(manager, &desired_for_download, &sources, &director_config).await?;
        report.director_downloaded = download_outcome.successfully_downloaded;
        report.director_failed = download_outcome.errors.len();

        report.director_logged = verify_operation_log(manager, Module::Director, "batch_download")
            .await
            .is_ok();

        if config.run_gatekeeper {
            println!("Admitting files with Gatekeeper...");
            let mut gatekeeper_config = GatekeeperConfig::default();
            gatekeeper_config.canonical_library_root = setup.test_library.clone();
            gatekeeper_config.staging_root = setup.test_staging.clone();
            gatekeeper_config.quarantine_root = setup.test_quarantine.clone();
            gatekeeper_config.audit_manifest_dir = setup.test_quarantine.join("audit");

            let owned_entries =
                build_gatekeeper_entries(&desired_for_download, &setup.test_staging);
            let desired_by_id: HashMap<i64, &DesiredTrack> = desired_for_download
                .iter()
                .map(|track| (track.id, track))
                .collect();
            let entries: Vec<(&Path, Option<&DesiredTrack>)> = owned_entries
                .iter()
                .filter_map(|(path, desired_id)| {
                    desired_by_id
                        .get(desired_id)
                        .map(|desired| (path.as_path(), Some(*desired)))
                })
                .collect();

            let outcome = manager
                .run_gatekeeper_with_manager(&entries, &gatekeeper_config)
                .await?;
            report.gatekeeper_admitted = outcome.admitted;

            report.gatekeeper_logged =
                verify_operation_log(manager, Module::Gatekeeper, "batch_ingest")
                    .await
                    .is_ok();
            if let Some((path, desired_id)) = owned_entries.first() {
                let trace = explain_audit_trace(manager, path.to_str(), Some(*desired_id)).await?;
                report.audit_trace_ok =
                    !trace.operation_events.is_empty() && !trace.gatekeeper_audit.is_empty();
            }
        }
    }

    println!("Verifying operation log completeness...");
    let log_report: LogVerification = verify_complete_operation_log(manager).await?;
    report.total_operations = log_report.total_operations;
    report.total_events = log_report.total_events;
    report.max_concurrent_locks = log_report.max_concurrent_locks;
    report.stalled_operations = log_report.stalled_operations;
    report.orphaned_operations = log_report.orphaned_operations;

    println!("Re-running Orchestrator reconciliation...");
    let reconciliation_2_op = manager
        .start_operation(Module::Orchestrator, "reconciliation")
        .await?;

    let reconciliation_2 = match reconcile_desired_against_local(
        manager,
        &reconciliation_2_op,
        &desired_tracks,
        &orchestrator_config.reconciliation,
    )
    .await
    {
        Ok(result) => {
            manager
                .complete_operation(&reconciliation_2_op, OperationStatus::Success)
                .await?;
            result
        }
        Err(error) => {
            manager
                .complete_operation(
                    &reconciliation_2_op,
                    OperationStatus::FailedAt(error.to_string()),
                )
                .await?;
            return Err(error.into());
        }
    };

    report.reconciliation_2_missing = reconciliation_2.missing_count;
    report.gaps_filled = report
        .reconciliation_missing
        .saturating_sub(report.reconciliation_2_missing);

    Ok(())
}

async fn ensure_validation_pipeline_tables(pool: &sqlx::SqlitePool) -> Result<()> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS sync_runs (
            id INTEGER PRIMARY KEY,
            run_id TEXT UNIQUE NOT NULL,
            started_at TIMESTAMP NOT NULL,
            ended_at TIMESTAMP,
            status TEXT NOT NULL,
            phase_reached TEXT NOT NULL,
            files_scanned INTEGER DEFAULT 0,
            files_upserted INTEGER DEFAULT 0,
            desired_tracks_imported INTEGER DEFAULT 0,
            reconciliation_completed BOOLEAN DEFAULT FALSE,
            delta_queue_entries INTEGER DEFAULT 0,
            error_message TEXT,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        )
        "#,
    )
    .execute(pool)
    .await?;

    let _ = sqlx::query("CREATE INDEX IF NOT EXISTS idx_sync_runs_run_id ON sync_runs(run_id)")
        .execute(pool)
        .await;
    let _ = sqlx::query("CREATE INDEX IF NOT EXISTS idx_sync_runs_status ON sync_runs(status)")
        .execute(pool)
        .await;

    // Librarian upserts depend on these uniqueness guarantees.
    let _ = sqlx::query(
        "CREATE UNIQUE INDEX IF NOT EXISTS uq_artists_normalized_name ON artists(normalized_name)",
    )
    .execute(pool)
    .await;
    let _ = sqlx::query(
        "CREATE UNIQUE INDEX IF NOT EXISTS uq_albums_artist_normalized_title ON albums(artist_id, normalized_title)",
    )
    .execute(pool)
    .await;
    let _ = sqlx::query("CREATE UNIQUE INDEX IF NOT EXISTS uq_tracks_isrc ON tracks(isrc)")
        .execute(pool)
        .await;

    Ok(())
}

fn print_import_summary(summary: &ImportSummary) {
    println!(
        "Imported {} tracks ({} duplicates skipped, {} spotify rows in DB)",
        summary.total_imported, summary.duplicates_skipped, summary.db_total_spotify_tracks
    );
}

async fn run_download_only(
    manager: &LibraryManager,
    setup: &TestLibrarySetup,
    config: &ValidationConfig,
    _root_op_id: &str,
    report: &mut ValidationReport,
) -> Result<()> {
    println!("Download-only mode — loading existing delta_queue...");

    let rows = sqlx::query_as::<_, DesiredTrack>(
        "SELECT d.id, d.source_name, d.source_track_id, d.source_album_id, d.source_artist_id, \
         d.artist_name, d.album_title, d.track_title, d.track_number, d.disc_number, \
         d.duration_ms, d.isrc, d.raw_payload_json, d.imported_at \
         FROM desired_tracks d \
         INNER JOIN delta_queue dq ON dq.desired_track_id = d.id \
         WHERE dq.action_type IN ('missing_download', 'upgrade_quality') \
           AND dq.processed_at IS NULL \
         ORDER BY dq.priority DESC, d.id",
    )
    .fetch_all(manager.db_pool())
    .await?;

    println!("Found {} tracks to download from delta_queue", rows.len());
    report.delta_queue_entries = rows.len();

    if rows.is_empty() {
        println!("Nothing to download.");
        return Ok(());
    }

    let sources = build_sources(config, setup);
    let mut director_config = DirectorConfig::default();
    director_config.staging_root = setup.test_staging.clone();
    director_config.temp_root = setup.test_staging.join("tmp");
    director_config.local_search_roots = vec![setup.test_library.clone()];

    let download_outcome = batch_download(manager, &rows, &sources, &director_config).await?;
    report.director_downloaded = download_outcome.successfully_downloaded;
    report.director_failed = download_outcome.errors.len();
    report.director_logged = verify_operation_log(manager, Module::Director, "batch_download")
        .await
        .is_ok();

    if config.run_gatekeeper {
        println!("Admitting files with Gatekeeper...");
        let mut gatekeeper_config = GatekeeperConfig::default();
        gatekeeper_config.canonical_library_root = setup.test_library.clone();
        gatekeeper_config.staging_root = setup.test_staging.clone();
        gatekeeper_config.quarantine_root = setup.test_quarantine.clone();
        gatekeeper_config.audit_manifest_dir = setup.test_quarantine.join("audit");

        let owned_entries = build_gatekeeper_entries(&rows, &setup.test_staging);
        let desired_by_id: HashMap<i64, &DesiredTrack> =
            rows.iter().map(|track| (track.id, track)).collect();
        let entries: Vec<(&Path, Option<&DesiredTrack>)> = owned_entries
            .iter()
            .filter_map(|(path, desired_id)| {
                desired_by_id
                    .get(desired_id)
                    .map(|desired| (path.as_path(), Some(*desired)))
            })
            .collect();

        let outcome = manager
            .run_gatekeeper_with_manager(&entries, &gatekeeper_config)
            .await?;
        report.gatekeeper_admitted = outcome.admitted;
        report.gatekeeper_logged =
            verify_operation_log(manager, Module::Gatekeeper, "batch_ingest")
                .await
                .is_ok();
        if let Some((path, desired_id)) = owned_entries.first() {
            let trace = explain_audit_trace(manager, path.to_str(), Some(*desired_id)).await?;
            report.audit_trace_ok =
                !trace.operation_events.is_empty() && !trace.gatekeeper_audit.is_empty();
        }
    }

    Ok(())
}

fn build_sources(
    config: &ValidationConfig,
    setup: &TestLibrarySetup,
) -> Vec<Arc<dyn SourceProvider>> {
    let mut providers: Vec<Arc<dyn SourceProvider>> = Vec::new();
    let enabled: HashMap<String, bool> = config
        .enabled_sources
        .iter()
        .map(|name| (name.to_ascii_lowercase(), true))
        .collect();

    if enabled.contains_key("local_cache") {
        providers.push(Arc::new(LocalCacheSource::new(vec![setup
            .test_library
            .clone()])));
    }
    if enabled.contains_key("http") {
        providers.push(Arc::new(HttpSource::new()));
    }
    if enabled.contains_key("youtube") {
        providers.push(Arc::new(YoutubeSource));
    }
    if enabled.contains_key("bandcamp") {
        providers.push(Arc::new(BandcampSource));
    }
    if enabled.contains_key("spotify") {
        providers.push(Arc::new(SpotifySource::new(String::new(), String::new())));
    }

    let director_config = DirectorConfig {
        staging_root: setup.test_staging.clone(),
        temp_root: setup.test_staging.join("tmp"),
        local_search_roots: vec![setup.test_library.clone()],
        ..DirectorConfig::default()
    };

    // Wire real acquisition providers via the bridge adapter.
    let slskd_url =
        std::env::var("SLSKD_URL").unwrap_or_else(|_| "http://localhost:5030".to_string());
    let slskd_user = std::env::var("SLSKD_USER").unwrap_or_else(|_| "slskd".to_string());
    let slskd_pass = std::env::var("SLSKD_PASSWORD").unwrap_or_else(|_| "slskd".to_string());

    let slskd_config = SlskdConnectionConfig {
        url: slskd_url,
        username: slskd_user,
        password: slskd_pass,
        api_key: None,
    };

    let slskd: Arc<dyn Provider> = Arc::new(SlskdProvider::new(
        slskd_config,
        vec![setup.test_library.clone(), setup.test_staging.clone()],
    ));
    providers.push(Arc::new(ValidationProviderSourceAdapter::new(
        slskd,
        &director_config,
    )));

    // Qobuz & Deezer: load credentials from env, log registration status.
    let remote_config = RemoteProviderConfig::from_env();

    let has_qobuz = remote_config.qobuz_app_id.is_some() && remote_config.qobuz_email.is_some();
    if has_qobuz {
        let qobuz: Arc<dyn Provider> = Arc::new(QobuzProvider::new(remote_config.clone()));
        providers.push(Arc::new(ValidationProviderSourceAdapter::new(
            qobuz,
            &director_config,
        )));
        println!(
            "[sources] Qobuz provider registered (email={})",
            remote_config.qobuz_email.as_deref().unwrap_or("?")
        );
    } else {
        println!(
            "[sources] Qobuz SKIPPED — missing env: QOBUZ_APP_ID={}, QOBUZ_EMAIL={}",
            if remote_config.qobuz_app_id.is_some() {
                "set"
            } else {
                "MISSING"
            },
            if remote_config.qobuz_email.is_some() {
                "set"
            } else {
                "MISSING"
            },
        );
    }

    let has_deezer = remote_config.deezer_arl.is_some();
    if has_deezer {
        let deezer: Arc<dyn Provider> = Arc::new(DeezerProvider::new(remote_config));
        providers.push(Arc::new(ValidationProviderSourceAdapter::new(
            deezer,
            &director_config,
        )));
        println!("[sources] Deezer provider registered (ARL present)");
    } else {
        println!("[sources] Deezer SKIPPED — DEEZER_ARL not set");
    }

    let names: Vec<&str> = providers.iter().map(|p| p.name()).collect();
    println!(
        "[sources] {} providers registered: {:?}",
        providers.len(),
        names
    );

    providers
}

struct ValidationProviderSourceAdapter {
    provider: Arc<dyn Provider>,
    config: DirectorConfig,
    name: &'static str,
}

impl ValidationProviderSourceAdapter {
    fn new(provider: Arc<dyn Provider>, config: &DirectorConfig) -> Self {
        let descriptor = provider.descriptor();
        let name: &'static str = Box::leak(descriptor.id.clone().into_boxed_str());
        Self {
            provider,
            config: config.clone(),
            name,
        }
    }
}

#[async_trait::async_trait]
impl SourceProvider for ValidationProviderSourceAdapter {
    fn name(&self) -> &'static str {
        self.name
    }

    fn can_handle(&self, _track: &DesiredTrack) -> bool {
        let desc = self.provider.descriptor();
        desc.capabilities.supports_search && desc.capabilities.supports_download
    }

    async fn resolve_download_url(
        &self,
        track: &DesiredTrack,
    ) -> std::result::Result<crate::director::sources::ResolvedTrack, crate::director::sources::SourceError> {
        let task = desired_track_to_task(track);
        let planner = StrategyPlanner;
        let strategy = planner.plan(&task, &[self.provider.descriptor()], &self.config);

        let candidates = self
            .provider
            .search(&task, &strategy)
            .await
            .map_err(|e| crate::director::sources::SourceError::ApiError(format!("{}: {}", self.name, e)))?;

        let best = candidates.first().ok_or_else(|| {
            crate::director::sources::SourceError::NotAvailable(format!(
                "{}: no candidates for {} - {}",
                self.name, track.artist_name, track.track_title
            ))
        })?;

        let temp_root = self.config.temp_root.join("validation_adapter");
        let temp_manager = TempManager::new(temp_root, self.config.temp_recovery.clone());
        let task_id = Uuid::new_v4().to_string();
        let temp_context = temp_manager
            .prepare_task(&task_id)
            .await
            .map_err(|e| crate::director::sources::SourceError::ApiError(format!("temp setup: {e}")))?;

        let acquisition = self
            .provider
            .acquire(&task, best, &temp_context, &strategy)
            .await
            .map_err(|e| {
                crate::director::sources::SourceError::ApiError(format!(
                    "{}: acquire failed: {}",
                    self.name, e
                ))
            })?;

        let file_url = format!("file://{}", acquisition.temp_path.to_string_lossy());

        let codec = acquisition
            .extension_hint
            .clone()
            .or_else(|| best.extension_hint.clone());
        let bitrate = best.bitrate_kbps;

        Ok(crate::director::sources::ResolvedTrack {
            download_url: file_url,
            suggested_filename: acquisition
                .temp_path
                .file_name()
                .and_then(|v| v.to_str())
                .unwrap_or("acquired.bin")
                .to_string(),
            expected_codec: codec,
            expected_bitrate: bitrate,
            expected_duration_ms: track.duration_ms.map(|v| v as u64),
            metadata: serde_json::json!({
                "source": self.name,
                "provider_candidate_id": best.provider_candidate_id,
                "metadata_confidence": best.metadata_confidence,
                "artist": best.artist,
                "title": best.title,
                "album": best.album,
            }),
        })
    }

    async fn check_availability(
        &self,
        track: &DesiredTrack,
    ) -> std::result::Result<bool, crate::director::sources::SourceError> {
        let task = desired_track_to_task(track);
        let planner = StrategyPlanner;
        let strategy = planner.plan(&task, &[self.provider.descriptor()], &self.config);

        match self.provider.search(&task, &strategy).await {
            Ok(candidates) => Ok(!candidates.is_empty()),
            Err(_) => Ok(false),
        }
    }
}

fn desired_track_to_task(track: &DesiredTrack) -> TrackTask {
    TrackTask {
        task_id: format!("validation-{}", track.id),
        source: track
            .source_track_id
            .as_ref()
            .filter(|id| id.starts_with("spotify:"))
            .map(|_| TrackTaskSource::SpotifyLibrary)
            .unwrap_or(TrackTaskSource::Manual),
        desired_track_id: Some(track.id),
        source_operation_id: None,
        target: NormalizedTrack {
            spotify_track_id: track.source_track_id.clone(),
            source_album_id: track.source_album_id.clone(),
            source_artist_id: track.source_artist_id.clone(),
            source_playlist: None,
            artist: track.artist_name.clone(),
            album_artist: None,
            title: track.track_title.clone(),
            album: track.album_title.clone(),
            track_number: track.track_number.map(|v| v as u32),
            disc_number: track.disc_number.map(|v| v as u32),
            year: None,
            duration_secs: track.duration_ms.map(|v| v as f64 / 1000.0),
            isrc: track.isrc.clone(),
            musicbrainz_recording_id: None,
            musicbrainz_release_group_id: None,
            musicbrainz_release_id: None,
            canonical_artist_id: None,
            canonical_release_id: None,
        },
        strategy: AcquisitionStrategy::Standard,
    }
}

fn build_gatekeeper_entries(
    desired_tracks: &[DesiredTrack],
    staging_root: &Path,
) -> Vec<(PathBuf, i64)> {
    let mut entries = Vec::new();

    for desired in desired_tracks {
        let staged = compute_staging_path(staging_root, desired);
        if staged.exists() {
            entries.push((staged, desired.id));
        }
    }

    entries
}
