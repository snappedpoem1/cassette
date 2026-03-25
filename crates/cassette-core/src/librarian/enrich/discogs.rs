use crate::librarian::error::Result;
use crate::librarian::models::Track;

#[derive(Debug, Clone, Default)]
pub struct DiscogsClient {
    pub token: Option<String>,
}

impl DiscogsClient {
    pub fn new(token: Option<String>) -> Self {
        Self { token }
    }
}

#[async_trait::async_trait]
impl super::MetadataEnricher for DiscogsClient {
    async fn enrich(&self, _track: &mut Track) -> Result<()> {
        Ok(())
    }
}
