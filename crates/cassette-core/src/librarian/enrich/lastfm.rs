use crate::librarian::error::Result;
use crate::librarian::models::Track;

#[derive(Debug, Clone, Default)]
pub struct LastFmClient {
    pub api_key: Option<String>,
}

impl LastFmClient {
    pub fn new(api_key: Option<String>) -> Self {
        Self { api_key }
    }
}

#[async_trait::async_trait]
impl super::MetadataEnricher for LastFmClient {
    async fn enrich(&self, _track: &mut Track) -> Result<()> {
        Ok(())
    }
}
