use crate::director::error::ProviderError;
use crate::director::models::{
    CandidateAcquisition, ProviderCapabilities, ProviderDescriptor, ProviderHealthState,
    ProviderHealthStatus, ProviderSearchCandidate, TrackTask,
};
use crate::director::provider::Provider;
use crate::director::strategy::StrategyPlan;
use crate::director::temp::TaskTempContext;
use crate::sources::{build_query, qobuz_search, qobuz_user_auth_token, RemoteProviderConfig};
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Cached Qobuz session so we authenticate once, not per-request.
/// Wrapped in RwLock so we can invalidate a stale token and re-auth.
struct QobuzSessionCache {
    client: reqwest::Client,
    user_auth_token: String,
}

#[derive(Clone)]
pub struct QobuzProvider {
    config: RemoteProviderConfig,
    session: Arc<RwLock<Option<QobuzSessionCache>>>,
}

impl std::fmt::Debug for QobuzProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QobuzProvider").finish()
    }
}

impl QobuzProvider {
    pub fn new(config: RemoteProviderConfig) -> Self {
        Self {
            config,
            session: Arc::new(RwLock::new(None)),
        }
    }

    async fn ensure_session(&self) -> Result<(), ProviderError> {
        // Fast path: session already exists
        if self.session.read().await.is_some() {
            return Ok(());
        }

        let mut guard = self.session.write().await;
        // Double-check after acquiring write lock
        if guard.is_some() {
            return Ok(());
        }

        let token = qobuz_user_auth_token(&self.config)
            .await
            .map_err(|_| ProviderError::AuthFailed {
                provider_id: "qobuz".to_string(),
            })?
            .ok_or_else(|| ProviderError::AuthFailed {
                provider_id: "qobuz".to_string(),
            })?;
        *guard = Some(QobuzSessionCache {
            client: reqwest::Client::new(),
            user_auth_token: token,
        });
        Ok(())
    }

    /// Invalidate the cached session so the next call re-authenticates.
    async fn invalidate_session(&self) {
        let mut guard = self.session.write().await;
        *guard = None;
    }
}

#[async_trait]
impl Provider for QobuzProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        ProviderDescriptor {
            id: "qobuz".to_string(),
            display_name: "Qobuz".to_string(),
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
        self.ensure_session().await?;
        Ok(ProviderHealthState {
            provider_id: "qobuz".to_string(),
            status: ProviderHealthStatus::Healthy,
            checked_at: chrono::Utc::now(),
            message: None,
        })
    }

    async fn search(
        &self,
        task: &TrackTask,
        _strategy: &StrategyPlan,
    ) -> Result<Vec<ProviderSearchCandidate>, ProviderError> {
        self.ensure_session().await?;

        let mut albums = Vec::new();
        for query in qobuz_query_candidates(task) {
            let result = qobuz_search(&self.config, &query)
                .await
                .map_err(|message| ProviderError::Network {
                    provider_id: "qobuz".to_string(),
                    message,
                })?;
            if !result.albums.is_empty() {
                albums = result.albums;
                break;
            }
        }

        Ok(albums
            .into_iter()
            .map(|album| {
                let mut confidence = 0.65_f32;
                if normalize(&album.artist).contains(&normalize(&task.target.artist)) {
                    confidence += 0.20;
                }
                if let Some(target_album) = task.target.album.as_deref() {
                    if normalize(&album.title).contains(&normalize(target_album)) {
                        confidence += 0.10;
                    }
                }

                ProviderSearchCandidate {
                    provider_id: "qobuz".to_string(),
                    provider_candidate_id: album.id,
                    artist: album.artist,
                    title: task.target.title.clone(),
                    album: Some(album.title),
                    duration_secs: task.target.duration_secs,
                    extension_hint: Some("flac".to_string()),
                    bitrate_kbps: Some(1000),
                    cover_art_url: album.cover_url,
                    metadata_confidence: confidence.clamp(0.0, 0.95),
                }
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
        let Some(app_id) = self.config.qobuz_app_id.as_deref() else {
            return Err(ProviderError::AuthFailed {
                provider_id: "qobuz".to_string(),
            });
        };
        self.ensure_session().await?;
        let session_guard = self.session.read().await;
        let session = session_guard.as_ref().ok_or_else(|| ProviderError::AuthFailed {
            provider_id: "qobuz".to_string(),
        })?;
        let client = &session.client;
        let user_auth_token = &session.user_auth_token;

        let album_response = client
            .get("https://www.qobuz.com/api.json/0.2/album/get")
            .query(&[
                ("album_id", candidate.provider_candidate_id.as_str()),
                ("app_id", app_id),
                ("user_auth_token", user_auth_token.as_str()),
            ])
            .send()
            .await
            .map_err(|error| ProviderError::Network {
                provider_id: "qobuz".to_string(),
                message: error.to_string(),
            })?;
        if album_response.status().as_u16() == 401 || album_response.status().as_u16() == 403 {
            drop(session_guard);
            self.invalidate_session().await;
            return Err(ProviderError::AuthFailed {
                provider_id: "qobuz".to_string(),
            });
        }
        if !album_response.status().is_success() {
            return Err(ProviderError::NotFound {
                provider_id: "qobuz".to_string(),
            });
        }

        let album_body = album_response
            .json::<Value>()
            .await
            .map_err(|error| ProviderError::Network {
                provider_id: "qobuz".to_string(),
                message: error.to_string(),
            })?;
        let tracks = album_body
            .pointer("/tracks/items")
            .and_then(Value::as_array)
            .ok_or_else(|| ProviderError::NotFound {
                provider_id: "qobuz".to_string(),
            })?;
        // Try to find a track matching the task's title (for single-track tasks).
        // If the title matches the album name (album-level task), fall back to the first track.
        let track_item = tracks
            .iter()
            .find(|item| {
                normalize(item.get("title").and_then(Value::as_str).unwrap_or_default())
                    .contains(&normalize(&task.target.title))
            })
            .or_else(|| tracks.first())
            .ok_or_else(|| ProviderError::NotFound {
                provider_id: "qobuz".to_string(),
            })?;

        let track_id = track_item
            .get("id")
            .map(Value::to_string)
            .unwrap_or_default()
            .trim_matches('"')
            .to_string();
        if track_id.is_empty() {
            return Err(ProviderError::NotFound {
                provider_id: "qobuz".to_string(),
            });
        }

        let timestamp = chrono::Utc::now().timestamp() as u64;
        let format_id = "27";
        let mut file_url = None::<String>;
        let mut secrets = Vec::<String>::new();
        if let Some(secret) = self
            .config
            .qobuz_app_secret
            .as_deref()
            .map(str::trim)
            .filter(|secret| !secret.is_empty())
        {
            secrets.push(secret.to_string());
        }
        secrets.extend(
            self.config
                .qobuz_secrets
                .clone()
                .unwrap_or_default()
                .split(',')
                .map(str::trim)
                .filter(|secret| !secret.is_empty())
                .map(ToString::to_string),
        );

        if secrets.is_empty() {
            return Err(ProviderError::AuthFailed {
                provider_id: "qobuz".to_string(),
            });
        }

        for secret in &secrets {
            let sig_input = format!(
                "trackgetFileUrlformat_id{format_id}intentstreamtrack_id{track_id}{timestamp}{secret}"
            );
            let digest = md5::compute(sig_input.as_bytes());
            let sig = format!("{digest:x}");
            let response = client
                .get("https://www.qobuz.com/api.json/0.2/track/getFileUrl")
                .query(&[
                    ("track_id", track_id.as_str()),
                    ("format_id", format_id),
                    ("intent", "stream"),
                    ("request_ts", &timestamp.to_string()),
                    ("request_sig", &sig),
                    ("app_id", app_id),
                    ("user_auth_token", user_auth_token.as_str()),
                ])
                .send()
                .await;

            if let Ok(response) = response {
                if response.status().is_success() {
                    if let Ok(body) = response.json::<Value>().await {
                        if let Some(url) = body.get("url").and_then(Value::as_str) {
                            file_url = Some(url.to_string());
                            break;
                        }
                    }
                }
            }
        }

        let Some(url) = file_url else {
            return Err(ProviderError::UnsupportedContent {
                provider_id: "qobuz".to_string(),
                message: "unable to obtain a direct file url".to_string(),
            });
        };

        // format_id 27 requests a lossless stream; some Qobuz URLs don't expose the
        // extension cleanly, so default to FLAC unless the URL clearly says MP3.
        let extension = if url.to_ascii_lowercase().contains(".mp3") {
            "mp3"
        } else {
            "flac"
        };
        // Include the track_id in the filename so that different candidates (different
        // albums/tracks for the same task) each get a unique temp file. Without this,
        // all candidates would overwrite the same path, and a later validation failure
        // could quarantine a file that an earlier valid candidate was relying on.
        let filename = format!(
            "qobuz-{}-{}.{}",
            sanitize(&track_id),
            sanitize(&task.target.title),
            extension
        );
        let destination = temp_context.active_dir.join(filename);
        let bytes = client
            .get(&url)
            .send()
            .await
            .map_err(|error| ProviderError::Network {
                provider_id: "qobuz".to_string(),
                message: error.to_string(),
            })?
            .bytes()
            .await
            .map_err(|error| ProviderError::Network {
                provider_id: "qobuz".to_string(),
                message: error.to_string(),
            })?;
        tokio::fs::write(&destination, &bytes)
            .await
            .map_err(|error| ProviderError::Other {
                provider_id: "qobuz".to_string(),
                message: error.to_string(),
            })?;

        Ok(CandidateAcquisition {
            provider_id: "qobuz".to_string(),
            provider_candidate_id: track_id,
            temp_path: destination,
            file_size: bytes.len() as u64,
            extension_hint: Some(extension.to_string()),
        })
    }
}

fn normalize(value: &str) -> String {
    value
        .to_ascii_lowercase()
        .chars()
        .map(|character| if character.is_alphanumeric() { character } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
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

fn qobuz_query_candidates(task: &TrackTask) -> Vec<String> {
    let mut queries = Vec::new();
    if let Some(album) = task.target.album.as_deref().filter(|value| !value.trim().is_empty()) {
        queries.push(format!("{} {}", task.target.artist, album));
    }
    queries.push(build_query(
        &task.target.artist,
        &task.target.title,
        task.target.album.as_deref(),
    ));
    queries.push(format!("{} {}", task.target.artist, task.target.title));
    queries.push(task.target.artist.clone());
    // Preserve insertion order (most specific first) — only remove exact duplicates.
    let mut seen = std::collections::HashSet::new();
    queries.retain(|q| seen.insert(q.clone()));
    queries
}
