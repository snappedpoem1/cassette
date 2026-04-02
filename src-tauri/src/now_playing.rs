use cassette_core::models::NowPlayingContext;

pub const LASTFM_API_KEY: &str = "b25b959554ed76058ac220b7b2e0a026";

pub struct LastfmArtistInfo {
    pub summary: Option<String>,
    pub tags: Vec<String>,
    pub listeners: Option<u64>,
}

pub struct LastfmAlbumInfo {
    pub summary: Option<String>,
    pub image_url: Option<String>,
}

pub struct LrclibResult {
    pub plain: Option<String>,
    pub synced: Option<String>,
}

pub fn base_now_playing_context(artist: &str, album: Option<String>) -> NowPlayingContext {
    NowPlayingContext {
        artist_name: artist.to_string(),
        album_title: album,
        ..NowPlayingContext::default()
    }
}

pub fn strip_lastfm_html_suffix(text: &str) -> String {
    if let Some(idx) = text.find("<a href=") {
        text[..idx].trim().to_string()
    } else {
        text.trim().to_string()
    }
}

pub fn parse_lastfm_artist_info(json: &serde_json::Value) -> Option<LastfmArtistInfo> {
    let artist_obj = json.get("artist")?;

    let summary = artist_obj
        .pointer("/bio/summary")
        .and_then(|v| v.as_str())
        .map(strip_lastfm_html_suffix)
        .filter(|s| !s.is_empty());

    let listeners = artist_obj
        .pointer("/stats/listeners")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse().ok());

    let tags = artist_obj
        .pointer("/tags/tag")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|t| t.get("name").and_then(|n| n.as_str()).map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    Some(LastfmArtistInfo {
        summary,
        tags,
        listeners,
    })
}

pub fn parse_lastfm_album_info(json: &serde_json::Value) -> Option<LastfmAlbumInfo> {
    let album_obj = json.get("album")?;

    let summary = album_obj
        .pointer("/wiki/summary")
        .and_then(|v| v.as_str())
        .map(strip_lastfm_html_suffix)
        .filter(|s| !s.is_empty());

    let image_url = album_obj
        .get("image")
        .and_then(|v| v.as_array())
        .and_then(|arr| {
            arr.iter()
                .rev()
                .find_map(|img| {
                    img.get("#text")
                        .and_then(|t| t.as_str())
                        .filter(|s| !s.is_empty())
                        .map(|s| s.to_string())
                })
        });

    Some(LastfmAlbumInfo { summary, image_url })
}
