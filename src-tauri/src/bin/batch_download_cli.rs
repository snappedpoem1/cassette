use cassette_core::db::Db;
use cassette_core::director::providers::{
    DeezerProvider, LocalArchiveProvider, QobuzProvider, RealDebridProvider, SlskdProvider,
    UsenetProvider, YtDlpProvider,
};
use cassette_core::director::{
    AcquisitionStrategy, Director, DirectorConfig, DirectorTaskResult, DuplicatePolicy,
    NormalizedTrack, Provider, ProviderPolicy, QualityPolicy, RetryPolicy, TempRecoveryPolicy,
    TrackTask, TrackTaskSource,
};
use cassette_core::director::ProviderError;
use cassette_core::director::models::{ProviderHealthStatus, ProviderSearchRecord};
use cassette_core::metadata::MetadataService;
use cassette_core::sources::{RemoteProviderConfig, SlskdConnectionConfig};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone)]
struct ProviderReadiness {
    provider_id: String,
    display_name: String,
    status: String,
    detail: String,
}

fn print_provider_readiness(readiness: &[ProviderReadiness]) {
    println!("\n=== Provider Readiness ===");
    for item in readiness {
        println!(
            "  {:<14} {:<12} {} ({})",
            item.provider_id, item.status, item.detail, item.display_name
        );
    }
}

fn classify_failure(result: &DirectorTaskResult) -> String {
    if result
        .attempts
        .iter()
        .any(|attempt| attempt.outcome.contains("auth failed"))
    {
        return "auth_failed".to_string();
    }
    if result
        .attempts
        .iter()
        .any(|attempt| {
            let outcome = attempt.outcome.to_ascii_lowercase();
            outcome.contains("too_many_requests")
                || outcome.contains("rate limited")
                || outcome.contains("429")
        })
    {
        return "rate_limited".to_string();
    }
    if result
        .attempts
        .iter()
        .any(|attempt| attempt.outcome.contains("os error 32"))
    {
        return "file_locked".to_string();
    }
    if result
        .provider_searches
        .iter()
        .any(|record| record.outcome.contains("cooldown") || record.outcome == "busy")
    {
        return "provider_busy".to_string();
    }
    if result
        .candidate_records
        .iter()
        .any(|record| record.outcome == "validation_failed")
    {
        return "validation_failed".to_string();
    }
    "provider_exhausted".to_string()
}

fn latest_failure_provider(searches: &[ProviderSearchRecord]) -> Option<String> {
    searches
        .iter()
        .rev()
        .find(|record| record.error.is_some() || record.outcome != "candidates_found")
        .map(|record| record.provider_id.clone())
}

fn usenet_is_configured(
    api_key: Option<&str>,
    sabnzbd_url: Option<&str>,
    sabnzbd_api_key: Option<&str>,
) -> bool {
    api_key.is_some() && sabnzbd_url.is_some() && sabnzbd_api_key.is_some()
}

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
    let bar: String = "#".repeat(filled) + &"-".repeat(bar_width - filled);

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
        "\r[{bar}] {done}/{total} {pct}%  +{finalized} -{failed} ={skipped}  {rate:.1}/min  {eta}\x1b[K"
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

fn normalize_task_component(value: &str) -> String {
    value
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|ch| if ch.is_alphanumeric() { ch } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn album_track_task_key(
    artist: &str,
    album: &str,
    disc_number: u32,
    track_number: u32,
    title: &str,
) -> String {
    format!(
        "spotify-album-track::{}::{}::{}::{:02}::{}",
        artist.to_ascii_lowercase(),
        album.to_ascii_lowercase(),
        disc_number,
        track_number,
        normalize_task_component(title),
    )
}

async fn resolve_album_track_tasks(
    metadata: &MetadataService,
    artist: &str,
    album: &str,
) -> Result<Vec<TrackTask>, Box<dyn std::error::Error>> {
    let release_with_tracks = metadata.resolve_release_with_tracks(artist, album).await?;
    if release_with_tracks.tracks.is_empty() {
        return Err(format!("No tracks found for {artist} - {album}").into());
    }
    let release_title = release_with_tracks.release.title.clone();
    let release_year = release_with_tracks.release.year;

    let mut tasks = release_with_tracks
        .tracks
        .into_iter()
        .map(|track| TrackTask {
            task_id: album_track_task_key(
                artist,
                album,
                track.disc_number,
                track.track_number,
                &track.title,
            ),
            source: TrackTaskSource::SpotifyHistory,
            target: NormalizedTrack {
                spotify_track_id: None,
                source_playlist: None,
                artist: if track.artist.trim().is_empty() {
                    artist.to_string()
                } else {
                    track.artist
                },
                album_artist: Some(artist.to_string()),
                title: track.title,
                album: Some(release_title.clone()),
                track_number: Some(track.track_number),
                disc_number: Some(track.disc_number),
                year: release_year,
                duration_secs: if track.duration_ms > 0 {
                    Some(track.duration_ms as f64 / 1000.0)
                } else {
                    None
                },
                isrc: None,
            },
            strategy: AcquisitionStrategy::DiscographyBatch,
        })
        .collect::<Vec<_>>();
    tasks.sort_by_key(|task| {
        (
            task.target.disc_number.unwrap_or(0),
            task.target.track_number.unwrap_or(0),
            task.target.title.to_ascii_lowercase(),
        )
    });
    Ok(tasks)
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
        let parts: Vec<&str> = task_id.splitn(3, "::").collect();
        if parts.len() != 3 {
            continue;
        }
        let (artist, track_title) = (parts[1], parts[2]);

        print!("  Looking up: {} - {}... ", artist, track_title);
        match mb.find_parent_album(artist, track_title).await {
            Ok(Some(release)) => {
                let parent_album = &release.title;
                let _ = db.upsert_spotify_album_history(artist, parent_album, 60_000, 1);
                println!(
                    "-> {} ({})",
                    parent_album,
                    release.release_group_type.as_deref().unwrap_or("?")
                );
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
        .map(|n: usize| if n == 0 { usize::MAX } else { n })
        .unwrap_or(250);

    let db_path = app_db_path()?;
    println!("DB: {}", db_path.display());

    let db = Db::open(&db_path).map_err(|e| e.to_string())?;

    if resolve_singles {
        return resolve_failed_singles(&db).await;
    }

    let library_base = read_setting(&db, "library_base").unwrap_or_else(|| "A:\\Music".to_string());
    let staging_folder =
        read_setting(&db, "staging_folder").unwrap_or_else(|| "A:\\Staging".to_string());

    let missing_albums = db.get_missing_spotify_albums(limit + 100)?;
    let completed_keys = db.get_completed_task_keys()?;

    println!(
        "Missing albums from Spotify: {} (completed tasks: {})",
        missing_albums.len(),
        completed_keys.len()
    );

    // Resolve album tracklists with a shared MetadataService (single rate limiter).
    // MB allows 1 req/sec — the governor in MetadataService enforces this globally.
    // Spotify credentials are passed in so the fallback chain can use them if MB/iTunes fail.
    let mut tasks = Vec::<TrackTask>::new();
    let spotify_id = read_setting(&db, "spotify_client_id");
    let spotify_secret = read_setting(&db, "spotify_client_secret");
    let metadata = Arc::new(
        MetadataService::with_spotify(spotify_id, spotify_secret).map_err(|e| e.to_string())?,
    );

    let albums_to_resolve: Vec<_> = missing_albums
        .iter()
        .filter(|a| !a.artist.trim().is_empty() && !a.album.trim().is_empty())
        .map(|a| (a.artist.trim().to_string(), a.album.trim().to_string()))
        .collect();

    println!("Resolving {} album tracklists (MB → iTunes → Spotify)...", albums_to_resolve.len());

    let mut skipped_albums = 0usize;
    for (artist, title) in &albums_to_resolve {
        if tasks.len() >= limit {
            break;
        }
        let result = resolve_album_track_tasks(&metadata, artist, title)
            .await
            .map_err(|e| e.to_string());
        match result {
            Ok(resolved_tasks) => {
                let new_count = resolved_tasks.iter()
                    .filter(|t| !completed_keys.contains(&t.task_id))
                    .count();
                if new_count > 0 {
                    print!("\r\x1b[K  {} - {}: {} tracks", artist, title, new_count);
                    let _ = std::io::Write::flush(&mut std::io::stdout());
                }
                for task in resolved_tasks {
                    if tasks.len() >= limit {
                        break;
                    }
                    if completed_keys.contains(&task.task_id) {
                        continue;
                    }
                    tasks.push(task);
                }
            }
            Err(error) => {
                print!("\r\x1b[K");
                println!("  Skip {artist} - {title}: {error}");
                skipped_albums += 1;
            }
        }
    }
    print!("\r\x1b[K");

    // Results already collected above in the sequential loop.
    if skipped_albums > 0 {
        println!("  ({skipped_albums} albums skipped — all metadata sources exhausted)");
    }

    println!("Will submit {} tasks (limit: {limit})", tasks.len());

    if dry_run {
        println!("\n=== DRY RUN - showing first 20 tasks ===");
        for (i, task) in tasks.iter().enumerate().take(20) {
            println!(
                "  [{:>3}] {} - {} - {}  (key: {})",
                i + 1,
                task.target.artist,
                task.target.album.clone().unwrap_or_default(),
                task.target.title,
                task.task_id
            );
        }
        if tasks.len() > 20 {
            println!("  ... and {} more", tasks.len() - 20);
        }
        println!("\nRun without --dry-run to submit these to the Director pipeline.");
        return Ok(());
    }

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
        worker_concurrency: 24,
        provider_timeout_secs: 120,
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
            ProviderPolicy {
                provider_id: "qobuz".to_string(),
                max_concurrency: 8,
            },
            ProviderPolicy {
                provider_id: "deezer".to_string(),
                max_concurrency: 8,
            },
            ProviderPolicy {
                provider_id: "slskd".to_string(),
                max_concurrency: 4,
            },
            ProviderPolicy {
                provider_id: "usenet".to_string(),
                max_concurrency: 2,
            },
            ProviderPolicy {
                provider_id: "local_archive".to_string(),
                max_concurrency: 4,
            },
            ProviderPolicy {
                provider_id: "yt_dlp".to_string(),
                max_concurrency: 4,
            },
            ProviderPolicy {
                provider_id: "real_debrid".to_string(),
                max_concurrency: 6,
            },
        ],
        staging_root: PathBuf::from(&staging_folder),
        ..DirectorConfig::default()
    };

    let rd_key = read_setting(&db, "real_debrid_key")
        .or_else(|| std::env::var("REAL_DEBRID_KEY").ok())
        .filter(|k| !k.trim().is_empty());

    let mut providers: Vec<Arc<dyn Provider>> = vec![
        Arc::new(SlskdProvider::new(
            slskd_connection,
            vec![PathBuf::from(&staging_folder), PathBuf::from(&library_base)],
        )),
        Arc::new(QobuzProvider::new(remote_config.clone())),
        Arc::new(DeezerProvider::new(remote_config.clone())),
        Arc::new(LocalArchiveProvider::new(config.local_search_roots.clone())),
        Arc::new(YtDlpProvider::new("yt-dlp")),
    ];
    let mut readiness = Vec::<ProviderReadiness>::new();
    let usenet_api_key = read_setting(&db, "nzbgeek_api_key");
    let usenet_sab_url = read_setting(&db, "sabnzbd_url");
    let usenet_sab_key = read_setting(&db, "sabnzbd_api_key");
    if usenet_is_configured(
        usenet_api_key.as_deref(),
        usenet_sab_url.as_deref(),
        usenet_sab_key.as_deref(),
    ) {
        providers.push(Arc::new(UsenetProvider {
            api_key: usenet_api_key,
            sabnzbd_url: usenet_sab_url,
            sabnzbd_api_key: usenet_sab_key,
            scan_roots: vec![PathBuf::from(&staging_folder), PathBuf::from(&library_base)],
        }));
    } else {
        readiness.push(ProviderReadiness {
            provider_id: "usenet".to_string(),
            display_name: "Usenet".to_string(),
            status: "unavailable".to_string(),
            detail: "missing nzbgeek and/or SABnzbd configuration".to_string(),
        });
    }
    if let Some(key) = rd_key {
        providers.push(Arc::new(RealDebridProvider::new(key)));
    } else {
        readiness.push(ProviderReadiness {
            provider_id: "real_debrid".to_string(),
            display_name: "Real-Debrid".to_string(),
            status: "unavailable".to_string(),
            detail: "missing REAL_DEBRID_KEY / real_debrid_key".to_string(),
        });
    }

    let mut usable_providers = Vec::<Arc<dyn Provider>>::new();
    for provider in providers {
        let descriptor = provider.descriptor();
        match provider.health_check().await {
            Ok(state) => {
                let status = match state.status {
                    ProviderHealthStatus::Healthy => "healthy",
                    ProviderHealthStatus::Unknown => "unknown",
                    ProviderHealthStatus::Down => "down",
                };
                readiness.push(ProviderReadiness {
                    provider_id: descriptor.id.clone(),
                    display_name: descriptor.display_name.clone(),
                    status: status.to_string(),
                    detail: state.message.unwrap_or_else(|| "ready".to_string()),
                });
                usable_providers.push(provider);
            }
            Err(error @ ProviderError::AuthFailed { .. })
            | Err(error @ ProviderError::NotFound { .. }) => {
                readiness.push(ProviderReadiness {
                    provider_id: descriptor.id.clone(),
                    display_name: descriptor.display_name.clone(),
                    status: "unavailable".to_string(),
                    detail: error.to_string(),
                });
            }
            Err(error) => {
                readiness.push(ProviderReadiness {
                    provider_id: descriptor.id.clone(),
                    display_name: descriptor.display_name.clone(),
                    status: "cooldown".to_string(),
                    detail: error.to_string(),
                });
                usable_providers.push(provider);
            }
        }
    }
    readiness.sort_by(|left, right| left.provider_id.cmp(&right.provider_id));
    print_provider_readiness(&readiness);
    if usable_providers.is_empty() {
        return Err("no usable providers remain after preflight".into());
    }

    let handle = Director::new(config, usable_providers).start();
    let mut result_rx = handle.subscribe_results();

    let total = tasks.len();
    let mut submitted_tasks = HashMap::<String, TrackTask>::new();
    println!("\nSubmitting {total} tasks to Director...");
    for (i, task) in tasks.iter().enumerate() {
        submitted_tasks.insert(task.task_id.clone(), task.clone());

        handle
            .submitter
            .submit(task.clone())
            .await
            .map_err(|e| e.to_string())?;

        if (i + 1) % 50 == 0 {
            println!("  Submitted {}/{}", i + 1, total);
        }
    }
    println!("All {total} tasks submitted. Waiting for results...\n");

    let mut finalized = 0usize;
    let mut failed = 0usize;
    let mut skipped = 0usize;
    let mut finalized_by_provider = HashMap::<String, usize>::new();
    let mut failed_by_class = HashMap::<String, usize>::new();
    let mut failed_by_provider = HashMap::<String, usize>::new();
    let start = std::time::Instant::now();

    print_progress(0, total, 0, 0, 0, 0, 0.0);

    while finalized + failed + skipped < total {
        match result_rx.recv().await {
            Ok(result) => {
                let request = submitted_tasks.get(&result.task_id);
                let _ = db.save_director_task_result(&result, request);
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
                        *finalized_by_provider.entry(provider.to_string()).or_default() += 1;
                        print!("\r\x1b[K");
                        println!("  +  {} ({})", result.task_id, provider);
                    }
                    cassette_core::director::FinalizedTrackDisposition::AlreadyPresent => {
                        skipped += 1;
                        print!("\r\x1b[K");
                        println!("  =  {} (already present)", result.task_id);
                    }
                    cassette_core::director::FinalizedTrackDisposition::Failed => {
                        failed += 1;
                        let failure_class = classify_failure(&result);
                        *failed_by_class.entry(failure_class.clone()).or_default() += 1;
                        if let Some(provider) = latest_failure_provider(&result.provider_searches) {
                            *failed_by_provider.entry(provider).or_default() += 1;
                        }
                        let err = result.error.as_deref().unwrap_or("unknown");
                        let short_err = if err.len() > 60 { &err[..60] } else { err };
                        print!("\r\x1b[K");
                        println!("  -  {} - {} [{}]", result.task_id, short_err, failure_class);
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
    print!("\r\x1b[K");

    let elapsed = start.elapsed().as_secs();
    println!("\n=== COMPLETE ===");
    println!("Finalized: {finalized} | Failed: {failed} | Skipped: {skipped}");
    println!("Total time: {elapsed}s");
    if !finalized_by_provider.is_empty() {
        println!("\nFinalized by provider:");
        let mut rows = finalized_by_provider.into_iter().collect::<Vec<_>>();
        rows.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
        for (provider, count) in rows {
            println!("  {provider:<14} {count}");
        }
    }
    if !failed_by_class.is_empty() {
        println!("\nFailed by class:");
        let mut rows = failed_by_class.into_iter().collect::<Vec<_>>();
        rows.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
        for (class, count) in rows {
            println!("  {class:<18} {count}");
        }
    }
    if !failed_by_provider.is_empty() {
        println!("\nFailed by provider:");
        let mut rows = failed_by_provider.into_iter().collect::<Vec<_>>();
        rows.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
        for (provider, count) in rows {
            println!("  {provider:<14} {count}");
        }
    }

    drop(handle);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usenet_preflight_requires_full_config() {
        assert!(!usenet_is_configured(None, Some("http://sab"), Some("key")));
        assert!(!usenet_is_configured(Some("api"), None, Some("key")));
        assert!(!usenet_is_configured(Some("api"), Some("http://sab"), None));
        assert!(usenet_is_configured(
            Some("api"),
            Some("http://sab"),
            Some("key")
        ));
    }
}
