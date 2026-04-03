use crate::director::error::ProviderError;
use crate::director::models::{
    CandidateAcquisition, ProviderCapabilities, ProviderDescriptor, ProviderHealthState,
    ProviderHealthStatus, ProviderSearchCandidate, TrackTask,
};
use crate::director::provider::Provider;
use crate::director::strategy::StrategyPlan;
use crate::director::temp::TaskTempContext;
use crate::sources::{build_query, count_matching_terms, is_audio_path, normalize_text};
use async_trait::async_trait;
use serde_json::Value;
use std::path::PathBuf;
use tokio::time::{sleep, Duration};
use tracing::warn;

#[derive(Debug, Clone)]
pub struct UsenetProvider {
    pub api_key: Option<String>,
    pub sabnzbd_url: Option<String>,
    pub sabnzbd_api_key: Option<String>,
    pub scan_roots: Vec<PathBuf>,
}

#[async_trait]
impl Provider for UsenetProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        ProviderDescriptor {
            id: "usenet".to_string(),
            display_name: "Usenet".to_string(),
            trust_rank: 30,
            capabilities: ProviderCapabilities {
                supports_search: true,
                supports_download: true,
                supports_lossless: true,
                supports_batch: false,
            },
        }
    }

    async fn health_check(&self) -> Result<ProviderHealthState, ProviderError> {
        let checked_at = chrono::Utc::now();
        let Some(api_key) = self.api_key.as_deref() else {
            return Ok(ProviderHealthState {
                provider_id: "usenet".to_string(),
                status: ProviderHealthStatus::Unknown,
                checked_at,
                message: Some("nzbgeek API key not configured".to_string()),
            });
        };

        let client = reqwest::Client::new();
        let response = client
            .get("https://api.nzbgeek.info/api")
            .query(&[
                ("t", "caps"),
                ("o", "json"),
                ("apikey", api_key),
            ])
            .send()
            .await
            .map_err(|error| ProviderError::Network {
                provider_id: "usenet".to_string(),
                message: error.to_string(),
            })?;

        if !response.status().is_success() {
            return Err(ProviderError::TemporaryOutage {
                provider_id: "usenet".to_string(),
                message: format!("health probe returned HTTP {}", response.status()),
            });
        }

        Ok(ProviderHealthState {
            provider_id: "usenet".to_string(),
            status: ProviderHealthStatus::Healthy,
            checked_at,
            message: None,
        })
    }

    async fn search(
        &self,
        task: &TrackTask,
        _strategy: &StrategyPlan,
    ) -> Result<Vec<ProviderSearchCandidate>, ProviderError> {
        let Some(api_key) = self.api_key.as_deref() else {
            return Err(ProviderError::AuthFailed {
                provider_id: "usenet".to_string(),
            });
        };
        let client = reqwest::Client::new();
        let response = client
            .get("https://api.nzbgeek.info/api")
            .query(&[
                ("t", "search"),
                ("cat", "3000"),
                ("q", build_query(&task.target.artist, &task.target.title, task.target.album.as_deref()).as_str()),
                ("apikey", api_key),
                ("o", "json"),
                ("limit", "5"),
            ])
            .send()
            .await
            .map_err(|error| ProviderError::Network {
                provider_id: "usenet".to_string(),
                message: error.to_string(),
            })?;
        let body = response
            .json::<Value>()
            .await
            .map_err(|error| ProviderError::Network {
                provider_id: "usenet".to_string(),
                message: error.to_string(),
            })?;
        let items = body
            .pointer("/channel/item")
            .and_then(Value::as_array)
            .or_else(|| body.get("item").and_then(Value::as_array))
            .cloned()
            .unwrap_or_default();

        let artist_terms = vec![task.target.artist.to_ascii_lowercase()];
        let title_terms = vec![task.target.title.to_ascii_lowercase()];
        let mut scored = items
            .into_iter()
            .map(|item| {
                let title = item.get("title").and_then(Value::as_str).unwrap_or_default();
                let normalized = normalize_text(title);
                let mut score = 0_i64;
                score += (count_matching_terms(&normalized, &artist_terms) as i64) * 20;
                score += (count_matching_terms(&normalized, &title_terms) as i64) * 30;
                if normalized.contains("flac") {
                    score += 100;
                }
                if normalized.contains("24bit") || normalized.contains("24 bit") {
                    score += 50;
                }
                (score, item)
            })
            .collect::<Vec<_>>();
        scored.sort_by(|left, right| right.0.cmp(&left.0));

        Ok(scored
            .into_iter()
            .take(5)
            .filter_map(|(_, item)| {
                let link = item
                    .get("link")
                    .and_then(Value::as_str)
                    .or_else(|| item.pointer("/enclosure/url").and_then(Value::as_str))
                    .or_else(|| item.get("guid").and_then(Value::as_str))?;
                Some(ProviderSearchCandidate {
                    provider_id: "usenet".to_string(),
                    provider_candidate_id: link.to_string(),
                    artist: task.target.artist.clone(),
                    title: task.target.title.clone(),
                    album: task.target.album.clone(),
                    duration_secs: task.target.duration_secs,
                    extension_hint: Some("nzb".to_string()),
                    bitrate_kbps: None,
                    cover_art_url: None,
                    metadata_confidence: 0.75,
                })
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
        let Some(api_key) = self.api_key.as_deref() else {
            return Err(ProviderError::AuthFailed {
                provider_id: "usenet".to_string(),
            });
        };
        let client = reqwest::Client::new();
        let nzb_bytes = client
            .get(&candidate.provider_candidate_id)
            .query(&[("apikey", api_key)])
            .send()
            .await
            .map_err(|error| ProviderError::Network {
                provider_id: "usenet".to_string(),
                message: error.to_string(),
            })?
            .bytes()
            .await
            .map_err(|error| ProviderError::Network {
                provider_id: "usenet".to_string(),
                message: error.to_string(),
            })?;

        let nzb_path = temp_context
            .active_dir
            .join(format!("{}-{}.nzb", sanitize(&task.target.artist), sanitize(&task.target.title)));
        tokio::fs::write(&nzb_path, &nzb_bytes)
            .await
            .map_err(|error| ProviderError::Other {
                provider_id: "usenet".to_string(),
                message: error.to_string(),
            })?;

        let mut sab_job_id: Option<String> = None;
        if let (Some(sabnzbd_url), Some(sabnzbd_api_key)) =
            (self.sabnzbd_url.as_deref(), self.sabnzbd_api_key.as_deref())
        {
            let form = reqwest::multipart::Form::new()
                .text("mode", "addfile")
                .text("apikey", sabnzbd_api_key.to_string())
                .text("cat", "audio")
                .text("output", "json")
                .part(
                    "name",
                    reqwest::multipart::Part::bytes(nzb_bytes.to_vec())
                        .file_name("cassette.nzb")
                        .mime_str("application/x-nzb")
                        .unwrap_or_else(|_| unreachable!()),
                );
            let sab_response = client
                .post(format!("{sabnzbd_url}/api"))
                .multipart(form)
                .send()
                .await
                .map_err(|error| ProviderError::Network {
                    provider_id: "usenet".to_string(),
                    message: format!("SABnzbd submit failed: {error}"),
                })?;
            if !sab_response.status().is_success() {
                return Err(ProviderError::Network {
                    provider_id: "usenet".to_string(),
                    message: format!("SABnzbd returned HTTP {}", sab_response.status()),
                });
            }
            let sab_body = sab_response
                .json::<Value>()
                .await
                .unwrap_or_else(|_| Value::Null);
            sab_job_id = extract_sab_job_id(&sab_body);
        }

        for _ in 0..24 {
            sleep(Duration::from_secs(5)).await;
            let mut scan_roots = self.scan_roots.clone();
            if let (Some(sabnzbd_url), Some(sabnzbd_api_key), Some(job_id)) = (
                self.sabnzbd_url.as_deref(),
                self.sabnzbd_api_key.as_deref(),
                sab_job_id.as_deref(),
            ) {
                match sab_completion_paths(&client, sabnzbd_url, sabnzbd_api_key, job_id).await {
                    Ok(extra_roots) => scan_roots.extend(extra_roots),
                    Err(error) => warn!(provider = "usenet", %job_id, %error, "SAB queue/history poll failed"),
                }
            }

            if let Some(found) = find_matching_audio_file(&scan_roots, &task.target.artist, &task.target.title) {
                let extension = found
                    .extension()
                    .and_then(|value| value.to_str())
                    .unwrap_or("bin")
                    .to_string();
                let destination = temp_context
                    .active_dir
                    .join(format!("usenet-{}.{}", sanitize(&task.target.title), extension));
                tokio::fs::copy(&found, &destination)
                    .await
                    .map_err(|error| ProviderError::Other {
                        provider_id: "usenet".to_string(),
                        message: error.to_string(),
                    })?;
                let file_size = tokio::fs::metadata(&destination)
                    .await
                    .map(|metadata| metadata.len())
                    .unwrap_or_default();
                return Ok(CandidateAcquisition {
                    provider_id: "usenet".to_string(),
                    provider_candidate_id: candidate.provider_candidate_id.clone(),
                    temp_path: destination,
                    file_size,
                    extension_hint: Some(extension),
                    resolved_metadata: None,
                });
            }
        }

        Err(ProviderError::TemporaryOutage {
            provider_id: "usenet".to_string(),
            message: "NZB submitted but no finalized audio file appeared in watched roots".to_string(),
        })
    }
}

fn extract_sab_job_id(body: &Value) -> Option<String> {
    body.pointer("/nzo_ids/0")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .or_else(|| body.get("nzo_id").and_then(Value::as_str).map(ToString::to_string))
}

async fn sab_completion_paths(
    client: &reqwest::Client,
    sabnzbd_url: &str,
    sabnzbd_api_key: &str,
    nzo_id: &str,
) -> Result<Vec<PathBuf>, ProviderError> {
    let queue_body = client
        .get(format!("{sabnzbd_url}/api"))
        .query(&[
            ("mode", "queue"),
            ("apikey", sabnzbd_api_key),
            ("output", "json"),
            ("limit", "50"),
        ])
        .send()
        .await
        .map_err(|error| ProviderError::Network {
            provider_id: "usenet".to_string(),
            message: format!("SABnzbd queue poll failed: {error}"),
        })?
        .json::<Value>()
        .await
        .map_err(|error| ProviderError::Network {
            provider_id: "usenet".to_string(),
            message: format!("SABnzbd queue JSON parse failed: {error}"),
        })?;

    if let Some(slot) = find_sab_slot(&queue_body, nzo_id) {
        let status = sab_slot_status(slot).unwrap_or_default();
        if matches!(status.as_str(), "downloading" | "queued" | "fetching" | "grabbing") {
            return Ok(collect_sab_slot_paths(slot));
        }
    }

    let history_body = client
        .get(format!("{sabnzbd_url}/api"))
        .query(&[
            ("mode", "history"),
            ("apikey", sabnzbd_api_key),
            ("output", "json"),
            ("limit", "50"),
        ])
        .send()
        .await
        .map_err(|error| ProviderError::Network {
            provider_id: "usenet".to_string(),
            message: format!("SABnzbd history poll failed: {error}"),
        })?
        .json::<Value>()
        .await
        .map_err(|error| ProviderError::Network {
            provider_id: "usenet".to_string(),
            message: format!("SABnzbd history JSON parse failed: {error}"),
        })?;

    if let Some(slot) = find_sab_slot(&history_body, nzo_id) {
        return Ok(collect_sab_slot_paths(slot));
    }

    Ok(Vec::new())
}

fn find_sab_slot<'a>(body: &'a Value, nzo_id: &str) -> Option<&'a Value> {
    body.pointer("/queue/slots")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .chain(
            body.pointer("/history/slots")
                .and_then(Value::as_array)
                .into_iter()
                .flatten(),
        )
        .find(|slot| {
            slot.get("nzo_id")
                .and_then(Value::as_str)
                .map(|value| value == nzo_id)
                .unwrap_or(false)
        })
}

fn sab_slot_status(slot: &Value) -> Option<String> {
    slot.get("status")
        .and_then(Value::as_str)
        .map(|value| value.to_ascii_lowercase())
}

fn collect_sab_slot_paths(slot: &Value) -> Vec<PathBuf> {
    ["storage", "path", "completed_path"]
        .iter()
        .filter_map(|key| slot.get(*key).and_then(Value::as_str))
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
        .collect()
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
                if !entry.file_type().is_file() || !is_audio_path(path) {
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
