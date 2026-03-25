use crate::director::sources::{ResolvedTrack, SourceError, SourceProvider};
use crate::librarian::models::DesiredTrack;
use async_trait::async_trait;

#[derive(Debug, Clone, Default)]
pub struct YoutubeSource;

#[async_trait]
impl SourceProvider for YoutubeSource {
    fn name(&self) -> &'static str {
        "youtube"
    }

    fn can_handle(&self, track: &DesiredTrack) -> bool {
        extract_youtube_url(track).is_some()
    }

    async fn resolve_download_url(&self, track: &DesiredTrack) -> Result<ResolvedTrack, SourceError> {
        let url = extract_youtube_url(track)
            .ok_or_else(|| SourceError::NotAvailable("No YouTube URL in desired payload".to_string()))?;

        Ok(ResolvedTrack {
            download_url: url.clone(),
            suggested_filename: format!("{} - {}.m4a", sanitize(&track.artist_name), sanitize(&track.track_title)),
            expected_codec: Some("m4a".to_string()),
            expected_bitrate: Some(128),
            expected_duration_ms: track.duration_ms.map(|d| d as u64),
            metadata: serde_json::json!({"source": "youtube", "url": url}),
        })
    }

    async fn check_availability(&self, track: &DesiredTrack) -> Result<bool, SourceError> {
        Ok(extract_youtube_url(track).is_some())
    }
}

fn extract_youtube_url(track: &DesiredTrack) -> Option<String> {
    let payload = track.raw_payload_json.as_ref()?;
    let value: serde_json::Value = serde_json::from_str(payload).ok()?;
    let candidate = value
        .get("youtube_url")
        .and_then(|v| v.as_str())
        .or_else(|| value.get("url").and_then(|v| v.as_str()))?;
    let lower = candidate.to_ascii_lowercase();
    if lower.contains("youtube.com") || lower.contains("youtu.be") {
        return Some(candidate.to_string());
    }
    None
}

fn sanitize(value: &str) -> String {
    value
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == ' ' || c == '-' { c } else { '_' })
        .collect::<String>()
        .trim()
        .to_string()
}
