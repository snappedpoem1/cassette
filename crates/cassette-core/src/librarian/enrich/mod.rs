pub mod discogs;
pub mod lastfm;

use crate::librarian::error::Result;
use crate::librarian::models::Track;

#[async_trait::async_trait]
pub trait MetadataEnricher: Send + Sync {
    async fn enrich(&self, track: &mut Track) -> Result<()>;
}
