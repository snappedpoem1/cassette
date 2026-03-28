use anyhow::Result;
use cassette_core::{
    db::{Db, PendingDirectorTask, TerminalDirectorTaskUpdate},
    director::{
        providers::{
            DeezerProvider, LocalArchiveProvider, QobuzProvider, RealDebridProvider, SlskdProvider,
            UsenetProvider, YtDlpProvider,
        },
        models::ProviderHealthState,
        Director, DirectorConfig, DirectorEvent, DirectorProgress, DirectorSubmission,
        DirectorHandle, DuplicatePolicy, ProviderPolicy, QualityPolicy, RetryPolicy,
        TempRecoveryPolicy, TrackTask,
    },
    downloader::DownloadConfig,
    models::{DownloadJob, DownloadStatus, PlaybackState},
    player::Player,
    sources::{RemoteProviderConfig, SlskdConnectionConfig},
};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use toml::Table as TomlTable;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};
use tracing::warn;

pub struct AppState {
    pub db: Arc<Mutex<Db>>,
    pub player: Arc<Player>,
    pub playback_state: Arc<Mutex<PlaybackState>>,
    pub download_jobs: Arc<Mutex<HashMap<String, DownloadJob>>>,
    pub cancelled_downloads: Arc<Mutex<HashSet<String>>>,
    pub director_handle: Arc<Mutex<DirectorHandle>>,
    pub director_submitter: DirectorSubmission,
    pub download_config: DownloadConfig,
    pub http_client: reqwest::Client,
}

impl AppState {
    pub fn new(db_path: &Path, app_handle: Option<AppHandle>) -> Result<Self> {
        let db = Db::open(db_path)?;
        bootstrap_library_roots(&db);

        let download_config = bootstrap_download_config(&db);
        let director_handle = build_director(&db, &download_config);
        Ok(Self::from_parts(
            db,
            director_handle,
            download_config,
            app_handle,
        ))
    }

    #[allow(dead_code)]
    pub fn new_with_director(
        db_path: &Path,
        director_handle: DirectorHandle,
        download_config: DownloadConfig,
        app_handle: Option<AppHandle>,
    ) -> Result<Self> {
        let db = Db::open(db_path)?;
        bootstrap_library_roots(&db);
        Ok(Self::from_parts(
            db,
            director_handle,
            download_config,
            app_handle,
        ))
    }

    fn from_parts(
        db: Db,
        director_handle: DirectorHandle,
        download_config: DownloadConfig,
        app_handle: Option<AppHandle>,
    ) -> Self {
        let director_submitter = director_handle.submitter.clone();
        let event_rx = director_handle.subscribe();
        let result_rx = director_handle.subscribe_results();
        let provider_health_rx = director_handle.subscribe_health();

        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("failed to build HTTP client");

        let state = Self {
            db: Arc::new(Mutex::new(db)),
            player: Arc::new(Player::new()),
            playback_state: Arc::new(Mutex::new(PlaybackState::default())),
            download_jobs: Arc::new(Mutex::new(HashMap::new())),
            cancelled_downloads: Arc::new(Mutex::new(HashSet::new())),
            director_handle: Arc::new(Mutex::new(director_handle)),
            director_submitter,
            download_config,
            http_client,
        };

        spawn_director_event_listener(
            app_handle.clone(),
            Arc::clone(&state.db),
            Arc::clone(&state.download_jobs),
            Arc::clone(&state.cancelled_downloads),
            event_rx,
        );
        spawn_director_result_listener(
            app_handle.clone(),
            Arc::clone(&state.db),
            Arc::clone(&state.download_jobs),
            Arc::clone(&state.cancelled_downloads),
            result_rx,
        );
        spawn_director_health_listener(provider_health_rx, app_handle.clone());
        state.resume_pending_downloads();

        state
    }

    pub fn persist_pending_task(&self, task: &TrackTask, progress: DirectorProgress) -> Result<()> {
        let db = self.db.lock().map_err(|error| anyhow::anyhow!(error.to_string()))?;
        db.upsert_director_pending_task(task, director_progress_label(progress))
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        Ok(())
    }

    pub fn delete_pending_task(&self, task_id: &str) -> Result<()> {
        let db = self.db.lock().map_err(|error| anyhow::anyhow!(error.to_string()))?;
        db.delete_director_pending_task(task_id)
            .map_err(|error| anyhow::anyhow!(error.to_string()))?;
        Ok(())
    }

    pub fn mark_download_cancelled(&self, task_id: &str) {
        if let Ok(mut cancelled) = self.cancelled_downloads.lock() {
            cancelled.insert(task_id.to_string());
        }
    }

    pub fn is_download_cancelled(&self, task_id: &str) -> bool {
        self.cancelled_downloads
            .lock()
            .map(|cancelled| cancelled.contains(task_id))
            .unwrap_or(false)
    }

    pub fn cancel_download(&self, task_id: &str) -> Result<bool> {
        let cancelled = self
            .director_handle
            .lock()
            .map(|handle| handle.cancel_task(task_id))
            .unwrap_or(false);
        if !cancelled {
            return Ok(false);
        }
        self.mark_download_cancelled(task_id);
        self.delete_pending_task(task_id)?;
        if let Ok(mut jobs) = self.download_jobs.lock() {
            if let Some(job) = jobs.get_mut(task_id) {
                job.status = DownloadStatus::Cancelled;
                job.progress = 0.0;
                job.error = Some("Cancellation requested".to_string());
            }
        }
        Ok(true)
    }

    fn resume_pending_downloads(&self) {
        let (pending, terminal_updates) = {
            let db = match self.db.lock() {
                Ok(db) => db,
                Err(error) => {
                    warn!(error = %error, "failed to lock db while loading pending director tasks");
                    return;
                }
            };

            let pending = match db.get_pending_director_tasks() {
                Ok(tasks) => tasks,
                Err(error) => {
                    warn!(error = %error, "failed to load pending director tasks");
                    return;
                }
            };
            let terminal_updates = db.get_terminal_director_task_updates().unwrap_or_default();
            (pending, terminal_updates)
        };

        let recovery_plan = build_pending_recovery_plan(pending, &terminal_updates);

        if let Ok(db) = self.db.lock() {
            for task_id in &recovery_plan.stale_task_ids {
                let _ = db.delete_director_pending_task(&task_id);
            }
        }

        if recovery_plan.resumable_tasks.is_empty() {
            return;
        }

        if let Ok(mut jobs) = self.download_jobs.lock() {
            for pending_task in &recovery_plan.resumable_tasks {
                jobs.insert(
                    pending_task.task.task_id.clone(),
                    download_job_from_task(&pending_task.task, &pending_task.progress),
                );
            }
        }

        let submitter = self.director_submitter.clone();
        tokio::spawn(async move {
            for pending_task in recovery_plan.resumable_tasks {
                let task_id = pending_task.task.task_id.clone();
                if let Err(error) = submitter.submit(pending_task.task).await {
                    warn!(task_id = %task_id, error = %error, "failed to resubmit pending director task");
                }
            }
        });
    }
}

struct PendingRecoveryPlan {
    resumable_tasks: Vec<PendingDirectorTask>,
    stale_task_ids: Vec<String>,
}

fn build_pending_recovery_plan(
    pending: Vec<PendingDirectorTask>,
    terminal_updates: &HashMap<String, TerminalDirectorTaskUpdate>,
) -> PendingRecoveryPlan {
    let mut resumable_tasks = Vec::new();
    let mut stale_task_ids = Vec::new();

    for pending_task in pending {
        let is_terminal_progress = matches!(
            pending_task.progress.as_str(),
            "Finalized" | "Cancelled" | "Failed" | "Exhausted" | "Skipped"
        );
        let has_newer_terminal_result = terminal_updates
            .get(&pending_task.task.task_id)
            .map(|update| update.updated_at >= pending_task.updated_at)
            .unwrap_or(false);

        if is_terminal_progress || has_newer_terminal_result {
            stale_task_ids.push(pending_task.task.task_id);
            continue;
        }

        resumable_tasks.push(pending_task);
    }

    PendingRecoveryPlan {
        resumable_tasks,
        stale_task_ids,
    }
}

fn build_director(db: &Db, download_config: &DownloadConfig) -> cassette_core::director::DirectorHandle {
    let library_root = db
        .get_setting("library_base")
        .ok()
        .flatten()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| download_config.library_base.clone());
    let staging_root = db
        .get_setting("staging_folder")
        .ok()
        .flatten()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| download_config.staging_folder.clone());

    let remote_provider_config = load_remote_provider_config(db, download_config);
    let slskd_connection = load_slskd_connection_config(db, download_config);
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
            ProviderPolicy {
                provider_id: "qobuz".to_string(),
                max_concurrency: 2, // 24-bit FLAC; drops connections above 3 concurrent albums
            },
            ProviderPolicy {
                provider_id: "deezer".to_string(),
                max_concurrency: 4, // 16-bit FLAC; smaller files, more robust
            },
            ProviderPolicy {
                provider_id: "slskd".to_string(),
                max_concurrency: 2, // P2P; internal global search semaphore limits further
            },
            ProviderPolicy {
                provider_id: "usenet".to_string(),
                max_concurrency: 1, // SABnzbd manages its own download queue
            },
            ProviderPolicy {
                provider_id: "local_archive".to_string(),
                max_concurrency: 2, // filesystem I/O, trivially parallel
            },
            ProviderPolicy {
                provider_id: "yt_dlp".to_string(),
                max_concurrency: 2, // YouTube/SoundCloud rate limits
            },
            ProviderPolicy {
                provider_id: "real_debrid".to_string(),
                max_concurrency: 3, // API ~250 req/min; bottleneck is torrent resolve time
            },
        ],
        staging_root: PathBuf::from(&staging_root),
        ..DirectorConfig::default()
    };

    let mut providers: Vec<Arc<dyn cassette_core::director::Provider>> = vec![
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

    // Real-Debrid: only add if API key is configured
    let rd_key = read_setting(db, "real_debrid_key")
        .or_else(|| download_config.real_debrid_key.clone())
        .filter(|k| !k.trim().is_empty());
    if let Some(key) = rd_key {
        providers.push(Arc::new(RealDebridProvider::new(key)));
    }

    Director::new(config, providers).start()
}

fn load_remote_provider_config(db: &Db, download_config: &DownloadConfig) -> RemoteProviderConfig {
    RemoteProviderConfig {
        qobuz_email: read_setting(db, "qobuz_email").or_else(|| download_config.qobuz_email.clone()),
        qobuz_password: read_setting(db, "qobuz_password")
            .or_else(|| download_config.qobuz_password.clone()),
        qobuz_password_hash: read_setting(db, "qobuz_password_hash"),
        qobuz_app_id: read_setting(db, "qobuz_app_id"),
        qobuz_app_secret: read_setting(db, "qobuz_app_secret"),
        qobuz_user_auth_token: read_setting(db, "qobuz_user_auth_token"),
        qobuz_secrets: read_setting(db, "qobuz_secrets"),
        deezer_arl: read_setting(db, "deezer_arl").or_else(|| download_config.deezer_arl.clone()),
        spotify_client_id: read_setting(db, "spotify_client_id")
            .or_else(|| download_config.spotify_client_id.clone()),
        spotify_client_secret: read_setting(db, "spotify_client_secret")
            .or_else(|| download_config.spotify_client_secret.clone()),
        spotify_access_token: read_setting(db, "spotify_access_token")
            .or_else(|| download_config.spotify_access_token.clone()),
    }
}

fn load_slskd_connection_config(db: &Db, download_config: &DownloadConfig) -> SlskdConnectionConfig {
    SlskdConnectionConfig {
        url: read_setting(db, "slskd_url")
            .or_else(|| download_config.slskd_url.clone())
            .unwrap_or_else(|| "http://localhost:5030".to_string()),
        username: read_setting(db, "slskd_user")
            .or_else(|| download_config.slskd_user.clone())
            .unwrap_or_else(|| "slskd".to_string()),
        password: read_setting(db, "slskd_pass")
            .or_else(|| download_config.slskd_pass.clone())
            .unwrap_or_else(|| "slskd".to_string()),
        api_key: read_setting(db, "slskd_api_key"),
    }
}

fn read_setting(db: &Db, key: &str) -> Option<String> {
    db.get_setting(key)
        .ok()
        .flatten()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn spawn_director_event_listener(
    app_handle: Option<AppHandle>,
    db: Arc<Mutex<Db>>,
    download_jobs: Arc<Mutex<HashMap<String, DownloadJob>>>,
    cancelled_downloads: Arc<Mutex<HashSet<String>>>,
    mut event_rx: tokio::sync::broadcast::Receiver<cassette_core::director::DirectorEvent>,
) {
    tokio::spawn(async move {
        while let Ok(event) = event_rx.recv().await {
            let is_cancelled = cancelled_downloads
                .lock()
                .map(|cancelled| cancelled.contains(&event.task_id))
                .unwrap_or(false);
            if is_cancelled && !matches!(event.progress, DirectorProgress::Cancelled) {
                continue;
            }

            if let Ok(mut jobs) = download_jobs.lock() {
                if let Some(job) = jobs.get_mut(&event.task_id) {
                    apply_director_event_to_job(job, &event);
                }
            }

            if let Ok(db) = db.lock() {
                let progress = director_progress_label(event.progress);
                if !matches!(
                    event.progress,
                    DirectorProgress::Finalized
                        | DirectorProgress::Failed
                        | DirectorProgress::Exhausted
                        | DirectorProgress::Cancelled
                        | DirectorProgress::Skipped
                ) {
                    let _ = db.update_director_pending_task_progress(&event.task_id, progress);
                }
            }

            if let Some(app_handle) = &app_handle {
                if let Err(error) = app_handle.emit("director-event", &event) {
                    warn!(task_id = %event.task_id, error = %error, "failed to emit director event");
                }
            }
        }
    });
}

fn spawn_director_result_listener(
    app_handle: Option<AppHandle>,
    db: Arc<Mutex<Db>>,
    download_jobs: Arc<Mutex<HashMap<String, DownloadJob>>>,
    cancelled_downloads: Arc<Mutex<HashSet<String>>>,
    mut result_rx: tokio::sync::broadcast::Receiver<cassette_core::director::DirectorTaskResult>,
) {
    tokio::spawn(async move {
        while let Ok(result) = result_rx.recv().await {
            let is_cancelled = cancelled_downloads
                .lock()
                .map(|cancelled| cancelled.contains(&result.task_id))
                .unwrap_or(false);
            if is_cancelled
                && !matches!(
                    result.disposition,
                    cassette_core::director::FinalizedTrackDisposition::Cancelled
                )
            {
                continue;
            }

            if let Ok(db) = db.lock() {
                let request = db
                    .get_pending_director_task(&result.task_id)
                    .ok()
                    .flatten();
                let _ = db.save_director_task_result(&result, request.as_ref().map(|task| &task.task));
                let _ = db.delete_director_pending_task(&result.task_id);
                if let Some(finalized) = &result.finalized {
                    if let Ok(track) = cassette_core::library::read_track_metadata(&finalized.path) {
                        let _ = db.upsert_track(&track);
                    }
                }
            }

            if let Ok(mut jobs) = download_jobs.lock() {
                if let Some(job) = jobs.get_mut(&result.task_id) {
                    match result.disposition {
                        cassette_core::director::FinalizedTrackDisposition::Finalized
                        | cassette_core::director::FinalizedTrackDisposition::AlreadyPresent
                        | cassette_core::director::FinalizedTrackDisposition::MetadataOnly => {
                            job.status = DownloadStatus::Done;
                            job.progress = 1.0;
                            job.error = None;
                        }
                        cassette_core::director::FinalizedTrackDisposition::Cancelled => {
                            job.status = DownloadStatus::Cancelled;
                            job.progress = 0.0;
                            job.error = result
                                .error
                                .clone()
                                .or_else(|| Some("Cancelled by user".to_string()));
                        }
                        cassette_core::director::FinalizedTrackDisposition::Failed => {
                            job.status = DownloadStatus::Failed;
                            job.progress = 0.0;
                        }
                    }
                    if !matches!(
                        result.disposition,
                        cassette_core::director::FinalizedTrackDisposition::Cancelled
                    ) {
                        job.error = result.error.clone();
                    }
                }
            }

            if let Ok(mut cancelled) = cancelled_downloads.lock() {
                cancelled.remove(&result.task_id);
            }

            if let Some(app_handle) = &app_handle {
                if let Err(error) = app_handle.emit("director-result", &result) {
                    warn!(task_id = %result.task_id, error = %error, "failed to emit director result");
                }
            }
        }
    });
}

fn spawn_director_health_listener(
    mut provider_health_rx: tokio::sync::broadcast::Receiver<ProviderHealthState>,
    app_handle: Option<AppHandle>,
) {
    tokio::spawn(async move {
        while let Ok(event) = provider_health_rx.recv().await {
            if let Some(app_handle) = &app_handle {
                if let Err(error) = app_handle.emit("director-provider-health", &event) {
                    warn!(provider_id = %event.provider_id, error = %error, "failed to emit provider health event");
                }
            }
        }
    });
}

fn bootstrap_library_roots(db: &Db) {
    if db
        .get_library_roots()
        .map(|roots| !roots.is_empty())
        .unwrap_or(false)
    {
        return;
    }

    if let Some(path) = detect_library_base() {
        let _ = db.add_library_root(&path);
    }
}

fn bootstrap_download_config(db: &Db) -> DownloadConfig {
    let mut config = DownloadConfig::from_env();
    let streamrip = load_streamrip_config();
    let slskd = load_slskd_config();

    if config.library_base.trim().is_empty() {
        config.library_base = detect_library_base().unwrap_or_else(|| "A:\\Music".to_string());
    }
    if config.staging_folder.trim().is_empty() {
        config.staging_folder = detect_staging_folder().unwrap_or_else(|| "A:\\Staging".to_string());
    }
    if config.slskd_url.is_none() {
        config.slskd_url = Some("http://localhost:5030".to_string());
    }
    if config.slskd_user.is_none() {
        config.slskd_user = Some("slskd".to_string());
    }
    if config.slskd_pass.is_none() {
        config.slskd_pass = Some("slskd".to_string());
    }
    if config.qobuz_email.is_none() {
        config.qobuz_email = streamrip.qobuz_email;
    }
    if config.deezer_arl.is_none() {
        config.deezer_arl = streamrip.deezer_arl;
    }

    // Persist recovered machine defaults into the app DB on first boot so the UI
    // reflects the last known-good Cassette environment without manual re-entry.
    persist_setting_if_missing(db, "library_base", Some(config.library_base.as_str()));
    persist_setting_if_missing(db, "staging_folder", Some(config.staging_folder.as_str()));
    persist_setting_if_missing(db, "slskd_url", config.slskd_url.as_deref());
    persist_setting_if_missing(db, "slskd_user", config.slskd_user.as_deref());
    persist_setting_if_missing(db, "slskd_pass", config.slskd_pass.as_deref());
    persist_setting_if_missing(db, "qobuz_email", config.qobuz_email.as_deref());
    persist_setting_if_missing(
        db,
        "qobuz_password_hash",
        streamrip.qobuz_password_hash.as_deref(),
    );
    persist_setting_if_missing(db, "qobuz_app_id", streamrip.qobuz_app_id.as_deref());
    persist_setting_if_missing(
        db,
        "qobuz_app_secret",
        read_env_non_empty("QOBUZ_APP_SECRET").as_deref(),
    );
    persist_setting_if_missing(db, "qobuz_secrets", streamrip.qobuz_secrets.as_deref());
    persist_setting_if_missing(db, "deezer_arl", config.deezer_arl.as_deref());
    persist_setting_if_missing(
        db,
        "nzbgeek_api_key",
        read_env_non_empty("NZBGEEK_API_KEY").as_deref(),
    );
    persist_setting_if_missing(
        db,
        "usenet_host",
        read_env_non_empty("USENET_HOST").as_deref(),
    );
    persist_setting_if_missing(
        db,
        "sabnzbd_url",
        read_env_non_empty("SABNZBD_URL").as_deref(),
    );
    persist_setting_if_missing(
        db,
        "sabnzbd_api_key",
        read_env_non_empty("SABNZBD_API_KEY").as_deref(),
    );

    // Preserve the daemon's live Soulseek account in env-style keys for scripts and
    // future daemon bootstrap without surfacing it in the UI settings model.
    persist_setting_if_missing(db, "soulseek_username", slskd.soulseek_username.as_deref());
    persist_setting_if_missing(db, "soulseek_password", slskd.soulseek_password.as_deref());

    config
}

fn persist_setting_if_missing(db: &Db, key: &str, value: Option<&str>) {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return;
    };

    if db.get_setting(key).ok().flatten().is_none() {
        let _ = db.set_setting(key, value);
    }
}

fn read_env_non_empty(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn detect_library_base() -> Option<String> {
    let env_value = std::env::var("LIBRARY_BASE").ok();
    first_existing_path(
        env_value
            .into_iter()
            .chain([
                "A:\\music".to_string(),
                "A:\\Music".to_string(),
            ])
            .collect::<Vec<_>>(),
    )
}

fn detect_staging_folder() -> Option<String> {
    let env_value = std::env::var("STAGING_FOLDER").ok();
    first_existing_path(
        env_value
            .into_iter()
            .chain([
                "A:\\Staging".to_string(),
                "A:\\staging".to_string(),
            ])
            .collect::<Vec<_>>(),
    )
}

fn first_existing_path(candidates: Vec<String>) -> Option<String> {
    candidates.into_iter().find(|candidate| Path::new(candidate).exists())
}

#[derive(Default)]
struct StreamripConfig {
    qobuz_email: Option<String>,
    qobuz_password_hash: Option<String>,
    qobuz_app_id: Option<String>,
    qobuz_secrets: Option<String>,
    deezer_arl: Option<String>,
}

fn load_streamrip_config() -> StreamripConfig {
    let Some(app_data) = std::env::var_os("APPDATA") else {
        return StreamripConfig::default();
    };

    let path = PathBuf::from(app_data).join("streamrip").join("config.toml");
    let Ok(contents) = fs::read_to_string(&path) else {
        return StreamripConfig::default();
    };

    let Ok(doc) = contents.parse::<TomlTable>() else {
        tracing::warn!("streamrip config.toml could not be parsed as TOML: {}", path.display());
        return StreamripConfig::default();
    };

    let toml_str = |section: &str, key: &str| -> Option<String> {
        doc.get(section)?.get(key)?.as_str().map(str::to_owned)
    };
    let toml_arr_csv = |section: &str, key: &str| -> Option<String> {
        let arr = doc.get(section)?.get(key)?.as_array()?;
        let csv = arr.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join(",");
        if csv.is_empty() { None } else { Some(csv) }
    };

    StreamripConfig {
        qobuz_email: toml_str("qobuz", "email_or_userid"),
        qobuz_password_hash: toml_str("qobuz", "password_or_token"),
        qobuz_app_id: toml_str("qobuz", "app_id"),
        qobuz_secrets: toml_arr_csv("qobuz", "secrets"),
        deezer_arl: toml_str("deezer", "arl"),
    }
}

#[derive(Default)]
struct SlskdConfig {
    soulseek_username: Option<String>,
    soulseek_password: Option<String>,
}

fn load_slskd_config() -> SlskdConfig {
    let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") else {
        return SlskdConfig::default();
    };

    let path = PathBuf::from(local_app_data).join("slskd").join("slskd.yml");
    let Ok(contents) = fs::read_to_string(path) else {
        return SlskdConfig::default();
    };

    SlskdConfig {
        soulseek_username: read_yaml_value(&contents, "username"),
        soulseek_password: read_yaml_value(&contents, "password"),
    }
}

fn read_yaml_value(contents: &str, key: &str) -> Option<String> {
    contents.lines().find_map(|line| {
        let trimmed = line.trim();
        let prefix = format!("{key}:");
        trimmed
            .strip_prefix(&prefix)
            .map(|value| value.trim().trim_matches('"').trim_matches('\'').to_string())
    })
}

fn director_progress_label(progress: DirectorProgress) -> &'static str {
    match progress {
        DirectorProgress::Queued => "Queued",
        DirectorProgress::InProgress => "InProgress",
        DirectorProgress::ProviderAttempt => "ProviderAttempt",
        DirectorProgress::Validating => "Validating",
        DirectorProgress::Tagging => "Tagging",
        DirectorProgress::Finalizing => "Finalizing",
        DirectorProgress::Finalized => "Finalized",
        DirectorProgress::Cancelled => "Cancelled",
        DirectorProgress::Failed => "Failed",
        DirectorProgress::Exhausted => "Exhausted",
        DirectorProgress::Skipped => "Skipped",
    }
}

fn download_job_from_task(task: &TrackTask, progress: &str) -> DownloadJob {
    let (status, pct) = match progress {
        "Queued" => (DownloadStatus::Queued, 0.0),
        "InProgress" | "ProviderAttempt" => (DownloadStatus::Searching, 0.15),
        "Validating" => (DownloadStatus::Verifying, 0.65),
        "Tagging" | "Finalizing" => (DownloadStatus::Verifying, 0.85),
        "Finalized" | "Skipped" => (DownloadStatus::Done, 1.0),
        "Cancelled" => (DownloadStatus::Cancelled, 0.0),
        "Failed" | "Exhausted" => (DownloadStatus::Failed, 0.0),
        _ => (DownloadStatus::Queued, 0.0),
    };

    DownloadJob {
        id: task.task_id.clone(),
        query: format!(
            "{} {}{}",
            task.target.artist,
            task.target.title,
            task.target
                .album
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .map(|value| format!(" {value}"))
                .unwrap_or_default()
        )
        .trim()
        .to_string(),
        artist: task.target.artist.clone(),
        title: task.target.title.clone(),
        album: task.target.album.clone(),
        status,
        provider: None,
        progress: pct,
        error: None,
    }
}

fn apply_director_event_to_job(job: &mut DownloadJob, event: &DirectorEvent) {
    job.provider = event.provider_id.clone();
    job.error = Some(event.message.clone());
    match event.progress {
        DirectorProgress::Queued => {
            job.status = DownloadStatus::Queued;
            job.progress = 0.0;
        }
        DirectorProgress::InProgress | DirectorProgress::ProviderAttempt => {
            job.status = DownloadStatus::Searching;
            job.progress = 0.15;
        }
        DirectorProgress::Validating => {
            job.status = DownloadStatus::Verifying;
            job.progress = 0.65;
        }
        DirectorProgress::Tagging | DirectorProgress::Finalizing => {
            job.status = DownloadStatus::Verifying;
            job.progress = 0.85;
        }
        DirectorProgress::Finalized => {
            job.status = DownloadStatus::Done;
            job.progress = 1.0;
            job.error = None;
        }
        DirectorProgress::Cancelled => {
            job.status = DownloadStatus::Cancelled;
            job.progress = 0.0;
            job.error = Some(event.message.clone());
        }
        DirectorProgress::Failed | DirectorProgress::Exhausted => {
            job.status = DownloadStatus::Failed;
            job.progress = 0.0;
        }
        DirectorProgress::Skipped => {
            job.status = DownloadStatus::Done;
            job.progress = 1.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cassette_core::director::{AcquisitionStrategy, NormalizedTrack, TrackTask, TrackTaskSource};

    fn pending_task(task_id: &str, progress: &str, updated_at: &str) -> PendingDirectorTask {
        PendingDirectorTask {
            task: TrackTask {
                task_id: task_id.to_string(),
                source: TrackTaskSource::Manual,
                target: NormalizedTrack {
                    spotify_track_id: None,
                    source_playlist: None,
                    artist: "Artist".to_string(),
                    album_artist: Some("Artist".to_string()),
                    title: "Song".to_string(),
                    album: Some("Album".to_string()),
                    track_number: Some(1),
                    disc_number: Some(1),
                    year: Some(2024),
                    duration_secs: Some(35.0),
                    isrc: None,
                },
                strategy: AcquisitionStrategy::ObscureFallbackHeavy,
            },
            strategy: "ObscureFallbackHeavy".to_string(),
            progress: progress.to_string(),
            created_at: updated_at.to_string(),
            updated_at: updated_at.to_string(),
        }
    }

    #[test]
    fn pending_recovery_plan_keeps_newer_retry_and_drops_stale_terminal_row() {
        let pending = vec![
            pending_task("stale-failed", "Queued", "2026-03-27 12:00:00"),
            pending_task("retry-failed", "Queued", "2026-03-27 12:00:03"),
            pending_task("terminal-progress", "Cancelled", "2026-03-27 12:00:04"),
        ];
        let terminal_updates = HashMap::from([
            (
                "stale-failed".to_string(),
                TerminalDirectorTaskUpdate {
                    disposition: "Failed".to_string(),
                    updated_at: "2026-03-27 12:00:02".to_string(),
                },
            ),
            (
                "retry-failed".to_string(),
                TerminalDirectorTaskUpdate {
                    disposition: "Failed".to_string(),
                    updated_at: "2026-03-27 12:00:01".to_string(),
                },
            ),
        ]);

        let plan = build_pending_recovery_plan(pending, &terminal_updates);

        assert_eq!(
            plan.resumable_tasks
                .iter()
                .map(|task| task.task.task_id.as_str())
                .collect::<Vec<_>>(),
            vec!["retry-failed"]
        );
        assert_eq!(
            plan.stale_task_ids,
            vec!["stale-failed".to_string(), "terminal-progress".to_string()]
        );
    }
}
