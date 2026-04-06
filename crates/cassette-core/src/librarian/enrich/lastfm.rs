use crate::librarian::error::Result;
use crate::librarian::models::Track;
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct LastFmArtistContext {
    pub summary: Option<String>,
    pub tags: Vec<String>,
    pub listeners: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct LastFmAlbumContext {
    pub summary: Option<String>,
    pub image_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LastFmRecentTrack {
    pub artist: String,
    pub title: String,
    pub album: Option<String>,
    pub played_at: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct LastFmClient {
    pub api_key: Option<String>,
}

impl LastFmClient {
    pub fn new(api_key: Option<String>) -> Self {
        Self { api_key }
    }

    pub fn is_configured(&self) -> bool {
        self.api_key
            .as_deref()
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false)
    }

    pub async fn fetch_artist_context(
        &self,
        client: &reqwest::Client,
        artist: &str,
    ) -> Option<LastFmArtistContext> {
        let api_key = self.api_key.as_deref()?.trim();
        if api_key.is_empty() {
            return None;
        }

        let response = client
            .get("https://ws.audioscrobbler.com/2.0/")
            .query(&[
                ("method", "artist.getinfo"),
                ("artist", artist),
                ("api_key", api_key),
                ("format", "json"),
            ])
            .send()
            .await
            .ok()?;

        if !response.status().is_success() {
            return None;
        }

        let json: Value = response.json().await.ok()?;
        parse_artist_context(&json)
    }

    pub async fn fetch_album_context(
        &self,
        client: &reqwest::Client,
        artist: &str,
        album: &str,
    ) -> Option<LastFmAlbumContext> {
        let api_key = self.api_key.as_deref()?.trim();
        if api_key.is_empty() {
            return None;
        }

        let response = client
            .get("https://ws.audioscrobbler.com/2.0/")
            .query(&[
                ("method", "album.getinfo"),
                ("artist", artist),
                ("album", album),
                ("api_key", api_key),
                ("format", "json"),
            ])
            .send()
            .await
            .ok()?;

        if !response.status().is_success() {
            return None;
        }

        let json: Value = response.json().await.ok()?;
        parse_album_context(&json)
    }

    pub async fn fetch_track_duration_ms(
        &self,
        client: &reqwest::Client,
        artist: &str,
        title: &str,
    ) -> Option<i64> {
        let api_key = self.api_key.as_deref()?.trim();
        if api_key.is_empty() {
            return None;
        }

        let response = client
            .get("https://ws.audioscrobbler.com/2.0/")
            .query(&[
                ("method", "track.getinfo"),
                ("artist", artist),
                ("track", title),
                ("api_key", api_key),
                ("format", "json"),
            ])
            .send()
            .await
            .ok()?;

        if !response.status().is_success() {
            return None;
        }

        let json: Value = response.json().await.ok()?;
        let duration_value = json.get("track").and_then(|track| track.get("duration"))?;

        let duration_ms = if let Some(value) = duration_value.as_i64() {
            value
        } else {
            duration_value.as_str()?.parse::<i64>().ok()?
        };

        (duration_ms > 0).then_some(duration_ms)
    }

    pub async fn fetch_recent_tracks(
        &self,
        client: &reqwest::Client,
        username: &str,
        limit: u32,
    ) -> Option<Vec<LastFmRecentTrack>> {
        let api_key = self.api_key.as_deref()?.trim();
        if api_key.is_empty() {
            return None;
        }

        let limit_value = limit.clamp(1, 200).to_string();
        let response = client
            .get("https://ws.audioscrobbler.com/2.0/")
            .query(&[
                ("method", "user.getrecenttracks"),
                ("user", username),
                ("api_key", api_key),
                ("format", "json"),
                ("limit", limit_value.as_str()),
                ("extended", "0"),
            ])
            .send()
            .await
            .ok()?;

        if !response.status().is_success() {
            return None;
        }

        let json: Value = response.json().await.ok()?;
        let tracks = json
            .get("recenttracks")
            .and_then(|recent| recent.get("track"))
            .and_then(|tracks| tracks.as_array())?;

        let parsed = tracks
            .iter()
            .filter_map(|entry| {
                let artist = entry
                    .get("artist")
                    .and_then(|artist| artist.get("#text"))
                    .and_then(|value| value.as_str())
                    .map(str::trim)
                    .filter(|value| !value.is_empty())?
                    .to_string();

                let title = entry
                    .get("name")
                    .and_then(|value| value.as_str())
                    .map(str::trim)
                    .filter(|value| !value.is_empty())?
                    .to_string();

                let album = entry
                    .get("album")
                    .and_then(|album| album.get("#text"))
                    .and_then(|value| value.as_str())
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(|value| value.to_string());

                let played_at = entry
                    .get("date")
                    .and_then(|date| date.get("uts"))
                    .and_then(|value| value.as_str())
                    .and_then(|value| value.parse::<i64>().ok())
                    .and_then(|value| chrono::DateTime::<chrono::Utc>::from_timestamp(value, 0))
                    .map(|value| value.format("%Y-%m-%d %H:%M:%S").to_string());

                Some(LastFmRecentTrack {
                    artist,
                    title,
                    album,
                    played_at,
                })
            })
            .collect::<Vec<_>>();

        Some(parsed)
    }
}

#[async_trait::async_trait]
impl super::MetadataEnricher for LastFmClient {
    async fn enrich(
        &self,
        track: &mut Track,
        context: Option<&super::EnrichmentContext>,
    ) -> Result<()> {
        if track.duration_ms.is_some() || !self.is_configured() {
            return Ok(());
        }

        let Some(context) = context else {
            return Ok(());
        };
        let Some(artist_name) = context.artist_name.as_deref() else {
            return Ok(());
        };
        if track.title.trim().is_empty() {
            return Ok(());
        }

        let client = reqwest::Client::new();
        if let Some(duration_ms) = self
            .fetch_track_duration_ms(&client, artist_name, &track.title)
            .await
        {
            track.duration_ms = Some(duration_ms);
        }

        Ok(())
    }
}

fn parse_artist_context(json: &Value) -> Option<LastFmArtistContext> {
    let artist = json.get("artist")?;
    let summary = artist
        .get("bio")
        .and_then(|bio| bio.get("summary"))
        .and_then(|value| value.as_str())
        .map(strip_lastfm_html_suffix)
        .filter(|value| !value.is_empty());

    let tags = artist
        .get("tags")
        .and_then(|tags| tags.get("tag"))
        .and_then(|tags| tags.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("name").and_then(|value| value.as_str()))
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let listeners = artist
        .get("stats")
        .and_then(|stats| stats.get("listeners"))
        .and_then(|value| value.as_str())
        .and_then(|value| value.parse::<u64>().ok());

    Some(LastFmArtistContext {
        summary,
        tags,
        listeners,
    })
}

fn parse_album_context(json: &Value) -> Option<LastFmAlbumContext> {
    let album = json.get("album")?;
    let summary = album
        .get("wiki")
        .and_then(|wiki| wiki.get("summary"))
        .and_then(|value| value.as_str())
        .map(strip_lastfm_html_suffix)
        .filter(|value| !value.is_empty());

    let image_url = album
        .get("image")
        .and_then(|images| images.as_array())
        .and_then(|items| {
            items
                .iter()
                .rev()
                .find_map(|item| item.get("#text").and_then(|value| value.as_str()))
        })
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    Some(LastFmAlbumContext { summary, image_url })
}

fn strip_lastfm_html_suffix(value: &str) -> String {
    value
        .split("<a href=")
        .next()
        .unwrap_or(value)
        .trim()
        .to_string()
}
