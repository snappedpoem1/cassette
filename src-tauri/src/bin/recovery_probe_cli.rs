use cassette_core::db::Db;
use cassette_core::director::{
    providers::LocalArchiveProvider, AcquisitionStrategy, Director, DirectorConfig,
    DirectorTaskResult, DuplicatePolicy, FinalizedTrackDisposition, NormalizedTrack,
    ProviderPolicy, QualityPolicy, RetryPolicy, TempRecoveryPolicy, TrackTask, TrackTaskSource,
};
use cassette_core::provider_settings::DownloadConfig;
use cassette_lib::state::AppState;
use std::path::Path;
use std::sync::Arc;
use tokio::time::{sleep, timeout, Duration, Instant};

fn write_probe_wav(path: &Path, duration_secs: u32) -> Result<(), String> {
    let sample_rate = 44_100_u32;
    let channels = 1_u16;
    let bits_per_sample = 16_u16;
    let duration_samples = sample_rate * duration_secs;
    let data_len = duration_samples * u32::from(channels) * u32::from(bits_per_sample / 8);
    let byte_rate = sample_rate * u32::from(channels) * u32::from(bits_per_sample / 8);
    let block_align = channels * (bits_per_sample / 8);
    let riff_len = 36 + data_len;

    let mut bytes = Vec::<u8>::with_capacity((44 + data_len) as usize);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&riff_len.to_le_bytes());
    bytes.extend_from_slice(b"WAVE");
    bytes.extend_from_slice(b"fmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&channels.to_le_bytes());
    bytes.extend_from_slice(&sample_rate.to_le_bytes());
    bytes.extend_from_slice(&byte_rate.to_le_bytes());
    bytes.extend_from_slice(&block_align.to_le_bytes());
    bytes.extend_from_slice(&bits_per_sample.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_len.to_le_bytes());
    bytes.resize(bytes.len() + data_len as usize, 0_u8);

    std::fs::write(path, bytes).map_err(|error| error.to_string())
}

fn make_task(task_id: &str, artist: &str, title: &str, album: &str) -> TrackTask {
    TrackTask {
        task_id: task_id.to_string(),
        source: TrackTaskSource::Manual,
        desired_track_id: None,
        source_operation_id: None,
        target: NormalizedTrack {
            spotify_track_id: None,
            source_playlist: None,
            artist: artist.to_string(),
            album_artist: Some(artist.to_string()),
            title: title.to_string(),
            album: Some(album.to_string()),
            track_number: Some(1),
            disc_number: Some(1),
            year: Some(2024),
            duration_secs: Some(35.0),
            isrc: None,
            musicbrainz_recording_id: None,
            musicbrainz_release_id: None,
            canonical_artist_id: None,
            canonical_release_id: None,
        },
        strategy: AcquisitionStrategy::Standard,
    }
}

fn seed_probe_db(db_path: &Path, library_root: &Path, staging_root: &Path) -> Result<(), String> {
    let db = Db::open(db_path).map_err(|error| error.to_string())?;
    db.set_setting("library_base", &library_root.to_string_lossy())
        .map_err(|error| error.to_string())?;
    db.set_setting("staging_folder", &staging_root.to_string_lossy())
        .map_err(|error| error.to_string())?;

    let resumable = make_task(
        "recovery-probe::resume",
        "Recovery Artist",
        "Recovery Song",
        "Recovery Album",
    );
    db.upsert_director_pending_task(&resumable, "Queued")
        .map_err(|error| error.to_string())?;

    let stale = make_task(
        "recovery-probe::stale",
        "Stale Artist",
        "Stale Song",
        "Stale Album",
    );
    db.upsert_director_pending_task(&stale, "Queued")
        .map_err(|error| error.to_string())?;
    std::thread::sleep(Duration::from_secs(1));
    db.save_director_task_result(&DirectorTaskResult {
        task_id: stale.task_id.clone(),
        disposition: FinalizedTrackDisposition::Cancelled,
        finalized: None,
        attempts: Vec::new(),
        error: Some("stale pending row after cancellation".to_string()),
        candidate_records: Vec::new(),
        provider_searches: Vec::new(),
    }, Some(&stale))
    .map_err(|error| error.to_string())?;

    Ok(())
}

fn build_probe_director(
    library_root: &Path,
    staging_root: &Path,
    db_path: &Path,
) -> cassette_core::director::DirectorHandle {
    let config = DirectorConfig {
        library_root: library_root.to_path_buf(),
        temp_root: staging_root.join(".director-temp"),
        runtime_db_path: Some(db_path.to_path_buf()),
        local_search_roots: vec![staging_root.to_path_buf()],
        worker_concurrency: 1,
        provider_timeout_secs: 5,
        retry_policy: RetryPolicy {
            max_attempts_per_provider: 1,
            base_backoff_millis: 100,
        },
        quality_policy: QualityPolicy {
            minimum_duration_secs: 30.0,
            max_duration_delta_secs: Some(2.0),
            preferred_extensions: vec!["wav".to_string()],
        },
        duplicate_policy: DuplicatePolicy::ReplaceIfBetter,
        temp_recovery: TempRecoveryPolicy {
            stale_after_hours: 24,
            quarantine_failures: true,
        },
        provider_policies: vec![ProviderPolicy {
            provider_id: "local_archive".to_string(),
            max_concurrency: 1,
        }],
        staging_root: staging_root.to_path_buf(),
        ..DirectorConfig::default()
    };

    Director::new(
        config,
        vec![Arc::new(LocalArchiveProvider::new(vec![
            staging_root.to_path_buf(),
        ]))],
    )
    .start()
}

#[tokio::main]
async fn main() -> Result<(), String> {
    let root = std::env::temp_dir().join(format!(
        "cassette-recovery-probe-{}",
        uuid::Uuid::new_v4()
    ));
    let library_root = root.join("library");
    let staging_root = root.join("staging");
    let db_path = root.join("cassette.db");

    std::fs::create_dir_all(&library_root).map_err(|error| error.to_string())?;
    std::fs::create_dir_all(&staging_root).map_err(|error| error.to_string())?;
    write_probe_wav(
        &staging_root.join("Recovery Artist - Recovery Song.wav"),
        35,
    )?;
    seed_probe_db(&db_path, &library_root, &staging_root)?;

    let download_config = DownloadConfig {
        library_base: library_root.to_string_lossy().to_string(),
        staging_folder: staging_root.to_string_lossy().to_string(),
        ..DownloadConfig::default()
    };
    let director_handle = build_probe_director(&library_root, &staging_root, &db_path);
    let state = AppState::new_with_director(&db_path, director_handle, download_config, None)
        .map_err(|error| error.to_string())?;
    let mut result_rx = state
        .director_handle
        .lock()
        .map_err(|error| error.to_string())?
        .subscribe_results();

    {
        let jobs = state
            .download_jobs
            .lock()
            .map_err(|error| error.to_string())?;
        if !jobs.contains_key("recovery-probe::resume") {
            return Err("startup recovery did not restore the resumable job".to_string());
        }
        if jobs.contains_key("recovery-probe::stale") {
            return Err("startup recovery resurrected a stale cancelled task".to_string());
        }
    }

    let recovered_result = timeout(Duration::from_secs(20), async {
        loop {
            let result = result_rx.recv().await.map_err(|error| error.to_string())?;
            if result.task_id == "recovery-probe::resume" {
                break Ok::<DirectorTaskResult, String>(result);
            }
        }
    })
    .await
    .map_err(|_| {
        let jobs = state
            .download_jobs
            .lock()
            .map(|jobs| format!("{jobs:#?}"))
            .unwrap_or_else(|error| format!("failed to lock jobs: {error}"));
        let pending = state
            .db
            .lock()
            .map_err(|error| error.to_string())
            .and_then(|db| db.get_pending_director_tasks().map_err(|error| error.to_string()))
            .map(|tasks| format!("{tasks:#?}"))
            .unwrap_or_else(|error| format!("failed to read pending tasks: {error}"));
        let library_files = walkdir::WalkDir::new(&library_root)
            .into_iter()
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path().display().to_string())
            .collect::<Vec<_>>();
        format!(
            "timed out waiting for resumed task result\njobs={jobs}\npending={pending}\nlibrary_files={library_files:#?}"
        )
    })??;

    if !matches!(recovered_result.disposition, FinalizedTrackDisposition::Finalized) {
        return Err(format!(
            "recovered task reached {:?} instead of Finalized",
            recovered_result.disposition
        ));
    }

    let expected_path = library_root
        .join("Recovery Artist")
        .join("Recovery Album")
        .join("01 - Recovery Song.wav");
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if expected_path.exists() {
            break;
        }
        sleep(Duration::from_millis(100)).await;
    }

    if !expected_path.exists() {
        let jobs = state
            .download_jobs
            .lock()
            .map_err(|error| error.to_string())?;
        return Err(format!(
            "recovered task finalized but expected path is missing: {} (job={:?})",
            expected_path.display(),
            jobs.get("recovery-probe::resume")
        ));
    }

    {
        let db = state.db.lock().map_err(|error| error.to_string())?;
        let pending = db
            .get_pending_director_tasks()
            .map_err(|error| error.to_string())?;
        if !pending.is_empty() {
            return Err(format!(
                "pending rows were not fully drained after recovery: {} remaining",
                pending.len()
            ));
        }
    }

    {
        let jobs = state
            .download_jobs
            .lock()
            .map_err(|error| error.to_string())?;
        let Some(job) = jobs.get("recovery-probe::resume") else {
            return Err("recovered job disappeared from the visible job map".to_string());
        };
        if !matches!(job.status, cassette_core::models::DownloadStatus::Done) {
            return Err(format!(
                "recovered job did not reach Done state; saw {:?}",
                job.status
            ));
        }
    }

    println!("Recovery probe OK");
    println!("  db: {}", db_path.display());
    println!("  resumed: recovery-probe::resume");
    println!("  filtered: recovery-probe::stale");
    println!("  finalized: {}", expected_path.display());

    Ok(())
}
