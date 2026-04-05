use crate::director::sources::{ResolvedTrack, SourceError, SourceProvider};
use crate::librarian::models::DesiredTrack;
use async_trait::async_trait;

#[derive(Debug, Clone, Default)]
pub struct BandcampSource;

#[async_trait]
impl SourceProvider for BandcampSource {
    fn name(&self) -> &'static str {
        "bandcamp"
    }

    fn can_handle(&self, track: &DesiredTrack) -> bool {
        extract_bandcamp_url(track).is_some()
    }

    async fn resolve_download_url(&self, track: &DesiredTrack) -> Result<ResolvedTrack, SourceError> {
        let url = extract_bandcamp_url(track)
            .ok_or_else(|| SourceError::NotAvailable("No Bandcamp URL in desired payload".to_string()))?;

        Ok(ResolvedTrack {
            download_url: url.clone(),
            suggested_filename: format!(
                "{} - {}.mp3",
                sanitize_component(&track.artist_name),
                sanitize_component(&track.track_title)
            ),
            expected_codec: Some("mp3".to_string()),
            expected_bitrate: None,
            expected_duration_ms: track.duration_ms.map(|duration| duration as u64),
            metadata: serde_json::json!({"source": "bandcamp", "url": url}),
        })
    }

    async fn check_availability(&self, track: &DesiredTrack) -> Result<bool, SourceError> {
        Ok(extract_bandcamp_url(track).is_some())
    }
}

fn extract_bandcamp_url(track: &DesiredTrack) -> Option<String> {
    let payload = track.raw_payload_json.as_ref()?;
    let value: serde_json::Value = serde_json::from_str(payload).ok()?;

    for key in ["bandcamp_url", "url", "download_url"] {
        if let Some(candidate) = value.get(key).and_then(|item| item.as_str()) {
            let lowered = candidate.to_ascii_lowercase();
            if lowered.contains("bandcamp.com") {
                return Some(candidate.to_string());
            }
        }
    }

    value
        .get("external_urls")
        .and_then(|urls| urls.as_object())
        .and_then(|urls| {
            urls.values().find_map(|value| {
                value.as_str().and_then(|candidate| {
                    if candidate.to_ascii_lowercase().contains("bandcamp.com") {
                        Some(candidate.to_string())
                    } else {
                        None
                    }
                })
            })
        })
}

fn sanitize_component(value: &str) -> String {
    value
        .trim()
        .chars()
        .map(|character| match character {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '\0' => '_',
            other => other,
        })
        .collect::<String>()
        .trim()
        .trim_end_matches('.')
        .to_string()
}
