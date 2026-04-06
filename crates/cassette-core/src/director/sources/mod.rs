pub mod bandcamp;
pub mod http;
pub mod local_cache;
pub mod provider_bridge;
pub mod spotify;
pub mod youtube;

use crate::librarian::models::DesiredTrack;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedTrack {
    pub download_url: String,
    pub suggested_filename: String,
    pub expected_codec: Option<String>,
    pub expected_bitrate: Option<u32>,
    pub expected_duration_ms: Option<u64>,
    pub metadata: serde_json::Value,
}

#[derive(thiserror::Error, Debug)]
pub enum SourceError {
    #[error("Track not available: {0}")]
    NotAvailable(String),

    #[error("API error: {0}")]
    ApiError(String),

    #[error("Invalid URL: {0}")]
    InvalidUrl(String),

    #[error("Rate limited: retry after {retry_after_secs}s")]
    RateLimited { retry_after_secs: u64 },

    #[error("Authentication required: {0}")]
    AuthRequired(String),

    #[error("Timeout resolving track")]
    Timeout,
}

#[async_trait]
pub trait SourceProvider: Send + Sync {
    fn name(&self) -> &'static str;
    fn can_handle(&self, track: &DesiredTrack) -> bool;
    async fn resolve_download_url(
        &self,
        track: &DesiredTrack,
    ) -> Result<ResolvedTrack, SourceError>;
    async fn check_availability(&self, track: &DesiredTrack) -> Result<bool, SourceError>;
}

pub use bandcamp::BandcampSource;
pub use http::HttpSource;
pub use local_cache::LocalCacheSource;
pub use provider_bridge::ProviderBridge;
pub use spotify::SpotifySource;
pub use youtube::YoutubeSource;
