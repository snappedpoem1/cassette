use cassette_lib::album_resolver::{
    metadata_service_from_remote_config,
    resolve_album_track_tasks as resolve_album_track_tasks_with_metadata,
};
use cassette_core::db::{Db, TrackPathUpdate};
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
use sqlx::Row;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use tracing_subscriber::prelude::*;

const STALE_CLAIM_MINUTES_DEFAULT: i64 = 30;
const MAX_ZERO_TRACK_RENAMES_ABSOLUTE: usize = 25;
const MAX_ZERO_TRACK_RENAMES_RATIO: f64 = 0.05;
const FINGERPRINT_BACKFILL_LIMIT_DEFAULT: usize = 64;
const FINGERPRINT_BACKFILL_CONCURRENCY_DEFAULT: usize = 4;

async fn pending_non_download_summary(pool: &sqlx::SqlitePool) -> Result<Option<String>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT action_type, COUNT(*) AS row_count
         FROM delta_queue
         WHERE processed_at IS NULL
           AND action_type NOT IN ('missing_download', 'upgrade_quality')
         GROUP BY action_type
         ORDER BY action_type ASC",
    )
    .fetch_all(pool)
    .await?;

    if rows.is_empty() {
        return Ok(None);
    }

    let summary = rows
        .into_iter()
        .map(|row| {
            let action_type: String = row.get("action_type");
            let row_count: i64 = row.get("row_count");
            format!("{action_type}={row_count}")
        })
        .collect::<Vec<_>>()
        .join(", ");
    Ok(Some(summary))
}

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

fn skip_content_hash(args: &[String], scan_mode: ScanMode) -> bool {
    let explicit_skip = args.iter().any(|arg| arg == "--skip-content-hash");
    let explicit_enable = args.iter().any(|arg| arg == "--with-content-hash");

    if explicit_skip && explicit_enable {
        return true;
    }

    if explicit_skip {
        return true;
    }

    if explicit_enable {
        return false;
    }

    !matches!(scan_mode, ScanMode::Full)
}

fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|arg| arg == flag)
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
    // DB wins; fall back to env var (uppercase key, e.g. real_debrid_key → REAL_DEBRID_KEY)
    db.get_setting(key)
        .ok()
        .flatten()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            std::env::var(key.to_ascii_uppercase())
                .ok()
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
        })
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

fn build_director(db: &Db, runtime_db_path: PathBuf) -> DirectorHandle {
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
        runtime_db_path: Some(runtime_db_path),
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
            ProviderPolicy { provider_id: "jackett".to_string(), max_concurrency: 3 },
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

    let rd_key = read_setting(db, "real_debrid_key");
    if let Some(ref key) = rd_key {
        providers.push(Arc::new(RealDebridProvider::with_direct_search(
            key.clone(),
            false,
        )));
    }

    // Jackett: multi-indexer torrent search via Torznab, resolved through Real-Debrid
    let jackett_url = read_setting(db, "jackett_url").filter(|u| !u.trim().is_empty());
    let jackett_api_key = read_setting(db, "jackett_api_key").filter(|k| !k.trim().is_empty());
    if let (Some(jurl), Some(jkey), Some(ref rdkey)) = (jackett_url, jackett_api_key, &rd_key) {
        providers.push(Arc::new(cassette_core::director::providers::JackettProvider::new(jurl, jkey, rdkey.clone())));
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
            source_album_id: claim.desired.source_album_id.clone(),
            source_artist_id: claim.desired.source_artist_id.clone(),
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
            musicbrainz_recording_id: None,
            musicbrainz_release_id: None,
            canonical_artist_id: None,
            canonical_release_id: None,
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

fn organize_finalized_subset(
    db: &Db,
    sidecar_db_path: &std::path::Path,
    library_base: &str,
    finalized_paths: &HashSet<String>,
) -> Result<(), String> {
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
    let updates = live
        .moved
        .iter()
        .map(|mv| TrackPathUpdate {
            track_id: mv.track_id,
            old_path: mv.old_path.clone(),
            new_path: mv.new_path.clone(),
        })
        .collect::<Vec<_>>();
    db.apply_track_path_updates(sidecar_db_path, &updates)
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Returns true for tracks that are virtually never available on streaming providers:
/// skits, intros, interludes, outros, spoken word inserts, hidden tracks, and
/// version suffixes that indicate alternate-only releases (live version, slow version, etc.).
fn is_unfetchable_track(title: &str) -> bool {
    let t = title.to_ascii_lowercase();
    // Substring patterns
    let contains_patterns = [
        "skit", "interlude", " intro", " outro", " instrumental",
        " reprise", " spoken", " narration", " monologue",
        " slow version", " live version", " acoustic version",
        " self titled demo", " demo", " revised", " dub",
        "endless nameless",
        "public service announcement", "drill sergeant", "bong hit",
        "mad flava", "heavy flow",
    ];
    // Suffix-only patterns (track title ends with these)
    let suffix_patterns = [
        "intro", "outro", "skit", "interlude", "instrumental",
        "reprise", " arto",
    ];
    contains_patterns.iter().any(|p| t.contains(p))
        || suffix_patterns.iter().any(|p| t.ends_with(p))
}

/// Resolve and submit Spotify missing albums directly to the Director.
/// Each album is resolved via MusicBrainz to per-track tasks, then submitted.
/// Returns (albums_attempted, tracks_submitted, errors).
async fn run_spotify_backlog(
    db: &Db,
    handle_director: &DirectorHandle,
    metadata: &cassette_core::metadata::MetadataService,
    min_plays: i64,
    limit: usize,
) -> Result<(usize, usize, Vec<String>), Box<dyn std::error::Error>> {
    use cassette_core::director::{AcquisitionStrategy, TrackTaskSource};

    let missing = db.get_missing_spotify_albums_with_min_plays(min_plays)?;
    let completed_keys = db.get_completed_task_keys()?;

    let mut albums_attempted = 0usize;
    let mut tracks_submitted = 0usize;
    let mut errors: Vec<String> = Vec::new();
    for album in &missing {
        if tracks_submitted >= limit {
            break;
        }
        let artist = album.artist.trim();
        let title = album.album.trim();
        if artist.is_empty() || title.is_empty() {
            continue;
        }
        albums_attempted += 1;

        let resolved = match resolve_album_track_tasks_with_metadata(
            metadata,
            artist,
            title,
            TrackTaskSource::SpotifyHistory,
            AcquisitionStrategy::DiscographyBatch,
        )
        .await
        {
            Ok(tasks) => tasks,
            Err(e) => {
                errors.push(format!("{artist} - {title}: {e}"));
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                continue;
            }
        };

        let has_bonus_disc = resolved
            .tasks
            .iter()
            .any(|task| task.target.disc_number.unwrap_or(1) > 1);
        for task in &resolved.tasks {
            if tracks_submitted >= limit {
                break;
            }
            if has_bonus_disc && task.target.disc_number.unwrap_or(1) > 1 {
                continue;
            }
            if is_unfetchable_track(&task.target.title) {
                continue;
            }
            if completed_keys.contains(&task.task_id) {
                continue;
            }
            handle_director.submitter.submit(task.clone()).await?;
            tracks_submitted += 1;
        }

        tokio::time::sleep(std::time::Duration::from_millis(350)).await;
    }

    Ok((albums_attempted, tracks_submitted, errors))
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
    let disable_content_hash = skip_content_hash(&args, scan_mode);
    let skip_scan = has_flag(&args, "--skip-scan");
    let skip_post_sync = has_flag(&args, "--skip-post-sync");
    let skip_organize_subset = has_flag(&args, "--skip-organize-subset");
    let limit = args
        .windows(2)
        .find(|window| window[0] == "--limit")
        .and_then(|window| window[1].parse::<usize>().ok())
        .unwrap_or(250);
    let desired_state_path = args
        .windows(2)
        .find(|window| window[0] == "--desired-state")
        .map(|window| PathBuf::from(&window[1]));
    let stale_claim_minutes = args
        .windows(2)
        .find(|window| window[0] == "--stale-claim-minutes")
        .and_then(|window| window[1].parse::<i64>().ok())
        .unwrap_or(STALE_CLAIM_MINUTES_DEFAULT);
    let import_spotify_missing = has_flag(&args, "--import-spotify-missing");
    let disable_fingerprint_backfill = has_flag(&args, "--skip-fingerprint-backfill");
    let fingerprint_backfill_limit = args
        .windows(2)
        .find(|window| window[0] == "--fingerprint-backfill-limit")
        .and_then(|window| window[1].parse::<usize>().ok())
        .unwrap_or(FINGERPRINT_BACKFILL_LIMIT_DEFAULT);
    let fingerprint_backfill_concurrency = args
        .windows(2)
        .find(|window| window[0] == "--fingerprint-backfill-concurrency")
        .and_then(|window| window[1].parse::<usize>().ok())
        .unwrap_or(FINGERPRINT_BACKFILL_CONCURRENCY_DEFAULT);
    let spotify_min_plays = args
        .windows(2)
        .find(|window| window[0] == "--min-plays")
        .and_then(|window| window[1].parse::<i64>().ok())
        .unwrap_or(10);

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
    librarian_config.enable_content_hashing = !disable_content_hash;
    librarian_config.enable_fingerprint_backfill =
        !disable_fingerprint_backfill && fingerprint_backfill_limit > 0;
    librarian_config.fingerprint_backfill_limit = fingerprint_backfill_limit;
    librarian_config.fingerprint_backfill_concurrency = fingerprint_backfill_concurrency.max(1);
    librarian_config.skip_scan = skip_scan;

    // Spotify backlog mode: resolve missing albums directly via Director, bypass sidecar queue
    if import_spotify_missing {
        println!(
            "engine_pipeline_cli: spotify-backlog mode (min_plays={}, limit={})",
            spotify_min_plays, limit
        );
        let handle_director = build_director(&db, db_path.clone());
        let mut results = handle_director.subscribe_results();

        let remote_provider_config = load_remote_provider_config(&db);
        let metadata = metadata_service_from_remote_config(&remote_provider_config)?;
        let (albums_attempted, tracks_submitted, errors) =
            run_spotify_backlog(&db, &handle_director, &metadata, spotify_min_plays, limit)
                .await?;

        println!(
            "spotify-backlog: albums_attempted={} tracks_submitted={} errors={}",
            albums_attempted, tracks_submitted, errors.len()
        );
        for err in &errors {
            eprintln!("  backlog-error: {err}");
        }

        let mut completed_spotify = 0usize;
        let mut finalized_spotify = HashSet::new();
        while completed_spotify < tracks_submitted {
            let result = results.recv().await?;
            db.save_director_task_result(&result, None)?;
            db.delete_director_pending_task(&result.task_id)?;
            if let Some(finalized) = &result.finalized {
                finalized_spotify.insert(finalized.path.to_string_lossy().to_string());
                if let Ok(mut track) = cassette_core::library::read_track_metadata(&finalized.path) {
                    cassette_core::library::enrich_track_with_director_result(&mut track, &result);
                    db.upsert_track(&track)?;
                }
            }
            completed_spotify += 1;
            if completed_spotify % 10 == 0 || completed_spotify == tracks_submitted {
                println!(
                    "  spotify-backlog progress: {}/{} tracks done, {} finalized",
                    completed_spotify,
                    tracks_submitted,
                    finalized_spotify.len()
                );
            }
        }

        handle_director.shutdown().await?;
        println!(
            "spotify-backlog complete: tracks_finalized={}",
            finalized_spotify.len()
        );
        return Ok(());
    }

    println!(
        "engine_pipeline_cli starting with scan mode: {} (content_hashing={}, fingerprint_backfill_limit={}, fingerprint_backfill_concurrency={}, skip_scan={}, skip_post_sync={}, skip_organize_subset={})",
        librarian_config.scan_mode.as_str(),
        if librarian_config.enable_content_hashing {
            "enabled"
        } else {
            "disabled"
        },
        if librarian_config.enable_fingerprint_backfill {
            librarian_config.fingerprint_backfill_limit.to_string()
        } else {
            "disabled".to_string()
        },
        if librarian_config.enable_fingerprint_backfill {
            librarian_config.fingerprint_backfill_concurrency.to_string()
        } else {
            "disabled".to_string()
        },
        librarian_config.skip_scan,
        skip_post_sync,
        skip_organize_subset
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
    let reclaimed = queue.reclaim_stale_claims(stale_claim_minutes).await?;
    if reclaimed > 0 {
        println!("Reclaimed {} stale queue claims", reclaimed);
    }

    let run_id = format!("engine-run-{}", uuid::Uuid::new_v4());
    let claimed = queue.claim_download_rows(&run_id, limit).await?;
    if claimed.is_empty() {
        if let Some(summary) = pending_non_download_summary(&pool).await? {
            println!("No actionable download rows. Pending review queue: {summary}");
        } else {
            println!("No actionable delta_queue rows.");
        }
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

    let handle_director = build_director(&db, db_path.clone());
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
            if let Ok(mut track) = cassette_core::library::read_track_metadata(&finalized.path) {
                cassette_core::library::enrich_track_with_director_result(&mut track, &result);
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

    if skip_post_sync {
        println!("Skipping post-run librarian sync by request.");
    } else {
        let mut post_sync_config = librarian_config.clone();
        post_sync_config.scan_mode = ScanMode::DeltaOnly;
        post_sync_config.skip_scan = false;
        let post_outcome = run_librarian_sync(&pool, &post_sync_config, None, true, &handle).await?;
        println!("{}", post_outcome.summary);
    }

    if skip_organize_subset {
        println!("Skipping organizer subset by request.");
    } else {
        organize_finalized_subset(&db, &librarian_db_path, &library_base, &finalized_paths)
            .map_err(|error| format!("organize subset failed: {error}"))?;
    }

    println!(
        "engine_pipeline_cli complete: claimed={} finalized_paths={}",
        expected,
        finalized_paths.len()
    );

    Ok(())
}
