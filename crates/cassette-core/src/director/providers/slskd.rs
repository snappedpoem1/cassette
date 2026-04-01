use crate::director::error::ProviderError;
use crate::director::models::{
    CandidateAcquisition, ProviderCapabilities, ProviderDescriptor, ProviderHealthState,
    ProviderHealthStatus, ProviderSearchCandidate, TrackTask,
};
use crate::director::provider::Provider;
use crate::director::strategy::StrategyPlan;
use crate::director::temp::TaskTempContext;
use crate::sources::{
    build_query, count_matching_terms, fetch_slskd_user_transfers, is_non_audio_path,
    normalize_text, normalized_terms, send_slskd_request, SlskdConnectionConfig,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures_util::stream::{FuturesUnordered, StreamExt};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::time::{sleep, Duration};

/// Global semaphore: slskd only allows one concurrent search operation.
static SLSKD_SEARCH_SEMAPHORE: std::sync::OnceLock<Arc<Semaphore>> = std::sync::OnceLock::new();

fn slskd_semaphore() -> Arc<Semaphore> {
    SLSKD_SEARCH_SEMAPHORE
        .get_or_init(|| Arc::new(Semaphore::new(1)))
        .clone()
}

#[derive(Debug, Clone)]
pub struct SlskdProvider {
    config: SlskdConnectionConfig,
    scan_roots: Vec<PathBuf>,
}

#[derive(Debug, Clone)]
struct SlskdCandidate {
    username: String,
    filename: String,
    size: u64,
}

impl SlskdProvider {
    pub fn new(config: SlskdConnectionConfig, scan_roots: Vec<PathBuf>) -> Self {
        Self { config, scan_roots }
    }
}

#[async_trait]
impl Provider for SlskdProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        ProviderDescriptor {
            id: "slskd".to_string(),
            display_name: "Soulseek / slskd".to_string(),
            trust_rank: 10,
            capabilities: ProviderCapabilities {
                supports_search: true,
                supports_download: true,
                supports_lossless: true,
                supports_batch: false,
            },
        }
    }

    async fn health_check(&self) -> Result<ProviderHealthState, ProviderError> {
        let client = reqwest::Client::new();
        let response = send_slskd_request(
            client.get(format!("{}/api/v0/server", self.config.url)),
            &self.config,
        )
        .await
        .map_err(|message| ProviderError::Network {
            provider_id: "slskd".to_string(),
            message,
        })?;

        if !response.status().is_success() {
            return Err(ProviderError::TemporaryOutage {
                provider_id: "slskd".to_string(),
                message: format!("health probe returned HTTP {}", response.status()),
            });
        }

        Ok(ProviderHealthState {
            provider_id: "slskd".to_string(),
            status: ProviderHealthStatus::Healthy,
            checked_at: Utc::now(),
            message: None,
        })
    }

    async fn search(
        &self,
        task: &TrackTask,
        _strategy: &StrategyPlan,
    ) -> Result<Vec<ProviderSearchCandidate>, ProviderError> {
        let request = SlskdTrackRequest {
            artist: task.target.artist.clone(),
            title: task.target.title.clone(),
            album: task.target.album.clone(),
        };
        let candidates = slskd_best_candidates(&self.config, &request).await?;
        Ok(candidates
            .into_iter()
            .take(8)
            .map(|candidate| ProviderSearchCandidate {
                provider_id: "slskd".to_string(),
                provider_candidate_id: format!("{}::{}::{}", candidate.username, candidate.filename, candidate.size),
                artist: task.target.artist.clone(),
                title: task.target.title.clone(),
                album: task.target.album.clone(),
                duration_secs: task.target.duration_secs,
                extension_hint: Path::new(&candidate.filename)
                    .extension()
                    .and_then(|value| value.to_str())
                    .map(ToString::to_string),
                bitrate_kbps: None,
                cover_art_url: None,
                metadata_confidence: 0.80,
            })
            .collect())
    }

    async fn acquire(
        &self,
        task: &TrackTask,
        candidate: &ProviderSearchCandidate,
        temp_context: &TaskTempContext,
        _strategy: &StrategyPlan,
    ) -> Result<CandidateAcquisition, ProviderError> {
        let mut parts = candidate.provider_candidate_id.splitn(3, "::");
        let username = parts.next().unwrap_or_default().to_string();
        let filename = parts.next().unwrap_or_default().to_string();
        let size = parts
            .next()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or_default();
        if username.is_empty() || filename.is_empty() {
            return Err(ProviderError::Other {
                provider_id: "slskd".to_string(),
                message: "invalid slskd candidate identifier".to_string(),
            });
        }

        let client = reqwest::Client::new();
        let response = send_slskd_request(
            client
                .post(format!("{}/api/v0/transfers/downloads/{}", self.config.url, username))
                .json(&serde_json::json!([{ "filename": filename, "size": size }])),
            &self.config,
        )
        .await
        .map_err(|message| ProviderError::Network {
            provider_id: "slskd".to_string(),
            message,
        })?;
        if !response.status().is_success() {
            return Err(ProviderError::Other {
                provider_id: "slskd".to_string(),
                message: format!("handoff failed: HTTP {}", response.status()),
            });
        }

        // Poll the slskd transfer API to check download progress.
        for _ in 0..24 {
            sleep(Duration::from_secs(5)).await;

            // Check transfer status via the API first.
            if let Ok(transfer_data) =
                fetch_slskd_user_transfers(&self.config, &username).await
            {
                if let Some(completed_path) =
                    find_completed_transfer(&transfer_data, &filename)
                {
                    // The transfer API reports a completed file. Check scan_roots for it.
                    let found = resolve_downloaded_file(
                        &self.scan_roots,
                        &completed_path,
                        &task.target.artist,
                        &task.target.title,
                    )
                    .await;
                    if let Some(found) = found {
                        return copy_to_temp(&found, candidate, task, temp_context).await;
                    }
                }
            }

            // Fallback: scan filesystem (async via spawn_blocking) for newly arrived files.
            let scan_roots = self.scan_roots.clone();
            let artist = task.target.artist.clone();
            let title = task.target.title.clone();
            let found = tokio::task::spawn_blocking(move || {
                find_matching_audio_file(&scan_roots, &artist, &title)
            })
            .await
            .unwrap_or(None);

            if let Some(found) = found {
                return copy_to_temp(&found, candidate, task, temp_context).await;
            }
        }

        Err(ProviderError::TemporaryOutage {
            provider_id: "slskd".to_string(),
            message: "queued transfer did not materialize into a local file before timeout"
                .to_string(),
        })
    }
}

#[derive(Debug, Clone)]
struct SlskdTrackRequest {
    artist: String,
    title: String,
    album: Option<String>,
}

async fn slskd_best_candidates(
    config: &SlskdConnectionConfig,
    request: &SlskdTrackRequest,
) -> Result<Vec<SlskdCandidate>, ProviderError> {
    let client = reqwest::Client::new();
    maybe_recover_search_queue(&client, config).await?;
    let mut ranked = Vec::new();
    let sem = slskd_semaphore();
    for query in slskd_query_candidates(request) {
        // slskd only allows one concurrent search — serialize via semaphore.
        // Use try_acquire to fail fast if another search is in progress, so the
        // track can fall through to qobuz/deezer without blocking.
        let _permit = match sem.try_acquire() {
            Ok(permit) => permit,
            Err(_) => {
                // Another search is in flight — let the director defer us to its
                // blocking second pass instead of classifying this as provider down.
                return Err(ProviderError::ProviderBusy {
                    provider_id: "slskd".to_string(),
                });
            }
        };

        let search_body = match create_search_with_recovery(&client, config, &query).await {
            Ok(body) => body,
            Err(_) => {
                ranked.extend(rank_slskd_candidates_from_history(&client, config, request).await?);
                continue;
            }
        };
        let Some(search_id) = search_body.get("id").and_then(Value::as_str) else {
            ranked.extend(rank_slskd_candidates_from_history(&client, config, request).await?);
            continue;
        };

        for _ in 0..10 {
            sleep(Duration::from_secs(3)).await;
            let detail_response = send_slskd_request(
                client.get(format!("{}/api/v0/searches/{search_id}/responses", config.url)),
                config,
            )
            .await
            .map_err(|message| ProviderError::Network {
                provider_id: "slskd".to_string(),
                message,
            })?;
            let detail_body = detail_response
                .json::<Value>()
                .await
                .map_err(|error| ProviderError::Network {
                    provider_id: "slskd".to_string(),
                    message: error.to_string(),
                })?;
            ranked.extend(rank_slskd_candidates(&detail_body, request));
        }
        // permit drops here, allowing next search
    }

    ranked.sort_by(|left, right| right.0.cmp(&left.0));
    Ok(ranked.into_iter().map(|(_, candidate)| candidate).collect())
}

async fn rank_slskd_candidates_from_history(
    client: &reqwest::Client,
    config: &SlskdConnectionConfig,
    request: &SlskdTrackRequest,
) -> Result<Vec<(i64, SlskdCandidate)>, ProviderError> {
    let history_response =
        send_slskd_request(client.get(format!("{}/api/v0/searches", config.url)), config)
            .await
            .map_err(|message| ProviderError::Network {
                provider_id: "slskd".to_string(),
                message,
            })?;
    if !history_response.status().is_success() {
        return Ok(Vec::new());
    }
    let history = history_response
        .json::<Vec<Value>>()
        .await
        .map_err(|error| ProviderError::Network {
            provider_id: "slskd".to_string(),
            message: error.to_string(),
        })?;

    let artist_norm = normalize_text(&request.artist);
    let title_norm = normalize_text(&request.title);
    let mut candidates = history
        .into_iter()
        .filter(|item| item.get("isComplete").and_then(Value::as_bool).unwrap_or(false))
        .filter(|item| item.get("responseCount").and_then(Value::as_u64).unwrap_or(0) > 0)
        .filter_map(|item| {
            let search_text = item
                .get("searchText")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let normalized = normalize_text(&search_text);
            let matches_artist = normalized.contains(&artist_norm);
            let matches_title = normalized.contains(&title_norm);
            if !(matches_artist || matches_title) {
                return None;
            }
            let started_at = item
                .get("startedAt")
                .and_then(Value::as_str)
                .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
                .map(|value| value.with_timezone(&Utc));
            let id = item.get("id").and_then(Value::as_str)?.to_string();
            Some((id, started_at))
        })
        .collect::<Vec<_>>();

    candidates.sort_by(|left, right| right.1.cmp(&left.1));
    let mut ranked = Vec::<(i64, SlskdCandidate)>::new();
    for (search_id, _) in candidates.into_iter().take(8) {
        let detail_response = send_slskd_request(
            client.get(format!("{}/api/v0/searches/{search_id}/responses", config.url)),
            config,
        )
        .await
        .map_err(|message| ProviderError::Network {
            provider_id: "slskd".to_string(),
            message,
        })?;
        if !detail_response.status().is_success() {
            continue;
        }
        let detail_body = detail_response
            .json::<Value>()
            .await
            .map_err(|error| ProviderError::Network {
                provider_id: "slskd".to_string(),
                message: error.to_string(),
            })?;
        ranked.extend(rank_slskd_candidates(&detail_body, request));
    }

    Ok(ranked)
}

async fn ensure_slskd_connected(
    client: &reqwest::Client,
    config: &SlskdConnectionConfig,
) -> Result<(), ProviderError> {
    let server_response = send_slskd_request(client.get(format!("{}/api/v0/server", config.url)), config)
        .await
        .map_err(|message| ProviderError::Network {
            provider_id: "slskd".to_string(),
            message,
        })?;
    let server_body = server_response
        .json::<Value>()
        .await
        .map_err(|error| ProviderError::Network {
            provider_id: "slskd".to_string(),
            message: error.to_string(),
        })?;
    let state = server_body
        .get("state")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    let connected = state.contains("connected");
    if connected {
        return Ok(());
    }

    let reconnect_response = send_slskd_request(
        client
            .put(format!("{}/api/v0/server", config.url))
            .json(&serde_json::json!({})),
        config,
    )
    .await
    .map_err(|message| ProviderError::Network {
        provider_id: "slskd".to_string(),
        message,
    })?;
    if !reconnect_response.status().is_success() {
        return Err(ProviderError::TemporaryOutage {
            provider_id: "slskd".to_string(),
            message: format!("failed to reconnect slskd server: HTTP {}", reconnect_response.status()),
        });
    }

    for _ in 0..10 {
        sleep(Duration::from_secs(2)).await;
        let server_response =
            send_slskd_request(client.get(format!("{}/api/v0/server", config.url)), config)
                .await
                .map_err(|message| ProviderError::Network {
                    provider_id: "slskd".to_string(),
                    message,
                })?;
        if !server_response.status().is_success() {
            continue;
        }
        let server_body = server_response
            .json::<Value>()
            .await
            .map_err(|error| ProviderError::Network {
                provider_id: "slskd".to_string(),
                message: error.to_string(),
            })?;
        let state = server_body
            .get("state")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_ascii_lowercase();
        if state.contains("connected") {
            return Ok(());
        }
    }

    Err(ProviderError::TemporaryOutage {
        provider_id: "slskd".to_string(),
        message: "slskd server did not reach connected state after reconnect attempts".to_string(),
    })
}

async fn create_search_with_recovery(
    client: &reqwest::Client,
    config: &SlskdConnectionConfig,
    query: &str,
) -> Result<Value, ProviderError> {
    for _attempt in 0..4 {
        let response = send_slskd_request(
            client
                .post(format!("{}/api/v0/searches", config.url))
                .json(&serde_json::json!({ "searchText": query })),
            config,
        )
        .await
        .map_err(|message| ProviderError::Network {
            provider_id: "slskd".to_string(),
            message,
        })?;

        let body = response
            .json::<Value>()
            .await
            .map_err(|error| ProviderError::Network {
                provider_id: "slskd".to_string(),
                message: error.to_string(),
            })?;

        if body.get("id").and_then(Value::as_str).is_some() {
            return Ok(body);
        }

        let message = serde_json::to_string(&body).unwrap_or_default().to_ascii_lowercase();
        if message.contains("must be connected and logged in") {
            ensure_slskd_connected(client, config).await?;
            sleep(Duration::from_secs(2)).await;
            continue;
        }

        return Ok(body);
    }

    Err(ProviderError::TemporaryOutage {
        provider_id: "slskd".to_string(),
        message: "unable to create slskd search".to_string(),
    })
}

async fn maybe_recover_search_queue(
    client: &reqwest::Client,
    config: &SlskdConnectionConfig,
) -> Result<(), ProviderError> {
    let response = send_slskd_request(client.get(format!("{}/api/v0/searches", config.url)), config)
        .await
        .map_err(|message| ProviderError::Network {
            provider_id: "slskd".to_string(),
            message,
        })?;

    if !response.status().is_success() {
        return Ok(());
    }

    let searches = response
        .json::<Vec<Value>>()
        .await
        .map_err(|error| ProviderError::Network {
            provider_id: "slskd".to_string(),
            message: error.to_string(),
        })?;

    let now = Utc::now();
    let mut queued = Vec::<String>::new();
    let mut stale_in_progress = Vec::<String>::new();

    for search in &searches {
        let state = search
            .get("state")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let started_at = search
            .get("startedAt")
            .and_then(Value::as_str)
            .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
            .map(|value| value.with_timezone(&Utc));
        let id = search
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        if id.is_empty() {
            continue;
        }

        if state.contains("Queued") {
            queued.push(id);
            continue;
        }

        if state.contains("InProgress") {
            let stale = started_at
                .map(|started| (now - started).num_minutes() >= 10)
                .unwrap_or(true);
            if stale {
                stale_in_progress.push(id);
            }
        }
    }

    // slskd can get wedged with an unbounded queued backlog and stale in-progress searches.
    // When that happens, new searches never run; purge the dead queue so downloads can continue.
    if queued.len() < 500 && stale_in_progress.is_empty() {
        return Ok(());
    }

    let mut to_delete = Vec::<String>::new();
    to_delete.extend(stale_in_progress);
    to_delete.extend(queued);

    const DELETE_CONCURRENCY: usize = 16;
    let mut in_flight = FuturesUnordered::new();
    let mut deleted = 0_usize;

    for id in to_delete {
        let req_client = client.clone();
        let req_config = config.clone();
        in_flight.push(async move {
            let result = send_slskd_request(
                req_client.delete(format!("{}/api/v0/searches/{}", req_config.url, id)),
                &req_config,
            )
            .await;
            matches!(result, Ok(response) if response.status().is_success())
        });

        if in_flight.len() >= DELETE_CONCURRENCY {
            if let Some(ok) = in_flight.next().await {
                if ok {
                    deleted += 1;
                }
            }
        }
    }

    while let Some(ok) = in_flight.next().await {
        if ok {
            deleted += 1;
        }
    }

    if deleted == 0 {
        return Err(ProviderError::TemporaryOutage {
            provider_id: "slskd".to_string(),
            message: "slskd search queue is saturated and could not be recovered".to_string(),
        });
    }

    Ok(())
}

fn slskd_query_candidates(request: &SlskdTrackRequest) -> Vec<String> {
    let mut queries = vec![
        build_query(&request.artist, &request.title, request.album.as_deref()),
        format!("{} {}", request.artist, request.title),
    ];
    if let Some(album) = request.album.as_deref().filter(|value| !value.trim().is_empty()) {
        queries.push(format!("{} {}", request.artist, album));
    }
    queries.push(request.artist.clone());
    // Preserve insertion order (most specific first) — only remove exact duplicates.
    let mut seen = std::collections::HashSet::new();
    queries.retain(|q| seen.insert(q.clone()));
    queries
}

fn rank_slskd_candidates(detail_body: &Value, request: &SlskdTrackRequest) -> Vec<(i64, SlskdCandidate)> {
    let artist_terms = normalized_terms(&request.artist);
    let title_terms = normalized_terms(&request.title);
    let album_terms = normalized_terms(request.album.as_deref().unwrap_or_default());

    slskd_response_items(detail_body)
        .into_iter()
        .flat_map(|response| {
            let username = response
                .get("username")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let queue_length = response
                .get("queueLength")
                .and_then(Value::as_u64)
                .unwrap_or_default();
            let has_free_upload_slot = response
                .get("hasFreeUploadSlot")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let artist_terms = artist_terms.clone();
            let title_terms = title_terms.clone();
            let album_terms = album_terms.clone();

            response
                .get("files")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(move |file| {
                    let filename = file.get("filename").and_then(Value::as_str).unwrap_or_default().to_string();
                    if filename.is_empty() || is_non_audio_path(&filename) {
                        return None;
                    }
                    let normalized = normalize_text(&filename);
                    let mut score = 0_i64;
                    score += (count_matching_terms(&normalized, &artist_terms) as i64) * 15;
                    score += (count_matching_terms(&normalized, &title_terms) as i64) * 25;
                    score += (count_matching_terms(&normalized, &album_terms) as i64) * 10;
                    if normalized.contains("flac") {
                        score += 120;
                    }
                    if normalized.contains("mp3") {
                        score += 20;
                    }
                    if has_free_upload_slot {
                        score += 20;
                    }
                    score -= (queue_length.min(200) as i64) / 2;
                    Some((
                        score,
                        SlskdCandidate {
                            username: username.clone(),
                            filename,
                            size: file.get("size").and_then(Value::as_u64).unwrap_or_default(),
                        },
                    ))
                })
        })
        .collect()
}

fn slskd_response_items(detail_body: &Value) -> Vec<&Value> {
    if let Some(items) = detail_body.as_array() {
        return items.iter().collect();
    }
    detail_body
        .get("responses")
        .and_then(Value::as_array)
        .map(|items| items.iter().collect())
        .unwrap_or_default()
}

fn find_matching_audio_file(roots: &[PathBuf], artist: &str, title: &str) -> Option<PathBuf> {
    let artist_key = normalize_text(artist);
    let title_key = normalize_text(title);
    roots.iter().find_map(|root| {
        if !root.exists() {
            return None;
        }
        walkdir::WalkDir::new(root)
            .follow_links(false)
            .into_iter()
            .filter_map(|entry| entry.ok())
            .find_map(|entry| {
                let path = entry.path();
                if !entry.file_type().is_file()
                    || is_non_audio_path(&path.to_string_lossy())
                    || !has_supported_audio_extension(path)
                {
                    return None;
                }
                let normalized = normalize_text(
                    path.file_name()
                        .and_then(|value| value.to_str())
                        .unwrap_or_default(),
                );
                if normalized.contains(&artist_key) && normalized.contains(&title_key) {
                    Some(path.to_path_buf())
                } else {
                    None
                }
            })
    })
}

fn sanitize(value: &str) -> String {
    value
        .chars()
        .map(|character| match character {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '\0' => '_',
            other => other,
        })
        .collect()
}

/// Check the transfer API response for a completed download matching the requested filename.
/// Returns the filename/path from the transfer data if the transfer is complete.
fn find_completed_transfer(transfer_data: &Value, requested_filename: &str) -> Option<String> {
    // The response may be structured as { "directories": [...] } or as an array directly.
    let directories = transfer_data
        .get("directories")
        .and_then(Value::as_array)
        .or_else(|| transfer_data.as_array());

    let dirs = directories?;
    for dir in dirs {
        let files = dir.get("files").and_then(Value::as_array);
        let files = files.or_else(|| dir.as_array());
        let Some(files) = files else { continue };
        for file in files {
            let fname = file
                .get("filename")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if fname.is_empty() || fname != requested_filename {
                continue;
            }
            let state = file
                .get("state")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_lowercase();
            // Also check the string form of state enum values slskd may return.
            let state_code = file
                .get("state")
                .and_then(Value::as_u64);
            let is_completed = state.contains("completed")
                || state.contains("succeeded")
                || state_code == Some(2); // 2 = Completed in slskd

            if !is_completed {
                continue;
            }

            let size = file.get("size").and_then(Value::as_u64).unwrap_or(0);
            let transferred = file
                .get("bytesTransferred")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            if size > 0 && transferred < size {
                continue;
            }
            return Some(fname.to_string());
        }
    }
    None
}

/// Try to resolve the downloaded file by checking if the filename from the transfer API
/// exists under any of the scan roots. Falls back to None if not found.
async fn resolve_downloaded_file(
    scan_roots: &[PathBuf],
    transfer_filename: &str,
    _artist: &str,
    _title: &str,
) -> Option<PathBuf> {
    // The transfer filename from soulseek uses backslash separators (Windows-style paths from
    // the remote peer). Extract just the file's basename.
    let basename = transfer_filename
        .rsplit(|c| c == '/' || c == '\\')
        .next()
        .unwrap_or(transfer_filename);

    for root in scan_roots {
        // Check the direct download location: root/<basename>
        let direct = root.join(basename);
        if has_supported_audio_extension(&direct) && tokio::fs::metadata(&direct).await.is_ok() {
            return Some(direct);
        }

        // Also try a recursive search in the root for this exact basename, but do it
        // asynchronously via spawn_blocking.
        let root = root.clone();
        let basename = basename.to_string();
        let found = tokio::task::spawn_blocking(move || {
            walkdir::WalkDir::new(&root)
                .follow_links(false)
                .into_iter()
                .filter_map(|entry| entry.ok())
                .find(|entry| {
                    entry.file_type().is_file()
                        && has_supported_audio_extension(entry.path())
                        && entry
                            .file_name()
                            .to_str()
                            .map(|name| name == basename)
                            .unwrap_or(false)
                })
                .map(|entry| entry.into_path())
        })
        .await
        .unwrap_or(None);

        if found.is_some() {
            return found;
        }
    }
    None
}

/// Copy a found file to the temp directory and return the acquisition result.
async fn copy_to_temp(
    found: &Path,
    candidate: &ProviderSearchCandidate,
    task: &TrackTask,
    temp_context: &TaskTempContext,
) -> Result<CandidateAcquisition, ProviderError> {
    let extension = found
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("bin")
        .to_string();
    let destination = temp_context
        .active_dir
        .join(format!("slskd-{}.{}", sanitize(&task.target.title), extension));
    tokio::fs::copy(found, &destination)
        .await
        .map_err(|error| ProviderError::Other {
            provider_id: "slskd".to_string(),
            message: error.to_string(),
        })?;
    let file_size = tokio::fs::metadata(&destination)
        .await
        .map(|metadata| metadata.len())
        .unwrap_or_default();
    if file_size < 1024 {
        return Err(ProviderError::InvalidAudio {
            provider_id: "slskd".to_string(),
            message: format!(
                "materialized file is implausibly small ({file_size} bytes): {}",
                destination.display()
            ),
        });
    }

    Ok(CandidateAcquisition {
        provider_id: "slskd".to_string(),
        provider_candidate_id: candidate.provider_candidate_id.clone(),
        temp_path: destination,
        file_size,
        extension_hint: Some(extension),
        resolved_metadata: None,
    })
}

fn has_supported_audio_extension(path: &Path) -> bool {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    matches!(
        extension.as_str(),
        "flac" | "mp3" | "m4a" | "aac" | "wav" | "ogg" | "opus" | "aiff" | "alac"
    )
}
