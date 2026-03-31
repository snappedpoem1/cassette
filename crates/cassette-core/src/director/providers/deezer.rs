use crate::director::error::ProviderError;
use crate::director::models::{
    CandidateAcquisition, ProviderCapabilities, ProviderDescriptor, ProviderSearchCandidate,
    TrackTask,
};
use crate::director::provider::Provider;
use crate::director::providers::crypto::stream_decrypt_deezer_to_file;
use crate::director::strategy::StrategyPlan;
use crate::director::temp::TaskTempContext;
use crate::librarian::matchers::fuzzy::levenshtein;
use crate::sources::{
    build_query, deezer_client, deezer_get_media_url, deezer_get_track_data, deezer_get_user_data,
    RemoteProviderConfig,
};
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
/// Cached Deezer session so we authenticate once, not per-request.
struct DeezerSessionCache {
    client: reqwest::Client,
    api_token: String,
    license_token: String,
}

#[derive(Clone)]
pub struct DeezerProvider {
    config: RemoteProviderConfig,
    session: Arc<RwLock<Option<DeezerSessionCache>>>,
}

impl std::fmt::Debug for DeezerProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeezerProvider").finish()
    }
}

impl DeezerProvider {
    pub fn new(config: RemoteProviderConfig) -> Self {
        Self {
            config,
            session: Arc::new(RwLock::new(None)),
        }
    }

    async fn ensure_session(&self) -> Result<(), ProviderError> {
        if self.session.read().await.is_some() {
            return Ok(());
        }

        let mut guard = self.session.write().await;
        if guard.is_some() {
            return Ok(());
        }

        let client = deezer_client(&self.config).map_err(|error| ProviderError::Other {
            provider_id: "deezer".to_string(),
            message: format!("auth failed: {error}"),
        })?;
        let session = deezer_get_user_data(&client)
            .await
            .map_err(|message| ProviderError::Other {
                provider_id: "deezer".to_string(),
                message: format!("auth failed: {message}"),
            })?;
        *guard = Some(DeezerSessionCache {
            client,
            api_token: session.api_token,
            license_token: session.license_token,
        });
        Ok(())
    }

    async fn invalidate_session(&self) {
        let mut guard = self.session.write().await;
        *guard = None;
    }

    async fn session_snapshot(&self) -> Result<DeezerSessionCache, ProviderError> {
        self.ensure_session().await?;
        let guard = self.session.read().await;
        guard.clone().ok_or_else(|| ProviderError::AuthFailed {
            provider_id: "deezer".to_string(),
        })
    }
}

#[async_trait]
impl Provider for DeezerProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        ProviderDescriptor {
            id: "deezer".to_string(),
            display_name: "Deezer".to_string(),
            trust_rank: 5,
            capabilities: ProviderCapabilities {
                supports_search: true,
                supports_download: true,
                supports_lossless: true,
                supports_batch: false,
            },
        }
    }

    async fn search(
        &self,
        task: &TrackTask,
        _strategy: &StrategyPlan,
    ) -> Result<Vec<ProviderSearchCandidate>, ProviderError> {
        let session = self.session_snapshot().await?;

        let query = build_query(&task.target.artist, &task.target.title, task.target.album.as_deref());
        let response = session
            .client
            .get("https://api.deezer.com/search/track")
            .query(&[("q", query.as_str()), ("limit", "10")])
            .send()
            .await
            .map_err(|error| ProviderError::Network {
                provider_id: "deezer".to_string(),
                message: error.to_string(),
            })?;
        if response.status().as_u16() == 401 || response.status().as_u16() == 403 {
            self.invalidate_session().await;
            return Err(ProviderError::AuthFailed {
                provider_id: "deezer".to_string(),
            });
        }
        if !response.status().is_success() {
            return Err(ProviderError::Network {
                provider_id: "deezer".to_string(),
                message: format!("search returned HTTP {}", response.status()),
            });
        }

        let body = response
            .json::<Value>()
            .await
            .map_err(|error| ProviderError::Network {
                provider_id: "deezer".to_string(),
                message: error.to_string(),
            })?;

        Ok(body
            .get("data")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|item| {
                let track_id = item
                    .get("id")
                    .map(Value::to_string)
                    .unwrap_or_default()
                    .trim_matches('"')
                    .to_string();
                if track_id.is_empty() {
                    return None;
                }

                let artist = item
                    .get("artist")
                    .and_then(|artist| artist.get("name"))
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let title = item
                    .get("title")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let album = item
                    .get("album")
                    .and_then(|album| album.get("title"))
                    .and_then(Value::as_str)
                    .map(ToString::to_string);
                let cover_art_url = item
                    .get("album")
                    .and_then(|album| {
                        album
                            .get("cover_xl")
                            .or_else(|| album.get("cover_big"))
                            .or_else(|| album.get("cover_medium"))
                    })
                    .and_then(Value::as_str)
                    .map(ToString::to_string);

                let mut confidence = 0.20_f32;
                confidence += normalized_match_confidence(&artist, &task.target.artist) * 0.40;
                confidence += normalized_match_confidence(&title, &task.target.title) * 0.30;
                if let (Some(target_album), Some(candidate_album)) =
                    (task.target.album.as_deref(), album.as_deref())
                {
                    confidence += normalized_match_confidence(candidate_album, target_album) * 0.10;
                }

                Some(ProviderSearchCandidate {
                    provider_id: "deezer".to_string(),
                    provider_candidate_id: track_id,
                    artist,
                    title,
                    album,
                    duration_secs: item.get("duration").and_then(Value::as_u64).map(|value| value as f64),
                    extension_hint: Some("flac".to_string()),
                    bitrate_kbps: None,
                    cover_art_url,
                    metadata_confidence: confidence.clamp(0.0, 0.95),
                })
            })
            .collect())
    }

    async fn acquire(
        &self,
        _task: &TrackTask,
        candidate: &ProviderSearchCandidate,
        temp_context: &TaskTempContext,
        _strategy: &StrategyPlan,
    ) -> Result<CandidateAcquisition, ProviderError> {
        let session = self.session_snapshot().await?;
        let track_id = &candidate.provider_candidate_id;

        // 1. Get track token via private gateway API
        let track_data = deezer_get_track_data(&session.client, &session.api_token, track_id)
            .await
            .map_err(|message| ProviderError::Network {
                provider_id: "deezer".to_string(),
                message,
            })?;

        // 2. Get encrypted media CDN URL (tries FLAC → 320 → 128)
        let (media_url, extension) =
            deezer_get_media_url(&session.client, &session.license_token, &track_data.track_token)
                .await
                .map_err(|message| ProviderError::UnsupportedContent {
                    provider_id: "deezer".to_string(),
                    message,
                })?;

        // 3. Download encrypted stream
        let response = session
            .client
            .get(&media_url)
            .send()
            .await
            .map_err(|error| ProviderError::Network {
                provider_id: "deezer".to_string(),
                message: error.to_string(),
            })?;
        if response.status().as_u16() == 401 || response.status().as_u16() == 403 {
            self.invalidate_session().await;
            return Err(ProviderError::AuthFailed {
                provider_id: "deezer".to_string(),
            });
        }
        if !response.status().is_success() {
            return Err(ProviderError::Network {
                provider_id: "deezer".to_string(),
                message: format!("CDN download HTTP {}", response.status()),
            });
        }

        // 5. Write decrypted file
        let filename = format!(
            "deezer-{}.{}",
            sanitize(&candidate.provider_candidate_id),
            extension,
        );
        let destination = temp_context.active_dir.join(filename);
        let written = stream_decrypt_deezer_to_file(response, &destination, &track_data.track_id)
            .await
            .map_err(|message| ProviderError::Other {
                provider_id: "deezer".to_string(),
                message,
            })?;
        if written == 0 {
            return Err(ProviderError::InvalidAudio {
                provider_id: "deezer".to_string(),
                message: "empty CDN payload".to_string(),
            });
        }

        Ok(CandidateAcquisition {
            provider_id: "deezer".to_string(),
            provider_candidate_id: candidate.provider_candidate_id.clone(),
            temp_path: destination,
            file_size: written,
            extension_hint: Some(extension),
        })
    }
}

fn normalize(value: &str) -> String {
    normalize_match_noise(value)
        .to_ascii_lowercase()
        .chars()
        .map(|character| if character.is_alphanumeric() { character } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn normalize_match_noise(value: &str) -> String {
    value
        .replace("pt.", "part")
        .replace("pt ", "part ")
        .replace("feat.", "feat ")
        .replace("featuring", "feat")
        .replace("deluxe edition", "")
        .replace("expanded edition", "")
        .replace("remastered", "")
        .replace("remaster", "")
        .replace("edition", "")
}

fn normalized_match_confidence(candidate: &str, target: &str) -> f32 {
    let candidate = normalize(candidate);
    let target = normalize(target);
    if candidate.is_empty() || target.is_empty() {
        return 0.0;
    }
    if candidate == target {
        return 1.0;
    }
    if candidate.contains(&target) || target.contains(&candidate) {
        return 0.94;
    }

    let distance = levenshtein(&candidate, &target);
    let max_len = candidate.chars().count().max(target.chars().count());
    if max_len == 0 {
        return 0.0;
    }
    let similarity = 1.0 - (distance as f32 / max_len as f32);
    if similarity >= 0.90 {
        similarity
    } else {
        0.0
    }
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
