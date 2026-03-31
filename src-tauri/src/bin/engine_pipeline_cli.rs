use cassette_core::db::Db;
use cassette_core::director::models::FinalizedTrackDisposition;
use cassette_core::director::providers::{
    DeezerProvider, LocalArchiveProvider, QobuzProvider, RealDebridProvider, SlskdProvider,
    UsenetProvider, YtDlpProvider,
};
use cassette_core::director::{
    AcquisitionStrategy, Director, DirectorConfig, DirectorHandle, DuplicatePolicy,
    NormalizedTrack, Provider, ProviderPolicy, QualityPolicy, RetryPolicy, TempRecoveryPolicy,
    TrackTask, TrackTaskSource,
};
use cassette_core::librarian::{LibrarianConfig, ScanMode, run_librarian_sync};
use cassette_core::library::organizer;
use cassette_core::orchestrator::delta::adapter::{ClaimedDownloadRow, DeltaQueueAdapter};
use cassette_core::sources::{RemoteProviderConfig, SlskdConnectionConfig};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use tracing_subscriber::prelude::*;

const STALE_CLAIM_MINUTES: i64 = 30;
const MAX_ZERO_TRACK_RENAMES_ABSOLUTE: usize = 25;
const MAX_ZERO_TRACK_RENAMES_RATIO: f64 = 0.05;

fn parse_scan_mode(args: &[String], resume_shorthand: bool) -> Result<ScanMode, String> {
    let explicit = args
        .windows(2)
        .find(|window| window[0] == "--scan-mode")
        .map(|window| window[1].to_ascii_lowercase());

    match (resume_shorthand, explicit.as_deref()) {
        (true, None) => Ok(ScanMode::Resume),
        (true, Some("resume")) => Ok(ScanMode::Resume),
        (true, Some(other)) => Err(format!(
            "--resume conflicts with --scan-mode {other}; use one scan mode"
        )),
        (false, Some("full")) | (false, None) => Ok(ScanMode::Full),
        (false, Some("resume")) => Ok(ScanMode::Resume),
        (false, Some("delta-only")) => Ok(ScanMode::DeltaOnly),
        (false, Some(other)) => Err(format!(
            "unsupported --scan-mode {other}; expected full, resume, or delta-only"
        )),
    }
}

fn app_db_path() -> Result<PathBuf, String> {
    let app_data = std::env::var("APPDATA").map_err(|e| e.to_string())?;
    Ok(PathBuf::from(app_data)
        .join("dev.cassette.app")
        .join("cassette.db"))
}

fn librarian_db_path() -> Result<PathBuf, String> {
    let app_db = app_db_path()?;
    let parent = app_db
        .parent()
        .ok_or_else(|| "app db has no parent directory".to_string())?;
    Ok(parent.join("cassette_librarian.db"))
}

fn read_setting(db: &Db, key: &str) -> Option<String> {
    db.get_setting(key)
        .ok()
        .flatten()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn load_remote_provider_config(db: &Db) -> RemoteProviderConfig {
    RemoteProviderConfig {
        qobuz_email: read_setting(db, "qobuz_email"),
        qobuz_password: read_setting(db, "qobuz_password"),
        qobuz_password_hash: read_setting(db, "qobuz_password_hash"),
        qobuz_app_id: read_setting(db, "qobuz_app_id"),
        qobuz_app_secret: read_setting(db, "qobuz_app_secret"),
        qobuz_user_auth_token: read_setting(db, "qobuz_user_auth_token"),
        qobuz_secrets: read_setting(db, "qobuz_secrets"),
        deezer_arl: read_setting(db, "deezer_arl"),
        spotify_client_id: read_setting(db, "spotify_client_id"),
        spotify_client_secret: read_setting(db, "spotify_client_secret"),
        spotify_access_token: read_setting(db, "spotify_access_token"),
    }
}

fn load_slskd_connection_config(db: &Db) -> SlskdConnectionConfig {
    SlskdConnectionConfig {
        url: read_setting(db, "slskd_url").unwrap_or_else(|| "http://localhost:5030".to_string()),
        username: read_setting(db, "slskd_user").unwrap_or_else(|| "slskd".to_string()),
        password: read_setting(db, "slskd_pass").unwrap_or_else(|| "slskd".to_string()),
        api_key: read_setting(db, "slskd_api_key"),
    }
}

fn build_director(db: &Db) -> DirectorHandle {
    let library_root = read_setting(db, "library_base").unwrap_or_else(|| "A:\\Music".to_string());
    let staging_root =
        read_setting(db, "staging_folder").unwrap_or_else(|| "A:\\Staging".to_string());
    let remote_provider_config = load_remote_provider_config(db);
    let slskd_connection = load_slskd_connection_config(db);
    let usenet_api_key = read_setting(db, "nzbgeek_api_key");
    let sabnzbd_url = read_setting(db, "sabnzbd_url");
    let sabnzbd_api_key = read_setting(db, "sabnzbd_api_key");

    let config = DirectorConfig {
        library_root: PathBuf::from(&library_root),
        temp_root: PathBuf::from(&staging_root).join(".director-temp"),
        local_search_roots: vec![PathBuf::from(&staging_root)],
        worker_concurrency: 8,
        provider_timeout_secs: 300,
        retry_policy: RetryPolicy {
            max_attempts_per_provider: 2,
            base_backoff_millis: 750,
        },
        quality_policy: QualityPolicy {
            minimum_duration_secs: 30.0,
            max_duration_delta_secs: Some(12.0),
            preferred_extensions: vec!["flac".to_string(), "wav".to_string(), "m4a".to_string()],
        },
        duplicate_policy: DuplicatePolicy::ReplaceIfBetter,
        temp_recovery: TempRecoveryPolicy {
            stale_after_hours: 24,
            quarantine_failures: true,
        },
        provider_policies: vec![
            ProviderPolicy { provider_id: "qobuz".to_string(), max_concurrency: 2 },
            ProviderPolicy { provider_id: "deezer".to_string(), max_concurrency: 4 },
            ProviderPolicy { provider_id: "slskd".to_string(), max_concurrency: 2 },
            ProviderPolicy { provider_id: "usenet".to_string(), max_concurrency: 1 },
            ProviderPolicy { provider_id: "local_archive".to_string(), max_concurrency: 2 },
            ProviderPolicy { provider_id: "yt_dlp".to_string(), max_concurrency: 2 },
            ProviderPolicy { provider_id: "real_debrid".to_string(), max_concurrency: 3 },
        ],
        staging_root: PathBuf::from(&staging_root),
        ..DirectorConfig::default()
    };

    let mut providers: Vec<Arc<dyn Provider>> = vec![
        Arc::new(SlskdProvider::new(
            slskd_connection,
            vec![PathBuf::from(&staging_root), PathBuf::from(&library_root)],
        )),
        Arc::new(QobuzProvider::new(remote_provider_config.clone())),
        Arc::new(DeezerProvider::new(remote_provider_config.clone())),
        Arc::new(UsenetProvider {
            api_key: usenet_api_key,
            sabnzbd_url,
            sabnzbd_api_key,
            scan_roots: vec![PathBuf::from(&staging_root), PathBuf::from(&library_root)],
        }),
        Arc::new(LocalArchiveProvider::new(config.local_search_roots.clone())),
        Arc::new(YtDlpProvider::new("yt-dlp")),
    ];

    if let Some(key) = read_setting(db, "real_debrid_key") {
        providers.push(Arc::new(RealDebridProvider::new(key)));
    }

    Director::new(config, providers).start()
}

fn task_from_claim(
    claim: &ClaimedDownloadRow,
    album_bundle_sizes: &HashMap<String, usize>,
) -> TrackTask {
    let album_key = format!(
        "{}::{}",
        claim.desired.artist_name.to_ascii_lowercase(),
        claim
            .desired
            .album_title
            .clone()
            .unwrap_or_default()
            .to_ascii_lowercase()
    );
    let bundle_size = album_bundle_sizes.get(&album_key).copied().unwrap_or(1);
    let strategy = if claim.action_type == "upgrade_quality" {
        AcquisitionStrategy::HighQualityOnly
    } else if bundle_size > 1 {
        AcquisitionStrategy::DiscographyBatch
    } else {
        AcquisitionStrategy::Standard
    };

    TrackTask {
        task_id: format!(
            "delta-{}-{}",
            claim.desired.id,
            claim.desired.track_title.to_ascii_lowercase()
        ),
        source: match claim.desired.source_name.to_ascii_lowercase().as_str() {
            "spotify" => TrackTaskSource::SpotifyLibrary,
            _ => TrackTaskSource::Manual,
        },
        desired_track_id: Some(claim.desired.id),
        source_operation_id: Some(if claim.source_operation_id.trim().is_empty() {
            format!("delta-claim-{}", claim.delta_id)
        } else {
            claim.source_operation_id.clone()
        }),
        target: NormalizedTrack {
            spotify_track_id: claim.desired.source_track_id.clone(),
            source_playlist: None,
            artist: claim.desired.artist_name.clone(),
            album_artist: Some(claim.desired.artist_name.clone()),
            title: claim.desired.track_title.clone(),
            album: claim.desired.album_title.clone(),
            track_number: claim.desired.track_number.map(|value| value as u32),
            disc_number: claim.desired.disc_number.map(|value| value as u32),
            year: None,
            duration_secs: claim.desired.duration_ms.map(|value| value as f64 / 1000.0),
            isrc: claim.desired.isrc.clone(),
        },
        strategy,
    }
}

fn album_bundle_sizes(claims: &[ClaimedDownloadRow]) -> HashMap<String, usize> {
    let mut counts = HashMap::new();
    for claim in claims {
        let key = format!(
            "{}::{}",
            claim.desired.artist_name.to_ascii_lowercase(),
            claim.desired.album_title.clone().unwrap_or_default().to_ascii_lowercase()
        );
        *counts.entry(key).or_insert(0) += 1;
    }
    counts
}

fn organize_finalized_subset(db: &Db, library_base: &str, finalized_paths: &HashSet<String>) -> Result<(), String> {
    if finalized_paths.is_empty() {
        return Ok(());
    }
    let tracks = db
        .get_all_tracks_unfiltered()
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter(|track| finalized_paths.contains(&track.path))
        .collect::<Vec<_>>();
    if tracks.is_empty() {
        return Ok(());
    }

    let dry_run = organizer::organize_tracks(library_base, &tracks, true);
    let zero_track_renames = dry_run
        .moved
        .iter()
        .filter(|mv| organizer::is_zero_track_rename(&mv.old_path, &mv.new_path))
        .count();
    let zero_track_ratio = if dry_run.moved.is_empty() {
        0.0
    } else {
        zero_track_renames as f64 / dry_run.moved.len() as f64
    };
    if zero_track_renames >= MAX_ZERO_TRACK_RENAMES_ABSOLUTE
        || zero_track_ratio >= MAX_ZERO_TRACK_RENAMES_RATIO
    {
        return Err(format!(
            "Refusing subset organize: detected {} unsafe zero-track renames ({:.1}%).",
            zero_track_renames,
            zero_track_ratio * 100.0
        ));
    }

    let live = organizer::organize_tracks(library_base, &tracks, false);
    for mv in &live.moved {
        db.update_track_path(mv.track_id, &mv.new_path)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (reload_layer, handle) =
        tracing_subscriber::reload::Layer::new(tracing_subscriber::EnvFilter::new("info"));
    tracing_subscriber::registry()
        .with(reload_layer)
        .with(tracing_subscriber::fmt::layer())
        .init();

    let args: Vec<String> = std::env::args().collect();
    let resume_shorthand = args.iter().any(|arg| arg == "--resume");
    let scan_mode = parse_scan_mode(&args, resume_shorthand)?;
    let limit = args
        .windows(2)
        .find(|window| window[0] == "--limit")
        .and_then(|window| window[1].parse::<usize>().ok())
        .unwrap_or(250);
    let desired_state_path = args
        .windows(2)
        .find(|window| window[0] == "--desired-state")
        .map(|window| PathBuf::from(&window[1]));

    let db_path = app_db_path().map_err(|e| format!("app db: {e}"))?;
    let librarian_db_path = librarian_db_path().map_err(|e| format!("librarian db: {e}"))?;
    let connect_options = SqliteConnectOptions::new()
        .filename(&librarian_db_path)
        .create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(connect_options)
        .await?;
    let db = Db::open(&db_path)?;

    let library_base = read_setting(&db, "library_base").unwrap_or_else(|| "A:\\Music".to_string());
    let library_roots = db
        .get_library_roots()?
        .into_iter()
        .map(|root| PathBuf::from(root.path))
        .collect::<Vec<_>>();
    let mut librarian_config = LibrarianConfig::default();
    librarian_config.sqlite_path = librarian_db_path.clone();
    librarian_config.library_roots = if library_roots.is_empty() {
        vec![PathBuf::from(&library_base)]
    } else {
        library_roots
    };
    librarian_config.desired_state_path = desired_state_path.clone();
    librarian_config.scan_mode = scan_mode;

    println!(
        "engine_pipeline_cli starting with scan mode: {}",
        librarian_config.scan_mode.as_str()
    );
    let outcome = run_librarian_sync(
        &pool,
        &librarian_config,
        desired_state_path,
        false,
        &handle,
    )
    .await?;
    println!("{}", outcome.summary);

    let queue = DeltaQueueAdapter::new(pool.clone());
    let reclaimed = queue.reclaim_stale_claims(STALE_CLAIM_MINUTES).await?;
    if reclaimed > 0 {
        println!("Reclaimed {} stale queue claims", reclaimed);
    }

    let run_id = format!("engine-run-{}", uuid::Uuid::new_v4());
    let claimed = queue.claim_download_rows(&run_id, limit).await?;
    if claimed.is_empty() {
        println!("No actionable delta_queue rows.");
        return Ok(());
    }

    let bundle_sizes = album_bundle_sizes(&claimed);
    let tasks = claimed
        .iter()
        .map(|claim| task_from_claim(claim, &bundle_sizes))
        .collect::<Vec<_>>();
    let claim_by_task = claimed
        .iter()
        .cloned()
        .zip(tasks.iter().cloned())
        .map(|(claim, task)| (task.task_id.clone(), (claim, task)))
        .collect::<HashMap<_, _>>();

    let handle_director = build_director(&db);
    let mut results = handle_director.subscribe_results();
    let submitter = handle_director.submitter.clone();

    for task in tasks.iter().cloned() {
        db.upsert_director_pending_task(&task, "Queued")?;
        submitter.submit(task).await?;
    }

    let mut finalized_paths = HashSet::new();
    let expected = claim_by_task.len();
    let mut completed = 0usize;
    while completed < expected {
        let result = results.recv().await?;
        let Some((claim, request_task)) = claim_by_task.get(&result.task_id) else {
            continue;
        };
        db.save_director_task_result(&result, Some(request_task))?;
        db.delete_director_pending_task(&result.task_id)?;
        if let Some(finalized) = &result.finalized {
            finalized_paths.insert(finalized.path.to_string_lossy().to_string());
            if let Ok(track) = cassette_core::library::read_track_metadata(&finalized.path) {
                db.upsert_track(&track)?;
            }
        }

        match result.disposition {
            FinalizedTrackDisposition::Finalized
            | FinalizedTrackDisposition::AlreadyPresent
            | FinalizedTrackDisposition::MetadataOnly => {
                queue.mark_processed(claim.desired.id).await?;
            }
            FinalizedTrackDisposition::Cancelled | FinalizedTrackDisposition::Failed => {
                queue.release_claim(claim.desired.id).await?;
            }
        }
        completed += 1;
    }

    handle_director.shutdown().await?;

    let mut post_sync_config = librarian_config.clone();
    post_sync_config.scan_mode = ScanMode::DeltaOnly;
    let post_outcome = run_librarian_sync(&pool, &post_sync_config, None, true, &handle).await?;
    println!("{}", post_outcome.summary);

    organize_finalized_subset(&db, &library_base, &finalized_paths)
        .map_err(|error| format!("organize subset failed: {error}"))?;

    println!(
        "engine_pipeline_cli complete: claimed={} finalized_paths={}",
        expected,
        finalized_paths.len()
    );

    Ok(())
}
