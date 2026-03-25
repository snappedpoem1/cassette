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
        let Some(payload) = &track.raw_payload_json else {
            return false;
        };
        payload.to_ascii_lowercase().contains("bandcamp")
    }

    async fn resolve_download_url(&self, _track: &DesiredTrack) -> Result<ResolvedTrack, SourceError> {
        Err(SourceError::NotAvailable(
            "Bandcamp resolver is not yet configured in this environment".to_string(),
        ))
    }

    async fn check_availability(&self, track: &DesiredTrack) -> Result<bool, SourceError> {
        Ok(self.can_handle(track))
    }
}
