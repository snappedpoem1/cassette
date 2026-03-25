use anyhow::Result;
use cassette_core::{
    db::Db,
    director::{
        providers::{
            DeezerProvider, LocalArchiveProvider, QobuzProvider, SlskdProvider, UsenetProvider,
            YtDlpProvider,
        },
        Director, DirectorConfig, DirectorProgress, DirectorSubmission, DuplicatePolicy,
        ProviderPolicy, QualityPolicy, RetryPolicy, TempRecoveryPolicy,
    },
    downloader::DownloadConfig,
    models::{DownloadJob, PlaybackState},
    player::Player,
    sources::{RemoteProviderConfig, SlskdConnectionConfig},
};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio::sync::Semaphore;

pub struct AppState {
    pub db: Arc<Mutex<Db>>,
    pub player: Arc<Player>,
    pub playback_state: Arc<Mutex<PlaybackState>>,
    pub download_jobs: Arc<Mutex<HashMap<String, DownloadJob>>>,
    #[allow(dead_code)]
    pub active_download_keys: Arc<Mutex<HashSet<String>>>,
    #[allow(dead_code)]
    pub download_semaphore: Arc<Semaphore>,
    pub director_submitter: DirectorSubmission,
    pub download_config: DownloadConfig,
}

impl AppState {
    pub fn new(db_path: &Path) -> Result<Self> {
        let db = Db::open(db_path)?;
        bootstrap_library_roots(&db);

        let download_config = bootstrap_download_config(&db);
        let director_handle = build_director(&db, &download_config);
        let event_rx = director_handle.subscribe();
        let result_rx = director_handle.subscribe_results();

        let state = Self {
            db: Arc::new(Mutex::new(db)),
            player: Arc::new(Player::new()),
            playback_state: Arc::new(Mutex::new(PlaybackState::default())),
            download_jobs: Arc::new(Mutex::new(HashMap::new())),
            active_download_keys: Arc::new(Mutex::new(HashSet::new())),
            download_semaphore: Arc::new(Semaphore::new(3)),
            director_submitter: director_handle.submitter.clone(),
            download_config,
        };

        spawn_director_event_listener(Arc::clone(&state.download_jobs), event_rx);
        spawn_director_result_listener(Arc::clone(&state.db), Arc::clone(&state.download_jobs), result_rx);

        Ok(state)
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
        worker_concurrency: 3,
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
                provider_id: "slskd".to_string(),
                max_concurrency: 1,
            },
            ProviderPolicy {
                provider_id: "qobuz".to_string(),
                max_concurrency: 1,
            },
            ProviderPolicy {
                provider_id: "deezer".to_string(),
                max_concurrency: 1,
            },
            ProviderPolicy {
                provider_id: "usenet".to_string(),
                max_concurrency: 1,
            },
            ProviderPolicy {
                provider_id: "local_archive".to_string(),
                max_concurrency: 1,
            },
            ProviderPolicy {
                provider_id: "yt_dlp".to_string(),
                max_concurrency: 1,
            },
        ],
        staging_root: PathBuf::from(&staging_root),
        ..DirectorConfig::default()
    };

    let providers: Vec<Arc<dyn cassette_core::director::Provider>> = vec![
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
    download_jobs: Arc<Mutex<HashMap<String, DownloadJob>>>,
    mut event_rx: tokio::sync::broadcast::Receiver<cassette_core::director::DirectorEvent>,
) {
    tauri::async_runtime::spawn(async move {
        while let Ok(event) = event_rx.recv().await {
            if let Ok(mut jobs) = download_jobs.lock() {
                if let Some(job) = jobs.get_mut(&event.task_id) {
                    job.provider = event.provider_id.clone();
                    job.error = Some(event.message.clone());
                    match event.progress {
                        DirectorProgress::Queued => {
                            job.status = cassette_core::models::DownloadStatus::Queued;
                            job.progress = 0.0;
                        }
                        DirectorProgress::InProgress | DirectorProgress::ProviderAttempt => {
                            job.status = cassette_core::models::DownloadStatus::Searching;
                            job.progress = 0.15;
                        }
                        DirectorProgress::Validating => {
                            job.status = cassette_core::models::DownloadStatus::Verifying;
                            job.progress = 0.65;
                        }
                        DirectorProgress::Tagging | DirectorProgress::Finalizing => {
                            job.status = cassette_core::models::DownloadStatus::Verifying;
                            job.progress = 0.85;
                        }
                        DirectorProgress::Finalized => {
                            job.status = cassette_core::models::DownloadStatus::Done;
                            job.progress = 1.0;
                            job.error = None;
                        }
                        DirectorProgress::Failed | DirectorProgress::Exhausted => {
                            job.status = cassette_core::models::DownloadStatus::Failed;
                            job.progress = 0.0;
                        }
                        DirectorProgress::Skipped => {
                            job.status = cassette_core::models::DownloadStatus::Done;
                            job.progress = 1.0;
                        }
                    }
                }
            }
        }
    });
}

fn spawn_director_result_listener(
    db: Arc<Mutex<Db>>,
    download_jobs: Arc<Mutex<HashMap<String, DownloadJob>>>,
    mut result_rx: tokio::sync::broadcast::Receiver<cassette_core::director::DirectorTaskResult>,
) {
    tauri::async_runtime::spawn(async move {
        while let Ok(result) = result_rx.recv().await {
            if let Ok(db) = db.lock() {
                let _ = db.save_director_task_result(&result);
                if let Some(finalized) = &result.finalized {
                    if let Ok(track) = cassette_core::library::read_track_metadata(&finalized.path) {
                        let _ = db.upsert_track(&track);
                    }
                }
            }

            if let Ok(mut jobs) = download_jobs.lock() {
                if let Some(job) = jobs.get_mut(&result.task_id) {
                    job.error = result.error.clone();
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
    let Ok(contents) = fs::read_to_string(path) else {
        return StreamripConfig::default();
    };

    StreamripConfig {
        qobuz_email: read_toml_string(&contents, "email_or_userid"),
        qobuz_password_hash: read_toml_string(&contents, "password_or_token"),
        qobuz_app_id: read_toml_string(&contents, "app_id"),
        qobuz_secrets: read_toml_array(&contents, "secrets"),
        deezer_arl: read_toml_string(&contents, "arl"),
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

fn read_toml_string(contents: &str, key: &str) -> Option<String> {
    contents.lines().find_map(|line| {
        let trimmed = line.trim();
        let prefix = format!("{key} = ");
        trimmed.strip_prefix(&prefix).map(|value| {
            value
                .trim()
                .trim_matches('"')
                .trim_matches('\'')
                .to_string()
        })
    })
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

fn read_toml_array(contents: &str, key: &str) -> Option<String> {
    contents.lines().find_map(|line| {
        let trimmed = line.trim();
        let prefix = format!("{key} = [");
        trimmed.strip_prefix(&prefix).map(|value| {
            value
                .trim()
                .trim_end_matches(']')
                .split(',')
                .map(|item| item.trim().trim_matches('"').trim_matches('\''))
                .filter(|item| !item.is_empty())
                .collect::<Vec<_>>()
                .join(",")
        })
    })
}
