use crate::state::AppState;
use cassette_core::director::{
    AcquisitionStrategy, DirectorProgress, NormalizedTrack, TrackTask,
    TrackTaskSource,
};
use cassette_core::metadata::MetadataService;
use cassette_core::models::{
    AcquisitionQueueReport, DownloadArtistDiscography, DownloadJob, DownloadMetadataSearchResult,
    DownloadStatus,
};
use cassette_core::sources::{fetch_slskd_transfers, get_artist_discography as fetch_artist_discography, search_metadata as search_catalog_metadata, RemoteProviderConfig, SlskdConnectionConfig};
use serde_json::Value;
use tauri::State;
use uuid::Uuid;

#[tauri::command]
pub async fn start_download(
    state: State<'_, AppState>,
    artist: String,
    title: String,
    album: Option<String>,
) -> Result<String, String> {
    let id = format!("job-{}", Uuid::new_v4());
    let task = build_track_task(&id, &artist, &title, album.clone(), AcquisitionStrategy::Standard);
    let job = DownloadJob {
        id: id.clone(),
        query: format!(
            "{} {}{}",
            artist,
            title,
            album
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .map(|value| format!(" {value}"))
                .unwrap_or_default()
        )
        .trim()
        .to_string(),
        artist: artist.clone(),
        title: title.clone(),
        album: album.clone(),
        status: DownloadStatus::Queued,
        provider: None,
        progress: 0.0,
        error: None,
    };

    state
        .download_jobs
        .lock()
        .map_err(|error| error.to_string())?
        .insert(id.clone(), job);

    if let Err(error) = state.persist_pending_task(&task, DirectorProgress::Queued) {
        if let Ok(mut jobs) = state.download_jobs.lock() {
            jobs.remove(&id);
        }
        return Err(error.to_string());
    }

    state
        .director_submitter
        .submit(task)
        .await
        .map_err(|error| {
            let _ = state.delete_pending_task(&id);
            if let Ok(mut jobs) = state.download_jobs.lock() {
                jobs.remove(&id);
            }
            error.to_string()
        })?;

    Ok(id)
}

#[tauri::command]
pub async fn cancel_download(
    state: State<'_, AppState>,
    task_id: String,
) -> Result<bool, String> {
    let cancelled = state
        .cancel_download(task_id.as_str())
        .map_err(|error| error.to_string())?;
    Ok(cancelled)
}

#[tauri::command]
pub async fn start_album_downloads(
    state: State<'_, AppState>,
    albums: Vec<serde_json::Value>,
) -> Result<Vec<String>, String> {
    let mut job_ids = Vec::new();
    let completed_keys = {
        let db = state.db.lock().map_err(|error| error.to_string())?;
        db.get_completed_task_keys().map_err(|error| error.to_string())?
    };
    for album in albums {
        let artist = album
            .get("artist")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let title = album
            .get("title")
            .or_else(|| album.get("name"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        if artist.trim().is_empty() || title.trim().is_empty() {
            continue;
        }
        let queued = queue_album_tracks(
            state.clone(),
            artist.as_str(),
            title.as_str(),
            TrackTaskSource::Manual,
            AcquisitionStrategy::DiscographyBatch,
            &completed_keys,
        )
        .await?;
        job_ids.extend(queued);
    }
    Ok(job_ids)
}

#[tauri::command]
pub async fn start_discography_downloads(
    state: State<'_, AppState>,
    artist: String,
    artist_mbid: Option<String>,
    include_singles: Option<bool>,
    include_eps: Option<bool>,
    include_compilations: Option<bool>,
    max_albums: Option<usize>,
) -> Result<AcquisitionQueueReport, String> {
    let provider_config = load_remote_provider_config(&state)?;
    let discography =
        fetch_artist_discography(&provider_config, artist.as_str(), artist_mbid).await?;
    queue_discography_with_rules(
        state,
        "discography",
        discography,
        include_singles.unwrap_or(false),
        include_eps.unwrap_or(false),
        include_compilations.unwrap_or(false),
        max_albums.unwrap_or(50),
    )
    .await
}

#[tauri::command]
pub async fn start_artist_downloads(
    state: State<'_, AppState>,
    artist: String,
    artist_mbid: Option<String>,
    include_singles: Option<bool>,
    include_eps: Option<bool>,
    include_compilations: Option<bool>,
    max_albums: Option<usize>,
) -> Result<AcquisitionQueueReport, String> {
    let provider_config = load_remote_provider_config(&state)?;
    let discography =
        fetch_artist_discography(&provider_config, artist.as_str(), artist_mbid).await?;
    queue_discography_with_rules(
        state,
        "artist",
        discography,
        include_singles.unwrap_or(false),
        include_eps.unwrap_or(false),
        include_compilations.unwrap_or(false),
        max_albums.unwrap_or(50),
    )
    .await
}

#[tauri::command]
pub async fn build_library_acquisition_queue(
    state: State<'_, AppState>,
    artist_filter: Option<String>,
    limit: Option<usize>,
) -> Result<AcquisitionQueueReport, String> {
    let albums = state
        .db
        .lock()
        .map_err(|error| error.to_string())?
        .get_albums()
        .map_err(|error| error.to_string())?;

    let artist_filter = artist_filter
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase());
    let mut seen = std::collections::HashSet::<String>::new();
    let completed_keys = state
        .db
        .lock()
        .map_err(|error| error.to_string())?
        .get_completed_task_keys()
        .map_err(|error| error.to_string())?;
    let mut report = AcquisitionQueueReport {
        scope: "library_database".to_string(),
        requested: albums.len(),
        ..AcquisitionQueueReport::default()
    };

    for album in albums {
        if let Some(filter) = &artist_filter {
            if !album.artist.to_ascii_lowercase().contains(filter) {
                report.skipped += 1;
                continue;
            }
        }

        if report.job_ids.len() >= limit.unwrap_or(500) {
            report.notes.push("Queue limit reached.".to_string());
            break;
        }

        let title = album.title.trim();
        let artist = album.artist.trim();
        if artist.is_empty() || title.is_empty() {
            report.skipped += 1;
            continue;
        }

        let key = format!(
            "{}::{}",
            artist.to_ascii_lowercase(),
            title.to_ascii_lowercase()
        );
        if !seen.insert(key) {
            report.skipped += 1;
            continue;
        }

        match queue_album_tracks(
            state.clone(),
            artist,
            title,
            TrackTaskSource::Manual,
            AcquisitionStrategy::DiscographyBatch,
            &completed_keys,
        )
        .await
        {
            Ok(job_ids) if !job_ids.is_empty() => {
                report.queued += job_ids.len();
                report.job_ids.extend(job_ids);
            }
            Ok(_) => {
                report.skipped += 1;
            }
            Err(error) => {
                report.skipped += 1;
                report
                    .notes
                    .push(format!("{} - {}: {error}", album.artist, album.title));
            }
        }
    }

    Ok(report)
}

#[tauri::command]
pub async fn start_spotify_missing_batch(
    state: State<'_, AppState>,
    limit: Option<usize>,
) -> Result<AcquisitionQueueReport, String> {
    let limit = limit.unwrap_or(250);
    let (missing_albums, completed_keys) = {
        let db = state.db.lock().map_err(|error| error.to_string())?;
        let missing = db
            .get_missing_spotify_albums(limit + 100) // fetch extra to account for skips
            .map_err(|error| error.to_string())?;
        let completed = db
            .get_completed_task_keys()
            .map_err(|error| error.to_string())?;
        (missing, completed)
    };

    let mut report = AcquisitionQueueReport {
        scope: "spotify_missing_batch".to_string(),
        requested: missing_albums.len(),
        ..AcquisitionQueueReport::default()
    };

    for album in &missing_albums {
        if report.queued >= limit {
            report.notes.push("Batch limit reached.".to_string());
            break;
        }

        let artist = album.artist.trim();
        let title = album.album.trim();
        if artist.is_empty() || title.is_empty() {
            report.skipped += 1;
            continue;
        }

        match queue_album_tracks(
            state.clone(),
            artist,
            title,
            TrackTaskSource::SpotifyHistory,
            AcquisitionStrategy::DiscographyBatch,
            &completed_keys,
        )
        .await
        {
            Ok(job_ids) if !job_ids.is_empty() => {
                report.queued += job_ids.len();
                report.job_ids.extend(job_ids);
            }
            Ok(_) => {
                report.skipped += 1;
                report.notes.push(format!("{artist} - {title}: no queueable tracks"));
            }
            Err(error) => {
                report.skipped += 1;
                report.notes.push(format!("{artist} - {title}: {error}"));
            }
        }
    }

    Ok(report)
}

#[tauri::command]
pub async fn get_download_jobs(state: State<'_, AppState>) -> Result<Vec<DownloadJob>, String> {
    let mut jobs = state
        .download_jobs
        .lock()
        .map_err(|error| error.to_string())?
        .values()
        .cloned()
        .collect::<Vec<_>>();

    if let Ok(transfers) = fetch_slskd_transfers(&load_slskd_config(&state)?).await {
        for transfer in transfers.iter().map(slskd_transfer_to_job) {
            if state.is_download_cancelled(&transfer.id) {
                continue;
            }

            if let Some(existing) = jobs.iter_mut().find(|job| download_jobs_match(job, &transfer)) {
                if state.is_download_cancelled(&existing.id) {
                    continue;
                }
                existing.status = transfer.status;
                existing.progress = transfer.progress;
                existing.provider = Some("slskd".to_string());
                existing.error = transfer.error.clone();
                if existing.album.is_none() {
                    existing.album = transfer.album.clone();
                }
            } else if jobs.iter().all(|job| job.id != transfer.id) {
                jobs.push(transfer);
            }
        }
    }

    Ok(jobs)
}

#[tauri::command]
pub async fn search_download_metadata(
    state: State<'_, AppState>,
    query: String,
) -> Result<DownloadMetadataSearchResult, String> {
    let provider_config = load_remote_provider_config(&state)?;
    search_catalog_metadata(&provider_config, &query).await
}

#[tauri::command]
pub async fn get_artist_discography(
    state: State<'_, AppState>,
    artist: String,
    artist_mbid: Option<String>,
) -> Result<DownloadArtistDiscography, String> {
    let provider_config = load_remote_provider_config(&state)?;
    fetch_artist_discography(&provider_config, artist.as_str(), artist_mbid).await
}

#[tauri::command]
pub async fn get_slskd_transfers(state: State<'_, AppState>) -> Result<Vec<Value>, String> {
    let slskd_config = load_slskd_config(&state)?;
    fetch_slskd_transfers(&slskd_config).await
}

fn slskd_transfer_to_job(transfer: &Value) -> DownloadJob {
    let filename = transfer
        .get("filename")
        .and_then(Value::as_str)
        .unwrap_or("Unknown transfer")
        .to_string();

    let username = transfer
        .get("username")
        .and_then(Value::as_str)
        .unwrap_or("slskd")
        .to_string();

    let state = transfer
        .get("state")
        .and_then(Value::as_str)
        .unwrap_or("Queued");

    let percent_complete = transfer
        .get("percentComplete")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);

    let progress = if percent_complete > 1.0 {
        (percent_complete / 100.0) as f32
    } else {
        percent_complete as f32
    };

    DownloadJob {
        id: transfer
            .get("id")
            .map(Value::to_string)
            .unwrap_or_else(|| format!("{username}:{filename}")),
        query: filename.clone(),
        artist: username,
        title: PathTitle::from_filename(&filename).title,
        album: None,
        status: map_transfer_status(state),
        provider: Some("slskd".to_string()),
        progress,
        error: transfer
            .get("exception")
            .and_then(Value::as_str)
            .map(ToString::to_string),
    }
}

fn map_transfer_status(state: &str) -> DownloadStatus {
    match state {
        "Completed" | "Succeeded" => DownloadStatus::Done,
        "InProgress" => DownloadStatus::Downloading,
        "Completed, Succeeded" => DownloadStatus::Done,
        "Cancelled" | "Errored" | "Failed" => DownloadStatus::Failed,
        "Requested" | "Queued" | "Initialized" => DownloadStatus::Queued,
        "InProgress, Queued" | "InProgress, Initializing" => DownloadStatus::Downloading,
        _ => DownloadStatus::Searching,
    }
}

fn build_track_task(
    id: &str,
    artist: &str,
    title: &str,
    album: Option<String>,
    strategy: AcquisitionStrategy,
) -> TrackTask {
    TrackTask {
        task_id: id.to_string(),
        source: TrackTaskSource::Manual,
        target: NormalizedTrack {
            spotify_track_id: None,
            source_playlist: None,
            artist: artist.to_string(),
            album_artist: Some(artist.to_string()),
            title: title.to_string(),
            album,
            track_number: None,
            disc_number: None,
            year: None,
            duration_secs: None,
            isrc: None,
        },
        strategy,
    }
}

fn load_remote_provider_config(state: &AppState) -> Result<RemoteProviderConfig, String> {
    let db = state.db.lock().map_err(|error| error.to_string())?;
    Ok(RemoteProviderConfig {
        qobuz_email: read_setting(&db, "qobuz_email").or_else(|| state.download_config.qobuz_email.clone()),
        qobuz_password: read_setting(&db, "qobuz_password").or_else(|| state.download_config.qobuz_password.clone()),
        qobuz_password_hash: read_setting(&db, "qobuz_password_hash"),
        qobuz_app_id: read_setting(&db, "qobuz_app_id"),
        qobuz_app_secret: read_setting(&db, "qobuz_app_secret"),
        qobuz_user_auth_token: read_setting(&db, "qobuz_user_auth_token"),
        qobuz_secrets: read_setting(&db, "qobuz_secrets"),
        deezer_arl: read_setting(&db, "deezer_arl").or_else(|| state.download_config.deezer_arl.clone()),
        spotify_client_id: read_setting(&db, "spotify_client_id").or_else(|| state.download_config.spotify_client_id.clone()),
        spotify_client_secret: read_setting(&db, "spotify_client_secret").or_else(|| state.download_config.spotify_client_secret.clone()),
        spotify_access_token: read_setting(&db, "spotify_access_token").or_else(|| state.download_config.spotify_access_token.clone()),
    })
}

fn load_slskd_config(state: &AppState) -> Result<SlskdConnectionConfig, String> {
    let db = state.db.lock().map_err(|error| error.to_string())?;
    Ok(SlskdConnectionConfig {
        url: read_setting(&db, "slskd_url")
            .or_else(|| state.download_config.slskd_url.clone())
            .unwrap_or_else(|| "http://localhost:5030".to_string()),
        username: read_setting(&db, "slskd_user")
            .or_else(|| state.download_config.slskd_user.clone())
            .unwrap_or_else(|| "slskd".to_string()),
        password: read_setting(&db, "slskd_pass")
            .or_else(|| state.download_config.slskd_pass.clone())
            .unwrap_or_else(|| "slskd".to_string()),
        api_key: read_setting(&db, "slskd_api_key"),
    })
}

fn read_setting(db: &cassette_core::db::Db, key: &str) -> Option<String> {
    db.get_setting(key)
        .ok()
        .flatten()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn download_jobs_match(job: &DownloadJob, transfer: &DownloadJob) -> bool {
    if job.id == transfer.id {
        return true;
    }

    if job.provider.as_deref() != Some("slskd") {
        return false;
    }

    let haystack = normalize_job_text(&transfer.query);
    let artist = normalize_job_text(&job.artist);
    let title = normalize_job_text(&job.title);

    (!artist.is_empty() && haystack.contains(&artist)) && (!title.is_empty() && haystack.contains(&title))
}

async fn queue_discography_with_rules(
    state: State<'_, AppState>,
    scope: &str,
    discography: DownloadArtistDiscography,
    include_singles: bool,
    include_eps: bool,
    include_compilations: bool,
    max_albums: usize,
) -> Result<AcquisitionQueueReport, String> {
    let completed_keys = state
        .db
        .lock()
        .map_err(|error| error.to_string())?
        .get_completed_task_keys()
        .map_err(|error| error.to_string())?;
    let mut report = AcquisitionQueueReport {
        scope: scope.to_string(),
        requested: discography.albums.len(),
        ..AcquisitionQueueReport::default()
    };
    let mut seen = std::collections::HashSet::<String>::new();

    for album in discography.albums {
        if report.job_ids.len() >= max_albums {
            report.notes.push("Queue limit reached.".to_string());
            break;
        }

        if !release_type_allowed(
            album.release_type.as_deref(),
            include_singles,
            include_eps,
            include_compilations,
        ) {
            report.skipped += 1;
            continue;
        }

        let title = album.title.trim();
        let artist = album.artist.trim();
        if title.is_empty() || artist.is_empty() {
            report.skipped += 1;
            continue;
        }

        let dedupe_key = format!(
            "{}::{}",
            artist.to_ascii_lowercase(),
            title.to_ascii_lowercase()
        );
        if !seen.insert(dedupe_key) {
            report.skipped += 1;
            continue;
        }

        match queue_album_tracks(
            state.clone(),
            artist,
            title,
            TrackTaskSource::Manual,
            AcquisitionStrategy::DiscographyBatch,
            &completed_keys,
        )
        .await
        {
            Ok(job_ids) if !job_ids.is_empty() => {
                report.queued += job_ids.len();
                report.job_ids.extend(job_ids);
            }
            Ok(_) => {
                report.skipped += 1;
                report.notes.push(format!("{artist} - {title}: no queueable tracks"));
            }
            Err(error) => {
                report.skipped += 1;
                report.notes.push(format!("{artist} - {title}: {error}"));
            }
        }
    }

    Ok(report)
}

pub(crate) async fn queue_album_tracks(
    state: State<'_, AppState>,
    artist: &str,
    album: &str,
    source: TrackTaskSource,
    strategy: AcquisitionStrategy,
    completed_keys: &std::collections::HashSet<String>,
) -> Result<Vec<String>, String> {
    let track_tasks = resolve_album_track_tasks(artist, album, source, strategy).await?;
    let mut queued_job_ids = Vec::new();

    for task in track_tasks {
        if completed_keys.contains(&task.task_id) {
            continue;
        }

        let job = DownloadJob {
            id: task.task_id.clone(),
            query: format!("{} - {} - {}", task.target.artist, task.target.album.clone().unwrap_or_default(), task.target.title),
            artist: task.target.artist.clone(),
            title: task.target.title.clone(),
            album: task.target.album.clone(),
            status: DownloadStatus::Queued,
            provider: None,
            progress: 0.0,
            error: None,
        };

        state
            .download_jobs
            .lock()
            .map_err(|error| error.to_string())?
            .insert(task.task_id.clone(), job);

        if let Err(error) = state.persist_pending_task(&task, DirectorProgress::Queued) {
            if let Ok(mut jobs) = state.download_jobs.lock() {
                jobs.remove(&task.task_id);
            }
            return Err(error.to_string());
        }

        match state.director_submitter.submit(task.clone()).await {
            Ok(()) => {
                queued_job_ids.push(task.task_id.clone());
            }
            Err(error) => {
                let _ = state.delete_pending_task(&task.task_id);
                if let Ok(mut jobs) = state.download_jobs.lock() {
                    jobs.remove(&task.task_id);
                }
                return Err(error.to_string());
            }
        }
    }

    Ok(queued_job_ids)
}

pub(crate) async fn resolve_album_track_tasks(
    artist: &str,
    album: &str,
    source: TrackTaskSource,
    strategy: AcquisitionStrategy,
) -> Result<Vec<TrackTask>, String> {
    let metadata = MetadataService::new().map_err(|error| error.to_string())?;
    let releases = metadata
        .search_release(artist, album)
        .await
        .map_err(|error| error.to_string())?;
    let release = releases
        .into_iter()
        .find(|release| release.track_count.unwrap_or(0) > 0)
        .ok_or_else(|| format!("MusicBrainz could not resolve album: {artist} - {album}"))?;
    let release_with_tracks = metadata
        .get_release_tracks(&release.id)
        .await
        .map_err(|error| error.to_string())?;
    if release_with_tracks.tracks.is_empty() {
        return Err(format!("MusicBrainz returned no tracks for {artist} - {album}"));
    }

    let mut tasks = release_with_tracks
        .tracks
        .into_iter()
        .map(|track| TrackTask {
            task_id: album_track_task_key(artist, album, track.disc_number, track.track_number, &track.title),
            source: source.clone(),
            target: NormalizedTrack {
                spotify_track_id: None,
                source_playlist: None,
                artist: if track.artist.trim().is_empty() {
                    artist.to_string()
                } else {
                    track.artist.clone()
                },
                album_artist: Some(artist.to_string()),
                title: track.title,
                album: Some(release.title.clone()),
                track_number: Some(track.track_number),
                disc_number: Some(track.disc_number),
                year: release.year,
                duration_secs: if track.duration_ms > 0 {
                    Some(track.duration_ms as f64 / 1000.0)
                } else {
                    None
                },
                isrc: None,
            },
            strategy,
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

pub(crate) fn album_track_task_key(
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

pub(crate) fn normalize_task_component(value: &str) -> String {
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

fn release_type_allowed(
    release_type: Option<&str>,
    include_singles: bool,
    include_eps: bool,
    include_compilations: bool,
) -> bool {
    let Some(raw) = release_type.map(str::trim).filter(|value| !value.is_empty()) else {
        return true;
    };
    let normalized = raw.to_ascii_lowercase();

    if normalized.contains("single") {
        return include_singles;
    }
    if normalized == "ep" || normalized.contains(" ep") || normalized.contains("extended play") {
        return include_eps;
    }
    if normalized.contains("compilation")
        || normalized.contains("various")
        || normalized.contains("anthology")
    {
        return include_compilations;
    }
    true
}

#[tauri::command]
pub async fn get_candidate_review(
    state: State<'_, AppState>,
    task_id: String,
) -> Result<Vec<cassette_core::db::CandidateReviewItem>, String> {
    let db = state.db.lock().map_err(|error| error.to_string())?;
    db.get_candidate_review(&task_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn get_task_provenance(
    state: State<'_, AppState>,
    task_id: String,
) -> Result<Option<String>, String> {
    let db = state.db.lock().map_err(|error| error.to_string())?;
    db.get_task_provenance(&task_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn get_recent_task_results(
    state: State<'_, AppState>,
    limit: Option<usize>,
) -> Result<Vec<serde_json::Value>, String> {
    let db = state.db.lock().map_err(|error| error.to_string())?;
    let results = db
        .get_recent_task_results(limit.unwrap_or(50))
        .map_err(|error| error.to_string())?;
    Ok(results
        .into_iter()
        .map(|(task_id, disposition, provider, error)| {
            serde_json::json!({
                "task_id": task_id,
                "disposition": disposition,
                "provider": provider,
                "error": error,
            })
        })
        .collect())
}

fn normalize_job_text(value: &str) -> String {
    value
        .chars()
        .map(|ch| if ch.is_alphanumeric() { ch.to_ascii_lowercase() } else { ' ' })
        .collect::<String>()
}

struct PathTitle {
    title: String,
}

impl PathTitle {
    fn from_filename(filename: &str) -> Self {
        let title = std::path::Path::new(filename)
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or(filename)
            .to_string();

        Self { title }
    }
}
