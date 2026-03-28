#[path = "../state.rs"]
mod state;

use cassette_core::db::Db;
use cassette_core::director::{
    AcquisitionStrategy, Director, DirectorConfig, DuplicatePolicy, NormalizedTrack, ProviderPolicy,
    QualityPolicy, RetryPolicy, TempRecoveryPolicy, TrackTask, TrackTaskSource,
};
use cassette_core::director::providers::{
    DeezerProvider, LocalArchiveProvider, QobuzProvider, RealDebridProvider, SlskdProvider,
    UsenetProvider, YtDlpProvider,
};
use cassette_core::metadata::MetadataService;
use cassette_core::sources::{RemoteProviderConfig, SlskdConnectionConfig};
use std::path::PathBuf;
use std::sync::Arc;

fn print_progress(
    done: usize,
    total: usize,
    finalized: usize,
    failed: usize,
    skipped: usize,
    elapsed: u64,
    rate: f64,
) {
    use std::io::Write;
    let pct = if total > 0 { done * 100 / total } else { 0 };
    let bar_width = 36usize;
    let filled = if total > 0 { done * bar_width / total } else { 0 };
    let bar: String = "█".repeat(filled) + &"░".repeat(bar_width - filled);

    let eta = if rate > 0.0 && done < total {
        let secs_left = ((total - done) as f64 / rate * 60.0) as u64;
        if secs_left < 3600 {
            format!("ETA {}m{:02}s", secs_left / 60, secs_left % 60)
        } else {
            format!("ETA {}h{:02}m", secs_left / 3600, (secs_left % 3600) / 60)
        }
    } else if done >= total {
        format!("{}s total", elapsed)
    } else {
        "ETA --:--".to_string()
    };

    print!(
        "\r[{bar}] {done}/{total} {pct}%  ✓{finalized} ✗{failed} ⤏{skipped}  {rate:.1}/min  {eta}\x1b[K"
    );
    let _ = std::io::stdout().flush();
}

fn app_db_path() -> Result<PathBuf, String> {
    let app_data = std::env::var("APPDATA").map_err(|e| e.to_string())?;
    Ok(PathBuf::from(app_data)
        .join("dev.cassette.app")
        .join("cassette.db"))
}

fn read_setting(db: &Db, key: &str) -> Option<String> {
    db.get_setting(key)
        .ok()
        .flatten()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

/// For each Failed task where album == title (likely a single), query MusicBrainz for the
/// parent album and insert it into spotify_album_history so the next batch run downloads it.
async fn resolve_failed_singles(db: &Db) -> Result<(), Box<dyn std::error::Error>> {
    let failed = db.get_failed_task_ids()?;
    if failed.is_empty() {
        println!("No failed tasks to resolve.");
        return Ok(());
    }
    println!("Resolving {} failed tasks via MusicBrainz...\n", failed.len());

    let mb = MetadataService::new()?;
    let mut resolved = 0usize;
    let mut unresolved = 0usize;

    for task_id in &failed {
        // task_id format: "spotify-batch::{artist}::{album}"
        let parts: Vec<&str> = task_id.splitn(3, "::").collect();
        if parts.len() != 3 {
            continue;
        }
        let (artist, track_title) = (parts[1], parts[2]);

        print!("  Looking up: {} — {}... ", artist, track_title);
        match mb.find_parent_album(artist, track_title).await {
            Ok(Some(release)) => {
                let parent_album = &release.title;
                // Insert into spotify_album_history — use 1 play_count so it qualifies for download
                let _ = db.upsert_spotify_album_history(artist, parent_album, 60_000, 1);
                println!("→ {} ({})", parent_album, release.release_group_type.as_deref().unwrap_or("?"));
                resolved += 1;
            }
            Ok(None) => {
                println!("not found");
                unresolved += 1;
            }
            Err(e) => {
                println!("error: {e}");
                unresolved += 1;
            }
        }
    }

    println!("\nResolved: {resolved} | Not found: {unresolved}");
    println!("Run without --resolve-singles to download the newly added albums.");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let args: Vec<String> = std::env::args().collect();
    let dry_run = args.iter().any(|a| a == "--dry-run");
    let resolve_singles = args.iter().any(|a| a == "--resolve-singles");
    let limit: usize = args
        .windows(2)
        .find(|w| w[0] == "--limit")
        .and_then(|w| w[1].parse().ok())
        .unwrap_or(250);

    let db_path = app_db_path()?;
    println!("DB: {}", db_path.display());

    let db = Db::open(&db_path).map_err(|e| e.to_string())?;

    // --resolve-singles: use MusicBrainz to find parent albums for failed single-track tasks,
    // then insert the resolved albums into spotify_album_history so the next run downloads them.
    if resolve_singles {
        return resolve_failed_singles(&db).await;
    }

    let library_base = read_setting(&db, "library_base").unwrap_or_else(|| "A:\\Music".to_string());
    let staging_folder =
        read_setting(&db, "staging_folder").unwrap_or_else(|| "A:\\Staging".to_string());

    // Fetch missing albums and completed task keys
    let missing_albums = db.get_missing_spotify_albums(limit + 100)?;
    let completed_keys = db.get_completed_task_keys()?;

    println!(
        "Missing albums from Spotify: {} (completed tasks: {})",
        missing_albums.len(),
        completed_keys.len()
    );

    // Filter and build task list
    let mut tasks: Vec<(String, String, String)> = Vec::new();
    for album in &missing_albums {
        if tasks.len() >= limit {
            break;
        }
        let artist = album.artist.trim();
        let title = album.album.trim();
        if artist.is_empty() || title.is_empty() {
            continue;
        }
        let task_key = format!(
            "spotify-batch::{}::{}",
            artist.to_ascii_lowercase(),
            title.to_ascii_lowercase()
        );
        if completed_keys.contains(&task_key) {
            continue;
        }
        tasks.push((task_key, artist.to_string(), title.to_string()));
    }

    println!("Will submit {} tasks (limit: {limit})", tasks.len());

    if dry_run {
        println!("\n=== DRY RUN — showing first 20 tasks ===");
        for (i, (key, artist, title)) in tasks.iter().enumerate().take(20) {
            println!("  [{:>3}] {artist} — {title}  (key: {key})", i + 1);
        }
        if tasks.len() > 20 {
            println!("  ... and {} more", tasks.len() - 20);
        }
        println!("\nRun without --dry-run to submit these to the Director pipeline.");
        return Ok(());
    }

    // Build Director with same config as AppState
    let remote_config = RemoteProviderConfig {
        qobuz_email: read_setting(&db, "qobuz_email"),
        qobuz_password: read_setting(&db, "qobuz_password"),
        qobuz_password_hash: read_setting(&db, "qobuz_password_hash"),
        qobuz_app_id: read_setting(&db, "qobuz_app_id"),
        qobuz_app_secret: read_setting(&db, "qobuz_app_secret"),
        qobuz_user_auth_token: read_setting(&db, "qobuz_user_auth_token"),
        qobuz_secrets: read_setting(&db, "qobuz_secrets"),
        deezer_arl: read_setting(&db, "deezer_arl"),
        spotify_client_id: read_setting(&db, "spotify_client_id"),
        spotify_client_secret: read_setting(&db, "spotify_client_secret"),
        spotify_access_token: read_setting(&db, "spotify_access_token"),
    };

    let slskd_connection = SlskdConnectionConfig {
        url: read_setting(&db, "slskd_url").unwrap_or_else(|| "http://localhost:5030".to_string()),
        username: read_setting(&db, "slskd_user").unwrap_or_else(|| "slskd".to_string()),
        password: read_setting(&db, "slskd_pass").unwrap_or_else(|| "slskd".to_string()),
        api_key: read_setting(&db, "slskd_api_key"),
    };

    let config = DirectorConfig {
        library_root: PathBuf::from(&library_base),
        temp_root: PathBuf::from(&staging_folder).join(".director-temp"),
        local_search_roots: vec![PathBuf::from(&staging_folder)],
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
        staging_root: PathBuf::from(&staging_folder),
        ..DirectorConfig::default()
    };

    let rd_key = read_setting(&db, "real_debrid_key")
        .or_else(|| std::env::var("REAL_DEBRID_KEY").ok())
        .filter(|k| !k.trim().is_empty());

    let mut providers: Vec<Arc<dyn cassette_core::director::Provider>> = vec![
        Arc::new(SlskdProvider::new(
            slskd_connection,
            vec![PathBuf::from(&staging_folder), PathBuf::from(&library_base)],
        )),
        Arc::new(QobuzProvider::new(remote_config.clone())),
        Arc::new(DeezerProvider::new(remote_config.clone())),
        Arc::new(UsenetProvider {
            api_key: read_setting(&db, "nzbgeek_api_key"),
            sabnzbd_url: read_setting(&db, "sabnzbd_url"),
            sabnzbd_api_key: read_setting(&db, "sabnzbd_api_key"),
            scan_roots: vec![PathBuf::from(&staging_folder), PathBuf::from(&library_base)],
        }),
        Arc::new(LocalArchiveProvider::new(config.local_search_roots.clone())),
        Arc::new(YtDlpProvider::new("yt-dlp")),
    ];
    if let Some(key) = rd_key {
        println!("Real-Debrid: enabled");
        providers.push(Arc::new(RealDebridProvider::new(key)));
    } else {
        println!("Real-Debrid: not configured (set REAL_DEBRID_KEY or real_debrid_key in DB)");
    }

    let handle = Director::new(config, providers).start();
    let mut result_rx = handle.subscribe_results();

    // Submit all tasks
    let total = tasks.len();
    println!("\nSubmitting {total} tasks to Director...");
    for (i, (task_key, artist, title)) in tasks.iter().enumerate() {
        let task = TrackTask {
            task_id: task_key.clone(),
            source: TrackTaskSource::SpotifyHistory,
            target: NormalizedTrack {
                spotify_track_id: None,
                source_playlist: None,
                artist: artist.clone(),
                album_artist: Some(artist.clone()),
                title: title.clone(),
                album: Some(title.clone()),
                track_number: None,
                disc_number: None,
                year: None,
                duration_secs: None,
                isrc: None,
            },
            strategy: AcquisitionStrategy::DiscographyBatch,
        };

        handle
            .submitter
            .submit(task)
            .await
            .map_err(|e| e.to_string())?;

        if (i + 1) % 50 == 0 {
            println!("  Submitted {}/{total}", i + 1);
        }
    }
    println!("All {total} tasks submitted. Waiting for results...\n");

    // Monitor results
    let mut finalized = 0usize;
    let mut failed = 0usize;
    let mut skipped = 0usize;
    let start = std::time::Instant::now();

    // Print initial empty bar
    print_progress(0, total, 0, 0, 0, 0, 0.0);

    while finalized + failed + skipped < total {
        match result_rx.recv().await {
            Ok(result) => {
                // Persist to DB
                let _ = db.save_director_task_result(&result);
                if let Some(ref finalized_track) = result.finalized {
                    if let Ok(track) =
                        cassette_core::library::read_track_metadata(&finalized_track.path)
                    {
                        let _ = db.upsert_track(&track);
                    }
                }

                let elapsed = start.elapsed().as_secs();
                let done = finalized + failed + skipped + 1;
                let rate = if elapsed > 0 {
                    (finalized + skipped) as f64 * 60.0 / elapsed as f64
                } else {
                    0.0
                };

                match result.disposition {
                    cassette_core::director::FinalizedTrackDisposition::Finalized => {
                        finalized += 1;
                        let provider = result
                            .finalized
                            .as_ref()
                            .map(|f| f.provenance.selected_provider.as_str())
                            .unwrap_or("?");
                        // Clear bar line, print result, reprint bar
                        print!("\r\x1b[K");
                        println!("  ✓  {} ({})", result.task_id, provider);
                    }
                    cassette_core::director::FinalizedTrackDisposition::AlreadyPresent => {
                        skipped += 1;
                        print!("\r\x1b[K");
                        println!("  ⤏  {} (already present)", result.task_id);
                    }
                    cassette_core::director::FinalizedTrackDisposition::Failed => {
                        failed += 1;
                        let err = result.error.as_deref().unwrap_or("unknown");
                        let short_err = if err.len() > 60 { &err[..60] } else { err };
                        print!("\r\x1b[K");
                        println!("  ✗  {} — {}", result.task_id, short_err);
                    }
                    _ => {
                        skipped += 1;
                    }
                }

                print_progress(done, total, finalized, failed, skipped, elapsed, rate);
            }
            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                print!("\r\x1b[K");
                eprintln!("  Warning: result listener lagged by {n} messages");
            }
            Err(_) => break,
        }
    }
    print!("\r\x1b[K"); // clear bar line before final summary

    let elapsed = start.elapsed().as_secs();
    println!("\n=== COMPLETE ===");
    println!("Finalized: {finalized} | Failed: {failed} | Skipped: {skipped}");
    println!("Total time: {elapsed}s");

    // Graceful shutdown
    drop(handle);

    Ok(())
}
