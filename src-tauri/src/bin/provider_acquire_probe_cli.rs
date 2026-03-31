use cassette_core::db::Db;
use cassette_core::director::config::{DirectorConfig, QualityPolicy, TempRecoveryPolicy};
use cassette_core::director::models::{
    AcquisitionStrategy, CandidateAcquisition, NormalizedTrack, ProviderSearchCandidate, TrackTask,
    TrackTaskSource,
};
use cassette_core::director::provider::Provider;
use cassette_core::director::providers::{DeezerProvider, QobuzProvider, SlskdProvider};
use cassette_core::director::strategy::{StrategyPlan, StrategyPlanner};
use cassette_core::director::temp::TaskTempContext;
use cassette_core::director::validation::validate_candidate;
use cassette_core::sources::{RemoteProviderConfig, SlskdConnectionConfig};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::time::{timeout, Duration};
use uuid::Uuid;

#[derive(Debug)]
struct ProbeResult {
    provider: String,
    status: &'static str,
    detail: String,
}

fn app_db_path() -> Result<PathBuf, String> {
    let app_data = std::env::var("APPDATA").map_err(|error| error.to_string())?;
    Ok(PathBuf::from(app_data)
        .join("dev.cassette.app")
        .join("cassette.db"))
}

fn read_setting(db: &Db, key: &str) -> Option<String> {
    db.get_setting(key)
        .ok()
        .flatten()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn remote_provider_config(db: &Db) -> RemoteProviderConfig {
    let env = RemoteProviderConfig::from_env();
    RemoteProviderConfig {
        qobuz_email: read_setting(db, "qobuz_email").or(env.qobuz_email),
        qobuz_password: read_setting(db, "qobuz_password").or(env.qobuz_password),
        qobuz_password_hash: read_setting(db, "qobuz_password_hash").or(env.qobuz_password_hash),
        qobuz_app_id: read_setting(db, "qobuz_app_id").or(env.qobuz_app_id),
        qobuz_app_secret: read_setting(db, "qobuz_app_secret").or(env.qobuz_app_secret),
        qobuz_user_auth_token: read_setting(db, "qobuz_user_auth_token").or(env.qobuz_user_auth_token),
        qobuz_secrets: read_setting(db, "qobuz_secrets").or(env.qobuz_secrets),
        deezer_arl: read_setting(db, "deezer_arl").or(env.deezer_arl),
        spotify_client_id: read_setting(db, "spotify_client_id").or(env.spotify_client_id),
        spotify_client_secret: read_setting(db, "spotify_client_secret").or(env.spotify_client_secret),
        spotify_access_token: read_setting(db, "spotify_access_token").or(env.spotify_access_token),
    }
}

fn slskd_config(db: &Db) -> SlskdConnectionConfig {
    SlskdConnectionConfig {
        url: read_setting(db, "slskd_url")
            .or_else(|| std::env::var("SLSKD_URL").ok())
            .unwrap_or_else(|| "http://localhost:5030".to_string()),
        username: read_setting(db, "slskd_user")
            .or_else(|| std::env::var("SLSKD_USER").ok())
            .unwrap_or_else(|| "slskd".to_string()),
        password: read_setting(db, "slskd_pass")
            .or_else(|| std::env::var("SLSKD_PASSWORD").ok())
            .unwrap_or_else(|| "slskd".to_string()),
        api_key: read_setting(db, "slskd_api_key"),
    }
}

fn library_root(db: &Db) -> PathBuf {
    read_setting(db, "library_base")
        .or_else(|| std::env::var("LIBRARY_BASE").ok())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("A:\\Music"))
}

fn staging_root(db: &Db) -> PathBuf {
    read_setting(db, "staging_folder")
        .or_else(|| std::env::var("STAGING_FOLDER").ok())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("A:\\Staging"))
}

fn parse_args() -> Result<(Vec<String>, TrackTask), String> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let mut provider = "all".to_string();
    let mut artist = "Brand New".to_string();
    let mut title = "Sic Transit Gloria... Glory Fades".to_string();
    let mut album = Some("Deja Entendu".to_string());

    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--provider" => {
                index += 1;
                provider = args.get(index).cloned().ok_or("--provider requires a value")?;
            }
            "--artist" => {
                index += 1;
                artist = args.get(index).cloned().ok_or("--artist requires a value")?;
            }
            "--title" => {
                index += 1;
                title = args.get(index).cloned().ok_or("--title requires a value")?;
            }
            "--album" => {
                index += 1;
                album = Some(args.get(index).cloned().ok_or("--album requires a value")?);
            }
            "--no-album" => {
                album = None;
            }
            other => {
                return Err(format!("Unknown argument: {other}"));
            }
        }
        index += 1;
    }

    let providers = match provider.as_str() {
        "all" => vec!["deezer".to_string(), "qobuz".to_string(), "slskd".to_string()],
        "deezer" | "qobuz" | "slskd" => vec![provider],
        _ => return Err("Provider must be one of: all, deezer, qobuz, slskd".to_string()),
    };

    let task = TrackTask {
        task_id: format!("probe-{}", Uuid::new_v4()),
        source: TrackTaskSource::Manual,
        desired_track_id: None,
        source_operation_id: None,
        target: NormalizedTrack {
            spotify_track_id: None,
            source_playlist: None,
            artist,
            album_artist: None,
            title,
            album,
            track_number: None,
            disc_number: None,
            year: None,
            duration_secs: None,
            isrc: None,
        },
        strategy: AcquisitionStrategy::SingleTrackPriority,
    };

    Ok((providers, task))
}

fn strategy_for(provider: &Arc<dyn Provider>, task: &TrackTask) -> StrategyPlan {
    let planner = StrategyPlanner;
    planner.plan(task, &[provider.descriptor()], &DirectorConfig::default())
}

async fn prepare_temp_context(provider_id: &str) -> Result<TaskTempContext, String> {
    let root = std::env::temp_dir()
        .join("cassette-provider-probes")
        .join(format!("{provider_id}-{}", Uuid::new_v4()));
    let active_dir = root.join("active");
    let quarantine_dir = root.join("quarantine");
    tokio::fs::create_dir_all(&active_dir)
        .await
        .map_err(|error| error.to_string())?;
    tokio::fs::create_dir_all(&quarantine_dir)
        .await
        .map_err(|error| error.to_string())?;

    Ok(TaskTempContext {
        task_id: format!("probe-{provider_id}"),
        root,
        active_dir,
        quarantine_dir,
    })
}

async fn cleanup_temp_context(context: &TaskTempContext) {
    if context.root.exists() {
        let _ = tokio::fs::remove_dir_all(&context.root).await;
    }
}

fn select_candidate(candidates: &[ProviderSearchCandidate], task: &TrackTask) -> Option<ProviderSearchCandidate> {
    ranked_candidates(candidates, task).into_iter().next()
}

fn ranked_candidates(candidates: &[ProviderSearchCandidate], task: &TrackTask) -> Vec<ProviderSearchCandidate> {
    let target_title = task.target.title.to_ascii_lowercase();
    let target_album = task.target.album.as_ref().map(|value| value.to_ascii_lowercase());

    let mut ranked = candidates.to_vec();
    ranked.sort_by(|left, right| {
        score_candidate(right, &target_title, target_album.as_deref())
            .cmp(&score_candidate(left, &target_title, target_album.as_deref()))
    });
    ranked
}

fn score_candidate(
    candidate: &ProviderSearchCandidate,
    target_title: &str,
    target_album: Option<&str>,
) -> i64 {
    let mut score = (candidate.metadata_confidence * 1000.0) as i64;
    if candidate.title.to_ascii_lowercase().contains(target_title) {
        score += 500;
    }
    if let (Some(target_album), Some(candidate_album)) = (target_album, candidate.album.as_deref()) {
        if candidate_album.to_ascii_lowercase().contains(target_album) {
            score += 200;
        }
    }
    score
}

async fn probe_provider(provider: Arc<dyn Provider>, task: &TrackTask) -> ProbeResult {
    let descriptor = provider.descriptor();
    let strategy = strategy_for(&provider, task);
    let search_timeout_secs = if descriptor.id == "slskd" { 240 } else { 90 };
    let context = match prepare_temp_context(&descriptor.id).await {
        Ok(context) => context,
        Err(detail) => {
            return ProbeResult {
                provider: descriptor.id,
                status: "FAIL",
                detail: format!("temp context failed: {detail}"),
            }
        }
    };

    let result = async {
        let search_result = timeout(
            Duration::from_secs(search_timeout_secs),
            provider.search(task, &strategy),
        )
        .await
        .map_err(|_| "search timed out".to_string())?
        .map_err(|error| format!("search failed: {error}"))?;

        if search_result.is_empty() {
            return Err("search returned no candidates".to_string());
        }

        let Some(primary_candidate) = select_candidate(&search_result, task) else {
            return Err("could not select a candidate".to_string());
        };

        let candidate_summary = format!(
            "candidates={} selected={} title='{}' album='{}'",
            search_result.len(),
            primary_candidate.provider_candidate_id,
            primary_candidate.title,
            primary_candidate.album.clone().unwrap_or_else(|| "<none>".to_string())
        );

        let ranked = ranked_candidates(&search_result, task);
        let mut acquisition = None::<CandidateAcquisition>;
        let mut errors = Vec::<String>::new();
        for candidate in ranked.into_iter().take(8) {
            match timeout(
                Duration::from_secs(180),
                provider.acquire(task, &candidate, &context, &strategy),
            )
            .await
            {
                Ok(Ok(acquired)) => {
                    acquisition = Some(acquired);
                    break;
                }
                Ok(Err(error)) => errors.push(format!(
                    "{}: {}",
                    candidate.provider_candidate_id, error
                )),
                Err(_) => errors.push(format!("{}: timed out", candidate.provider_candidate_id)),
            }
        }
        let Some(acquisition) = acquisition else {
            let joined = if errors.is_empty() {
                "acquire failed for all candidate attempts".to_string()
            } else {
                format!("acquire failed for all attempts: {}", errors.join(" | "))
            };
            return Err(format!("{candidate_summary}; {joined}"));
        };

        let validation = validate_acquisition(task, &acquisition).await?;
        Ok(format!("{candidate_summary}; {validation}"))
    }
    .await;

    cleanup_temp_context(&context).await;

    match result {
        Ok(detail) => ProbeResult {
            provider: descriptor.id,
            status: "OK",
            detail,
        },
        Err(detail) => ProbeResult {
            provider: descriptor.id,
            status: "FAIL",
            detail,
        },
    }
}

async fn validate_acquisition(
    task: &TrackTask,
    acquisition: &CandidateAcquisition,
) -> Result<String, String> {
    let report = validate_candidate(
        acquisition.temp_path.clone(),
        task.target.clone(),
        QualityPolicy::default(),
    )
    .await
    .map_err(|error| format!("validation failed: {error}"))?;

    Ok(format!(
        "acquired {} bytes as {} ({:?}, duration={:?})",
        acquisition.file_size,
        acquisition.temp_path.display(),
        report.quality,
        report.duration_secs
    ))
}

#[tokio::main]
async fn main() -> Result<(), String> {
    let _ = dotenvy::dotenv();
    let db = Db::open(&app_db_path()?).map_err(|error| error.to_string())?;
    let (providers, task) = parse_args()?;
    let remote = remote_provider_config(&db);
    let slskd = slskd_config(&db);
    let library_root = library_root(&db);
    let staging_root = staging_root(&db);

    let scan_roots = vec![staging_root.clone(), library_root.clone()];
    let available: Vec<Arc<dyn Provider>> = vec![
        Arc::new(DeezerProvider::new(remote.clone())),
        Arc::new(QobuzProvider::new(remote.clone())),
        Arc::new(SlskdProvider::new(slskd, scan_roots)),
    ];

    println!(
        "Probing providers for artist='{}' title='{}' album='{}'",
        task.target.artist,
        task.target.title,
        task.target.album.clone().unwrap_or_else(|| "<none>".to_string())
    );
    println!("{:<10} {:<6} detail", "provider", "status");

    for provider_id in providers {
        let maybe_provider = available
            .iter()
            .find(|provider| provider.descriptor().id == provider_id)
            .cloned();

        let result = if let Some(provider) = maybe_provider {
            probe_provider(provider, &task).await
        } else {
            ProbeResult {
                provider: provider_id,
                status: "FAIL",
                detail: "provider not registered".to_string(),
            }
        };

        println!(
            "{:<10} {:<6} {}",
            result.provider, result.status, result.detail
        );
    }

    let _temp_policy = TempRecoveryPolicy::default();
    Ok(())
}
