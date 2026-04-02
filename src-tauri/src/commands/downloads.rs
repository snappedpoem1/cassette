use crate::state::{AppState, BacklogRunStatus};
use cassette_core::acquisition::{
    AcquisitionRequest, AcquisitionRequestStatus, AcquisitionScope, ConfirmationPolicy,
};
use cassette_core::director::{
    AcquisitionStrategy, NormalizedTrack, TrackTask,
    TrackTaskSource,
};
use cassette_core::librarian::models::{AcquisitionRequestEvent, AcquisitionRequestRow};
use cassette_core::metadata::MetadataService;
use cassette_core::models::{
    AcquisitionQueueReport, DownloadArtistDiscography, DownloadJob, DownloadMetadataSearchResult,
    DownloadStatus,
};
use cassette_core::sources::{fetch_slskd_transfers, get_artist_discography as fetch_artist_discography, search_metadata as search_catalog_metadata, RemoteProviderConfig, SlskdConnectionConfig};
use serde::Serialize;
use serde_json::Value;
use tauri::{Emitter, State};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
pub struct AcquisitionRequestListItem {
    pub id: i64,
    pub scope: String,
    pub artist: String,
    pub album: Option<String>,
    pub title: String,
    pub status: String,
    pub strategy: String,
    pub task_id: Option<String>,
    pub request_signature: String,
    pub selected_provider: Option<String>,
    pub failure_class: Option<String>,
    pub final_path: Option<String>,
    pub execution_disposition: Option<String>,
    pub updated_at: String,
    pub created_at: String,
}

#[tauri::command]
pub async fn start_download(
    state: State<'_, AppState>,
    artist: String,
    title: String,
    album: Option<String>,
) -> Result<String, String> {
    let id = format!("job-{}", Uuid::new_v4());
    let task = build_track_task(&id, &artist, &title, album.clone(), AcquisitionStrategy::Standard);
    let request = request_from_track_task(&task, "manual");
    queue_track_request(state, request).await
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
        desired_track_id: None,
        source_operation_id: None,
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
            musicbrainz_recording_id: None,
            musicbrainz_release_id: None,
            canonical_artist_id: None,
            canonical_release_id: None,
        },
        strategy,
    }
}

fn request_source_name(source: &TrackTaskSource) -> &'static str {
    match source {
        TrackTaskSource::SpotifyLibrary => "spotify_library",
        TrackTaskSource::SpotifyHistory => "spotify_history",
        TrackTaskSource::SpotifyPlaylist { .. } => "spotify_playlist",
        TrackTaskSource::Manual => "manual",
    }
}

fn request_from_track_task(task: &TrackTask, source_name: &str) -> AcquisitionRequest {
    AcquisitionRequest {
        id: None,
        scope: AcquisitionScope::Track,
        source: task.source.clone(),
        source_name: source_name.to_string(),
        source_track_id: task.target.spotify_track_id.clone(),
        source_album_id: None,
        source_artist_id: None,
        artist: task.target.artist.clone(),
        album: task.target.album.clone(),
        title: task.target.title.clone(),
        track_number: task.target.track_number,
        disc_number: task.target.disc_number,
        year: task.target.year,
        duration_secs: task.target.duration_secs,
        isrc: task.target.isrc.clone(),
        musicbrainz_recording_id: task.target.musicbrainz_recording_id.clone(),
        musicbrainz_release_id: task.target.musicbrainz_release_id.clone(),
        canonical_artist_id: task.target.canonical_artist_id,
        canonical_release_id: task.target.canonical_release_id,
        strategy: task.strategy,
        quality_policy: None,
        excluded_providers: Vec::new(),
        edition_policy: None,
        confirmation_policy: ConfirmationPolicy::Automatic,
        desired_track_id: task.desired_track_id,
        source_operation_id: task.source_operation_id.clone(),
        task_id: Some(task.task_id.clone()),
        request_signature: Some(cassette_core::db::director_request_signature(task)),
        status: AcquisitionRequestStatus::Pending,
        raw_payload_json: serde_json::to_string(task).ok(),
    }
}

async fn queue_track_request(
    state: State<'_, AppState>,
    request: AcquisitionRequest,
) -> Result<String, String> {
    let task = request.to_track_task();
    let job = DownloadJob {
        id: task.task_id.clone(),
        query: format!(
            "{} - {} - {}",
            task.target.artist,
            task.target.album.clone().unwrap_or_default(),
            task.target.title
        ),
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

    match state.submit_acquisition_request(&request).await {
        Ok(_) => Ok(task.task_id),
        Err(error) => {
            if let Ok(mut jobs) = state.download_jobs.lock() {
                jobs.remove(&task.task_id);
            }
            Err(error.to_string())
        }
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
        let source_name = request_source_name(&task.source);
        let request = request_from_track_task(&task, source_name);
        match queue_track_request(state.clone(), request).await {
            Ok(task_id) => queued_job_ids.push(task_id),
            Err(error) => return Err(error),
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
            desired_track_id: None,
            source_operation_id: None,
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
                musicbrainz_recording_id: None,
                musicbrainz_release_id: Some(release.id.clone()),
                canonical_artist_id: None,
                canonical_release_id: None,
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

#[tauri::command]
pub async fn create_acquisition_request(
    state: State<'_, AppState>,
    request: AcquisitionRequest,
) -> Result<AcquisitionRequestRow, String> {
    state
        .create_acquisition_request(&request)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn list_acquisition_requests(
    state: State<'_, AppState>,
    status: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<AcquisitionRequestListItem>, String> {
    let rows = state
        .control_db
        .list_acquisition_requests(status.as_deref(), limit.unwrap_or(100))
        .await
        .map_err(|error| error.to_string())?;

    let db = state.db.lock().map_err(|error| error.to_string())?;
    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        let execution = row
            .task_id
            .as_deref()
            .map(|task_id| db.get_task_execution_summary(task_id))
            .transpose()
            .map_err(|error| error.to_string())?
            .flatten();

        items.push(AcquisitionRequestListItem {
            id: row.id,
            scope: row.scope,
            artist: row.artist,
            album: row.album,
            title: row.title,
            status: row.status,
            strategy: row.strategy,
            task_id: row.task_id,
            request_signature: row.request_signature,
            selected_provider: execution.as_ref().and_then(|value| value.provider.clone()),
            failure_class: execution.as_ref().and_then(|value| value.failure_class.clone()),
            final_path: execution.as_ref().and_then(|value| value.final_path.clone()),
            execution_disposition: execution.as_ref().map(|value| value.disposition.clone()),
            updated_at: execution
                .as_ref()
                .map(|value| value.updated_at.clone())
                .unwrap_or(row.updated_at),
            created_at: row.created_at,
        });
    }

    Ok(items)
}

#[tauri::command]
pub async fn get_acquisition_request_timeline(
    state: State<'_, AppState>,
    request_id: i64,
) -> Result<Vec<AcquisitionRequestEvent>, String> {
    state
        .control_db
        .get_acquisition_request_timeline(request_id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn get_request_candidate_review(
    state: State<'_, AppState>,
    request_id: i64,
) -> Result<Vec<cassette_core::db::CandidateReviewItem>, String> {
    let request = state
        .control_db
        .get_acquisition_request(request_id)
        .await
        .map_err(|error| error.to_string())?;
    let Some(task_id) = request.and_then(|row| row.task_id) else {
        return Ok(Vec::new());
    };

    let db = state.db.lock().map_err(|error| error.to_string())?;
    db.get_candidate_review(&task_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn get_request_lineage(
    state: State<'_, AppState>,
    request_id: i64,
) -> Result<serde_json::Value, String> {
    let request = state
        .control_db
        .get_acquisition_request(request_id)
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("request {request_id} not found"))?;
    let timeline = state
        .control_db
        .get_acquisition_request_timeline(request_id)
        .await
        .map_err(|error| error.to_string())?;

    let (execution, provenance, candidate_review) = if let Some(task_id) = request.task_id.as_deref() {
        let db = state.db.lock().map_err(|error| error.to_string())?;
        (
            db.get_task_execution_summary(task_id)
                .map_err(|error| error.to_string())?,
            db.get_task_provenance(task_id)
                .map_err(|error| error.to_string())?,
            db.get_candidate_review(task_id)
                .map_err(|error| error.to_string())?,
        )
    } else {
        (None, None, Vec::new())
    };

    Ok(serde_json::json!({
        "request": request,
        "timeline": timeline,
        "execution": execution,
        "provenance": provenance,
        "candidate_review": candidate_review,
    }))
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

// ── Background album backlog downloader ──────────────────────────────────────

fn now_iso() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Simple ISO-like UTC string without chrono dep in this crate
    let s = secs;
    let mins = s / 60 % 60;
    let hours = s / 3600 % 24;
    let days_since_epoch = s / 86400;
    // Approximate — good enough for display; precise parsing not needed here
    format!("epoch+{}d {:02}:{:02}", days_since_epoch, hours, mins)
}

/// Start a background run that processes the Spotify missing-album backlog.
/// Emits `director-backlog-progress` events on the app handle as it runs.
/// Only one backlog run can be active at a time; a second call is a no-op.
#[tauri::command]
pub async fn start_backlog_run(
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
    batch_size: Option<usize>,
    limit: Option<usize>,
) -> Result<BacklogRunStatus, String> {
    {
        let status = state.backlog_status.lock().map_err(|e| e.to_string())?;
        if status.running {
            return Ok(status.clone());
        }
    }

    state.backlog_cancel.store(false, std::sync::atomic::Ordering::SeqCst);

    {
        let mut status = state.backlog_status.lock().map_err(|e| e.to_string())?;
        *status = BacklogRunStatus {
            running: true,
            started_at: Some(now_iso()),
            ..BacklogRunStatus::default()
        };
    }

    let db = Arc::clone(&state.db);
    let control_db = Arc::clone(&state.control_db);
    let download_jobs = Arc::clone(&state.download_jobs);
    let submitter = state.director_submitter.clone();
    let backlog_status = Arc::clone(&state.backlog_status);
    let backlog_cancel = Arc::clone(&state.backlog_cancel);
    let batch = batch_size.unwrap_or(10);
    let hard_limit = limit.unwrap_or(500);

    tokio::spawn(async move {
        let mut total_tracks = 0usize;
        let mut albums_queued = 0usize;
        let mut albums_skipped = 0usize;
        let mut errors: Vec<String> = Vec::new();

        'outer: loop {
            if backlog_cancel.load(std::sync::atomic::Ordering::SeqCst) {
                break;
            }

            // Fetch next batch of missing albums from DB
            let missing_albums = {
                let Ok(db) = db.lock() else { break };
                db.get_missing_spotify_albums(batch + 20).unwrap_or_default()
            };
            let completed_keys = {
                let Ok(db) = db.lock() else { break };
                db.get_completed_task_keys().unwrap_or_default()
            };

            if missing_albums.is_empty() {
                break;
            }

            for album in &missing_albums {
                if backlog_cancel.load(std::sync::atomic::Ordering::SeqCst) {
                    break 'outer;
                }
                if total_tracks >= hard_limit {
                    break 'outer;
                }

                let artist = album.artist.trim().to_string();
                let title = album.album.trim().to_string();
                if artist.is_empty() || title.is_empty() {
                    albums_skipped += 1;
                    continue;
                }

                // Update current album in status
                {
                    if let Ok(mut s) = backlog_status.lock() {
                        s.current_album = Some(format!("{artist} — {title}"));
                        s.albums_queued = albums_queued;
                        s.albums_skipped = albums_skipped;
                        s.tracks_submitted = total_tracks;
                    }
                }
                let _ = app_handle.emit("director-backlog-progress", {
                    serde_json::json!({
                        "running": true,
                        "current_album": format!("{artist} — {title}"),
                        "albums_queued": albums_queued,
                        "albums_skipped": albums_skipped,
                        "tracks_submitted": total_tracks,
                    })
                });

                // Resolve tracklist via MusicBrainz
                let track_tasks = match resolve_album_track_tasks(
                    &artist,
                    &title,
                    TrackTaskSource::SpotifyHistory,
                    AcquisitionStrategy::DiscographyBatch,
                )
                .await
                {
                    Ok(tasks) => tasks,
                    Err(e) => {
                        errors.push(format!("{artist} - {title}: {e}"));
                        albums_skipped += 1;
                        // Brief pause so we don't hammer MusicBrainz on errors
                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                        continue;
                    }
                };

                let mut submitted_any = false;
                for task in &track_tasks {
                    if completed_keys.contains(&task.task_id) {
                        continue;
                    }
                    if total_tracks >= hard_limit {
                        break;
                    }

                    let job = DownloadJob {
                        id: task.task_id.clone(),
                        query: format!(
                            "{} - {} - {}",
                            task.target.artist,
                            task.target.album.clone().unwrap_or_default(),
                            task.target.title
                        ),
                        artist: task.target.artist.clone(),
                        title: task.target.title.clone(),
                        album: task.target.album.clone(),
                        status: DownloadStatus::Queued,
                        provider: None,
                        progress: 0.0,
                        error: None,
                    };
                    if let Ok(mut jobs) = download_jobs.lock() {
                        jobs.insert(task.task_id.clone(), job);
                    }
                    let request = request_from_track_task(&task, request_source_name(&task.source));
                    let request_row = match control_db.create_acquisition_request(&request).await {
                        Ok(row) => row,
                        Err(error) => {
                            errors.push(format!("{artist} - {title}: {error}"));
                            continue;
                        }
                    };
                    if let Ok(db) = db.lock() {
                        let _ = db.upsert_director_pending_task(&task, "Queued");
                    }
                    let _ = control_db
                        .update_acquisition_request_status_by_task_id(
                            &task.task_id,
                            AcquisitionRequestStatus::Queued.as_str(),
                            "runtime_queued",
                            Some("queued for director submission"),
                            None,
                        )
                        .await;
                    if submitter.submit(task.clone()).await.is_ok() {
                        let _ = control_db
                            .update_acquisition_request_status_by_task_id(
                                request_row.task_id.as_deref().unwrap_or_default(),
                                AcquisitionRequestStatus::Submitted.as_str(),
                                "director_submitted",
                                Some("submitted to director"),
                                None,
                            )
                            .await;
                        total_tracks += 1;
                        submitted_any = true;
                    } else {
                        if let Ok(db) = db.lock() {
                            let _ = db.delete_director_pending_task(&task.task_id);
                        }
                        let _ = control_db
                            .update_acquisition_request_status_by_task_id(
                                &task.task_id,
                                AcquisitionRequestStatus::Failed.as_str(),
                                "director_submit_failed",
                                Some("director submission failed"),
                                None,
                            )
                            .await;
                    }
                }

                if submitted_any {
                    albums_queued += 1;
                } else {
                    albums_skipped += 1;
                }

                // Rate-limit MusicBrainz lookups
                tokio::time::sleep(std::time::Duration::from_millis(350)).await;
            }

            // If we got fewer results than the batch we asked for, the queue is exhausted
            if missing_albums.len() < batch {
                break;
            }
        }

        let finished_at = now_iso();
        if let Ok(mut s) = backlog_status.lock() {
            s.running = false;
            s.current_album = None;
            s.albums_queued = albums_queued;
            s.albums_skipped = albums_skipped;
            s.tracks_submitted = total_tracks;
            s.errors = errors.clone();
            s.finished_at = Some(finished_at.clone());
        }
        let _ = app_handle.emit("director-backlog-progress", serde_json::json!({
            "running": false,
            "albums_queued": albums_queued,
            "albums_skipped": albums_skipped,
            "tracks_submitted": total_tracks,
            "finished_at": finished_at,
            "errors": errors,
        }));
    });

    let status = state.backlog_status.lock().map_err(|e| e.to_string())?.clone();
    Ok(status)
}

#[tauri::command]
pub async fn stop_backlog_run(state: State<'_, AppState>) -> Result<(), String> {
    state.backlog_cancel.store(true, std::sync::atomic::Ordering::SeqCst);
    if let Ok(mut s) = state.backlog_status.lock() {
        if s.running {
            s.running = false;
            s.finished_at = Some(now_iso());
            s.current_album = None;
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn get_backlog_status(state: State<'_, AppState>) -> Result<BacklogRunStatus, String> {
    let status = state.backlog_status.lock().map_err(|e| e.to_string())?.clone();
    Ok(status)
}

/// Returns debug statistics: recent task results, provider attempt counts, pending queue size.
#[tauri::command]
pub async fn get_director_debug_stats(
    state: State<'_, AppState>,
    limit: Option<usize>,
) -> Result<serde_json::Value, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let recent = db
        .get_recent_task_results(limit.unwrap_or(100))
        .map_err(|e| e.to_string())?;

    let pending_count = db
        .get_pending_director_tasks()
        .map_err(|e| e.to_string())?
        .len();

    // Provider attempt breakdown from recent results
    let mut provider_counts: std::collections::HashMap<String, (usize, usize)> =
        std::collections::HashMap::new();
    for (_, disposition, provider, _) in &recent {
        if !provider.is_empty() {
            let entry = provider_counts.entry(provider.clone()).or_insert((0, 0));
            if disposition == "Finalized" || disposition == "AlreadyPresent" {
                entry.0 += 1;
            } else {
                entry.1 += 1;
            }
        }
    }

    let provider_stats: Vec<serde_json::Value> = provider_counts
        .into_iter()
        .map(|(provider, (success, failed))| {
            serde_json::json!({
                "provider": provider,
                "success": success,
                "failed": failed,
            })
        })
        .collect();

    let recent_results: Vec<serde_json::Value> = recent
        .into_iter()
        .take(limit.unwrap_or(50))
        .map(|(task_id, disposition, provider, error)| {
            serde_json::json!({
                "task_id": task_id,
                "disposition": disposition,
                "provider": provider,
                "error": error,
            })
        })
        .collect();

    Ok(serde_json::json!({
        "pending_count": pending_count,
        "provider_stats": provider_stats,
        "recent_results": recent_results,
    }))
}

use std::sync::Arc;
