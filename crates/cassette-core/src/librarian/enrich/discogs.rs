use crate::librarian::error::Result;
use crate::librarian::models::Track;
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct DiscogsReleaseContext {
    pub release_id: String,
    pub year: Option<i32>,
    pub genres: Vec<String>,
    pub styles: Vec<String>,
    pub labels: Vec<String>,
    pub country: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct DiscogsClient {
    pub token: Option<String>,
}

impl DiscogsClient {
    pub fn new(token: Option<String>) -> Self {
        Self { token }
    }

    pub fn is_configured(&self) -> bool {
        self.token
            .as_deref()
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false)
    }

    pub async fn fetch_release_context(
        &self,
        client: &reqwest::Client,
        artist: &str,
        album: &str,
    ) -> Option<DiscogsReleaseContext> {
        if !self.is_configured() {
            return None;
        }
        let token = self.token.as_deref()?.trim();
        if token.is_empty() {
            return None;
        }

        let response = client
            .get("https://api.discogs.com/database/search")
            .header("Authorization", format!("Discogs token={token}"))
            .header("User-Agent", "Cassette/0.1 (+local)")
            .query(&[
                ("artist", artist),
                ("release_title", album),
                ("type", "release"),
                ("per_page", "5"),
                ("page", "1"),
            ])
            .send()
            .await
            .ok()?;

        if !response.status().is_success() {
            return None;
        }

        let json: Value = response.json().await.ok()?;
        let results = json.get("results")?.as_array()?;
        let best = results.first()?;

        let release_id = best
            .get("id")
            .and_then(|v| v.as_i64())
            .map(|id| id.to_string())?;
        let year = best
            .get("year")
            .and_then(|v| v.as_i64())
            .and_then(|value| i32::try_from(value).ok())
            .filter(|value| *value > 0);

        let genres = string_array(best.get("genre"));
        let styles = string_array(best.get("style"));
        let labels = string_array(best.get("label"));
        let country = best
            .get("country")
            .and_then(|v| v.as_str())
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty());

        Some(DiscogsReleaseContext {
            release_id,
            year,
            genres,
            styles,
            labels,
            country,
        })
    }
}

#[async_trait::async_trait]
impl super::MetadataEnricher for DiscogsClient {
    async fn enrich(
        &self,
        track: &mut Track,
        context: Option<&super::EnrichmentContext>,
    ) -> Result<()> {
        if track.discogs_id.is_some() || !self.is_configured() {
            return Ok(());
        }

        let Some(context) = context else {
            return Ok(());
        };
        let Some(artist_name) = context.artist_name.as_deref() else {
            return Ok(());
        };
        let Some(album_title) = context.album_title.as_deref() else {
            return Ok(());
        };

        let client = reqwest::Client::new();
        if let Some(release) = self
            .fetch_release_context(&client, artist_name, album_title)
            .await
        {
            track.discogs_id = Some(release.release_id);
        }

        Ok(())
    }
}

fn string_array(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(|v| v.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str())
                .map(|item| item.trim().to_string())
                .filter(|item| !item.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}
