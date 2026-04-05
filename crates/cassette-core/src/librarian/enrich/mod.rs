pub mod discogs;
pub mod lastfm;

use crate::librarian::error::Result;
use crate::librarian::models::Track;

#[derive(Debug, Clone, Default)]
pub struct EnrichmentContext {
    pub artist_name: Option<String>,
    pub album_title: Option<String>,
}

#[async_trait::async_trait]
pub trait MetadataEnricher: Send + Sync {
    async fn enrich(&self, track: &mut Track, context: Option<&EnrichmentContext>) -> Result<()>;
}
