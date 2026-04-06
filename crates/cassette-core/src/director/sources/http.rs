use crate::director::sources::{ResolvedTrack, SourceError, SourceProvider};
use crate::librarian::models::DesiredTrack;
use async_trait::async_trait;

#[derive(Debug, Clone, Default)]
pub struct HttpSource {
    client: reqwest::Client,
}

impl HttpSource {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl SourceProvider for HttpSource {
    fn name(&self) -> &'static str {
        "http"
    }

    fn can_handle(&self, track: &DesiredTrack) -> bool {
        extract_http_url(track).is_some()
    }

    async fn resolve_download_url(
        &self,
        track: &DesiredTrack,
    ) -> Result<ResolvedTrack, SourceError> {
        let url = extract_http_url(track).ok_or_else(|| {
            SourceError::NotAvailable("No HTTP URL in desired track payload".to_string())
        })?;

        let filename = format!(
            "{} - {}.bin",
            sanitize_component(&track.artist_name),
            sanitize_component(&track.track_title)
        );

        Ok(ResolvedTrack {
            download_url: url.clone(),
            suggested_filename: filename,
            expected_codec: None,
            expected_bitrate: None,
            expected_duration_ms: track.duration_ms.map(|d| d as u64),
            metadata: serde_json::json!({"source": "http", "url": url}),
        })
    }

    async fn check_availability(&self, track: &DesiredTrack) -> Result<bool, SourceError> {
        let url = extract_http_url(track).ok_or_else(|| {
            SourceError::NotAvailable("No HTTP URL in desired track payload".to_string())
        })?;

        let response = self
            .client
            .head(url)
            .send()
            .await
            .map_err(|error| SourceError::ApiError(error.to_string()))?;
        Ok(response.status().is_success())
    }
}

fn extract_http_url(track: &DesiredTrack) -> Option<String> {
    let payload = track.raw_payload_json.as_ref()?;
    let value: serde_json::Value = serde_json::from_str(payload).ok()?;

    // Accept several payload conventions for local flexibility.
    for key in ["download_url", "url", "http_url", "direct_url"] {
        if let Some(url) = value.get(key).and_then(|v| v.as_str()) {
            if url.starts_with("http://") || url.starts_with("https://") {
                return Some(url.to_string());
            }
        }
    }

    value
        .get("external_urls")
        .and_then(|v| v.as_object())
        .and_then(|map| {
            map.values().find_map(|v| {
                v.as_str().and_then(|url| {
                    if url.starts_with("http://") || url.starts_with("https://") {
                        Some(url.to_string())
                    } else {
                        None
                    }
                })
            })
        })
}

fn sanitize_component(value: &str) -> String {
    let cleaned = value
        .trim()
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '\0' => '_',
            other => other,
        })
        .collect::<String>();
    cleaned.trim().trim_end_matches('.').to_string()
}
