use crate::director::sources::{ResolvedTrack, SourceError, SourceProvider};
use crate::librarian::models::DesiredTrack;
use async_trait::async_trait;

#[derive(Debug, Clone)]
pub struct SpotifySource {
    pub client_id: String,
    pub client_secret: String,
}

impl SpotifySource {
    pub fn new(client_id: impl Into<String>, client_secret: impl Into<String>) -> Self {
        Self {
            client_id: client_id.into(),
            client_secret: client_secret.into(),
        }
    }
}

#[async_trait]
impl SourceProvider for SpotifySource {
    fn name(&self) -> &'static str {
        "spotify"
    }

    fn can_handle(&self, track: &DesiredTrack) -> bool {
        let Some(payload) = &track.raw_payload_json else {
            return false;
        };
        payload.contains("spotify")
    }

    async fn resolve_download_url(&self, _track: &DesiredTrack) -> Result<ResolvedTrack, SourceError> {
        Err(SourceError::NotAvailable(
            "Spotify source resolves metadata only; direct download is intentionally unsupported".to_string(),
        ))
    }

    async fn check_availability(&self, track: &DesiredTrack) -> Result<bool, SourceError> {
        Ok(self.can_handle(track))
    }
}
