use crate::models::{
    DownloadAlbumResult, DownloadArtistDiscography, DownloadArtistResult,
    DownloadMetadataSearchResult,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;
use tokio::sync::Mutex;

/// Canonical list of audio file extensions recognized across the entire codebase.
/// All modules should use `is_audio_extension()` or `is_audio_path()` instead of
/// maintaining their own lists.
pub const AUDIO_EXTENSIONS: &[&str] = &[
    "flac", "wav", "alac", "dsf", "dff", "aiff", "ape", "wv", "m4a", "mp3", "aac", "ogg", "opus",
];

/// Check whether a file extension (without the dot) is a recognized audio format.
pub fn is_audio_extension(ext: &str) -> bool {
    AUDIO_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str())
}

/// Check whether a path points to a recognized audio file.
pub fn is_audio_path(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(is_audio_extension)
        .unwrap_or(false)
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RemoteProviderConfig {
    pub qobuz_email: Option<String>,
    pub qobuz_password: Option<String>,
    pub qobuz_password_hash: Option<String>,
    pub qobuz_app_id: Option<String>,
    pub qobuz_app_secret: Option<String>,
    pub qobuz_user_auth_token: Option<String>,
    pub qobuz_secrets: Option<String>,
    pub deezer_arl: Option<String>,
    pub spotify_client_id: Option<String>,
    pub spotify_client_secret: Option<String>,
    pub spotify_access_token: Option<String>,
}

impl RemoteProviderConfig {
    pub fn from_env() -> Self {
        Self {
            qobuz_email: std::env::var("QOBUZ_EMAIL").ok(),
            qobuz_password: std::env::var("QOBUZ_PASSWORD").ok(),
            qobuz_password_hash: std::env::var("QOBUZ_PASSWORD_HASH").ok(),
            qobuz_app_id: std::env::var("QOBUZ_APP_ID").ok(),
            qobuz_app_secret: std::env::var("QOBUZ_APP_SECRET").ok(),
            qobuz_user_auth_token: std::env::var("QOBUZ_USER_AUTH_TOKEN").ok(),
            qobuz_secrets: std::env::var("QOBUZ_SECRETS").ok(),
            deezer_arl: std::env::var("DEEZER_ARL").ok(),
            spotify_client_id: std::env::var("SPOTIFY_CLIENT_ID").ok(),
            spotify_client_secret: std::env::var("SPOTIFY_CLIENT_SECRET").ok(),
            spotify_access_token: std::env::var("SPOTIFY_ACCESS_TOKEN").ok(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SlskdConnectionConfig {
    pub url: String,
    pub username: String,
    pub password: String,
    pub api_key: Option<String>,
}

pub async fn search_metadata(
    provider_config: &RemoteProviderConfig,
    query: &str,
) -> Result<DownloadMetadataSearchResult, String> {
    if query.trim().is_empty() {
        return Ok(DownloadMetadataSearchResult::default());
    }

    let mut artists = Vec::new();
    let mut albums = Vec::new();
    let mut errors = Vec::new();

    match qobuz_search(provider_config, query).await {
        Ok(result) => {
            artists.extend(result.artists);
            albums.extend(result.albums);
        }
        Err(error) => errors.push(format!("Qobuz: {error}")),
    }
    match deezer_search(provider_config, query).await {
        Ok(result) => {
            artists.extend(result.artists);
            albums.extend(result.albums);
        }
        Err(error) => errors.push(format!("Deezer: {error}")),
    }
    match spotify_search(provider_config, query).await {
        Ok(result) => {
            artists.extend(result.artists);
            albums.extend(result.albums);
        }
        Err(error) => errors.push(format!("Spotify: {error}")),
    }

    if artists.is_empty() && albums.is_empty() && !errors.is_empty() {
        return Err(errors.join(" | "));
    }

    dedupe_artists(&mut artists);
    dedupe_albums(&mut albums);
    Ok(DownloadMetadataSearchResult { artists, albums })
}

pub async fn get_artist_discography(
    provider_config: &RemoteProviderConfig,
    artist: &str,
    artist_mbid: Option<String>,
) -> Result<DownloadArtistDiscography, String> {
    if let Ok(result) = qobuz_discography(provider_config, artist, artist_mbid.clone()).await {
        return Ok(result);
    }
    if let Ok(result) = deezer_discography(provider_config, artist, artist_mbid.clone()).await {
        return Ok(result);
    }
    if let Ok(result) = spotify_discography(provider_config, artist, artist_mbid.clone()).await {
        return Ok(result);
    }

    Ok(DownloadArtistDiscography {
        artist: DownloadArtistResult {
            id: artist.to_string(),
            name: artist.to_string(),
            artist_mbid,
            ..DownloadArtistResult::default()
        },
        albums: Vec::new(),
    })
}

pub async fn fetch_slskd_transfers(config: &SlskdConnectionConfig) -> Result<Vec<Value>, String> {
    let client = reqwest::Client::new();
    let response =
        send_slskd_request(client.get(format!("{}/api/v0/transfers/downloads", config.url)), config)
            .await?;

    if !response.status().is_success() {
        return Err(format!("slskd returned HTTP {}", response.status()));
    }

    response.json::<Vec<Value>>().await.map_err(|error| error.to_string())
}

/// Fetch download transfers for a specific user from slskd.
pub async fn fetch_slskd_user_transfers(
    config: &SlskdConnectionConfig,
    username: &str,
) -> Result<Value, String> {
    let client = reqwest::Client::new();
    let response = send_slskd_request(
        client.get(format!(
            "{}/api/v0/transfers/downloads/{}",
            config.url,
            urlencoding::encode(username)
        )),
        config,
    )
    .await?;

    if !response.status().is_success() {
        return Err(format!("slskd user transfers returned HTTP {}", response.status()));
    }

    response.json::<Value>().await.map_err(|error| error.to_string())
}

pub async fn qobuz_search(
    provider_config: &RemoteProviderConfig,
    query: &str,
) -> Result<DownloadMetadataSearchResult, String> {
    let Some(app_id) = present(&provider_config.qobuz_app_id) else {
        return Ok(DownloadMetadataSearchResult::default());
    };
    let Some(user_auth_token) = qobuz_user_auth_token(provider_config).await? else {
        return Ok(DownloadMetadataSearchResult::default());
    };

    let client = reqwest::Client::new();
    let response = client
        .get("https://www.qobuz.com/api.json/0.2/catalog/search")
        .query(&[
            ("query", query),
            ("limit", "8"),
            ("app_id", app_id),
            ("user_auth_token", user_auth_token.as_str()),
        ])
        .send()
        .await
        .map_err(|error| error.to_string())?;

    if !response.status().is_success() {
        return Err(format!("HTTP {}", response.status()));
    }

    let body = response.json::<Value>().await.map_err(|error| error.to_string())?;
    Ok(DownloadMetadataSearchResult {
        artists: qobuz_artists_from_search(&body),
        albums: qobuz_albums_from_search(&body),
    })
}

pub async fn qobuz_discography(
    provider_config: &RemoteProviderConfig,
    artist: &str,
    artist_mbid: Option<String>,
) -> Result<DownloadArtistDiscography, String> {
    let search = qobuz_search(provider_config, artist).await?;
    let Some(found_artist) = search.artists.first().cloned() else {
        return Err("No Qobuz artist match found.".to_string());
    };
    let Some(app_id) = present(&provider_config.qobuz_app_id) else {
        return Err("Qobuz app ID missing.".to_string());
    };
    let Some(user_auth_token) = qobuz_user_auth_token(provider_config).await? else {
        return Err("Qobuz auth token unavailable.".to_string());
    };

    let client = reqwest::Client::new();
    let response = client
        .get("https://www.qobuz.com/api.json/0.2/artist/get")
        .query(&[
            ("artist_id", found_artist.id.as_str()),
            ("extra", "albums"),
            ("limit", "50"),
            ("app_id", app_id),
            ("user_auth_token", user_auth_token.as_str()),
        ])
        .send()
        .await
        .map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!("HTTP {}", response.status()));
    }

    let body = response.json::<Value>().await.map_err(|error| error.to_string())?;
    let albums = body
        .get("albums")
        .and_then(|value| value.get("items"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(qobuz_album_from_value)
                .filter(|album| !album.title.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    Ok(DownloadArtistDiscography {
        artist: DownloadArtistResult {
            artist_mbid,
            ..found_artist
        },
        albums,
    })
}

pub async fn deezer_search(
    provider_config: &RemoteProviderConfig,
    query: &str,
) -> Result<DownloadMetadataSearchResult, String> {
    let client = deezer_client(provider_config)?;
    let artists_response = client
        .get("https://api.deezer.com/search/artist")
        .query(&[("q", query)])
        .send()
        .await
        .map_err(|error| error.to_string())?;
    let albums_response = client
        .get("https://api.deezer.com/search/album")
        .query(&[("q", query)])
        .send()
        .await
        .map_err(|error| error.to_string())?;

    let artists_body = artists_response
        .json::<Value>()
        .await
        .map_err(|error| error.to_string())?;
    let albums_body = albums_response
        .json::<Value>()
        .await
        .map_err(|error| error.to_string())?;

    Ok(DownloadMetadataSearchResult {
        artists: artists_body
            .get("data")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .map(deezer_artist_from_value)
                    .filter(|artist| !artist.name.is_empty())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default(),
        albums: albums_body
            .get("data")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .map(deezer_album_from_value)
                    .filter(|album| !album.title.is_empty())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default(),
    })
}

pub async fn deezer_discography(
    provider_config: &RemoteProviderConfig,
    artist: &str,
    artist_mbid: Option<String>,
) -> Result<DownloadArtistDiscography, String> {
    let search = deezer_search(provider_config, artist).await?;
    let Some(found_artist) = search.artists.first().cloned() else {
        return Err("No Deezer artist match found.".to_string());
    };
    let client = deezer_client(provider_config)?;
    let response = client
        .get(format!("https://api.deezer.com/artist/{}/albums", found_artist.id))
        .query(&[("limit", "50")])
        .send()
        .await
        .map_err(|error| error.to_string())?;

    let body = response.json::<Value>().await.map_err(|error| error.to_string())?;
    let albums = body
        .get("data")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(deezer_album_from_value)
                .filter(|album| !album.title.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    Ok(DownloadArtistDiscography {
        artist: DownloadArtistResult {
            artist_mbid,
            ..found_artist
        },
        albums,
    })
}

pub async fn spotify_search(
    provider_config: &RemoteProviderConfig,
    query: &str,
) -> Result<DownloadMetadataSearchResult, String> {
    let Some(token) = spotify_bearer_token(provider_config).await? else {
        return Ok(DownloadMetadataSearchResult::default());
    };
    let client = reqwest::Client::new();
    let response = client
        .get("https://api.spotify.com/v1/search")
        .bearer_auth(token)
        .query(&[("q", query), ("type", "artist,album"), ("limit", "10")])
        .send()
        .await
        .map_err(|error| error.to_string())?;

    if !response.status().is_success() {
        return Err(format!("HTTP {}", response.status()));
    }

    let body = response.json::<Value>().await.map_err(|error| error.to_string())?;
    Ok(DownloadMetadataSearchResult {
        artists: body
            .get("artists")
            .and_then(|value| value.get("items"))
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .map(spotify_artist_from_value)
                    .filter(|artist| !artist.name.is_empty())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default(),
        albums: body
            .get("albums")
            .and_then(|value| value.get("items"))
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .map(spotify_album_from_value)
                    .filter(|album| !album.title.is_empty())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default(),
    })
}

pub async fn spotify_discography(
    provider_config: &RemoteProviderConfig,
    artist: &str,
    artist_mbid: Option<String>,
) -> Result<DownloadArtistDiscography, String> {
    let search = spotify_search(provider_config, artist).await?;
    let Some(found_artist) = search.artists.first().cloned() else {
        return Err("No Spotify artist match found.".to_string());
    };
    let Some(token) = spotify_bearer_token(provider_config).await? else {
        return Err("Spotify token unavailable.".to_string());
    };
    let client = reqwest::Client::new();
    let response = client
        .get(format!(
            "https://api.spotify.com/v1/artists/{}/albums",
            found_artist.id
        ))
        .bearer_auth(token)
        .query(&[("include_groups", "album,single,compilation"), ("limit", "50")])
        .send()
        .await
        .map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!("HTTP {}", response.status()));
    }
    let body = response.json::<Value>().await.map_err(|error| error.to_string())?;
    let albums = body
        .get("items")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(spotify_album_from_value)
                .filter(|album| !album.title.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Ok(DownloadArtistDiscography {
        artist: DownloadArtistResult {
            artist_mbid,
            ..found_artist
        },
        albums,
    })
}

pub async fn qobuz_user_auth_token(
    provider_config: &RemoteProviderConfig,
) -> Result<Option<String>, String> {
    if let Some(token) = present(&provider_config.qobuz_user_auth_token) {
        return Ok(Some(token.to_string()));
    }
    let Some(email) = present(&provider_config.qobuz_email) else {
        return Ok(None);
    };
    let Some(app_id) = present(&provider_config.qobuz_app_id) else {
        return Ok(None);
    };
    let password = present(&provider_config.qobuz_password_hash)
        .or_else(|| present(&provider_config.qobuz_password));
    let Some(password) = password else {
        return Ok(None);
    };

    let client = reqwest::Client::new();
    let response = client
        .post("https://www.qobuz.com/api.json/0.2/user/login")
        .form(&[("email", email), ("password", password), ("app_id", app_id)])
        .send()
        .await
        .map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!("HTTP {}", response.status()));
    }
    let body = response.json::<Value>().await.map_err(|error| error.to_string())?;
    Ok(body
        .get("user_auth_token")
        .and_then(Value::as_str)
        .map(ToString::to_string))
}

pub async fn spotify_bearer_token(
    provider_config: &RemoteProviderConfig,
) -> Result<Option<String>, String> {
    if let Some(token) = present(&provider_config.spotify_access_token) {
        return Ok(Some(token.to_string()));
    }
    let Some(client_id) = present(&provider_config.spotify_client_id) else {
        return Ok(None);
    };
    let Some(client_secret) = present(&provider_config.spotify_client_secret) else {
        return Ok(None);
    };

    let client = reqwest::Client::new();
    let response = client
        .post("https://accounts.spotify.com/api/token")
        .basic_auth(client_id, Some(client_secret))
        .form(&[("grant_type", "client_credentials")])
        .send()
        .await
        .map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!("HTTP {}", response.status()));
    }
    let body = response.json::<Value>().await.map_err(|error| error.to_string())?;
    Ok(body
        .get("access_token")
        .and_then(Value::as_str)
        .map(ToString::to_string))
}

pub fn deezer_client(provider_config: &RemoteProviderConfig) -> Result<reqwest::Client, String> {
    let mut headers = reqwest::header::HeaderMap::new();
    if let Some(arl) = present(&provider_config.deezer_arl) {
        let cookie = format!("arl={arl}");
        let header_value = reqwest::header::HeaderValue::from_str(&cookie)
            .map_err(|error| error.to_string())?;
        headers.insert(reqwest::header::COOKIE, header_value);
    }
    headers.insert(
        reqwest::header::USER_AGENT,
        reqwest::header::HeaderValue::from_static(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/135.0.0.0 Safari/537.36",
        ),
    );
    headers.insert(
        reqwest::header::ACCEPT,
        reqwest::header::HeaderValue::from_static("application/json, text/plain, */*"),
    );
    headers.insert(
        reqwest::header::REFERER,
        reqwest::header::HeaderValue::from_static("https://www.deezer.com/"),
    );
    headers.insert(
        reqwest::header::ORIGIN,
        reqwest::header::HeaderValue::from_static("https://www.deezer.com"),
    );

    reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .map_err(|error| error.to_string())
}

/// Cached slskd JWT authorization header, keyed by base URL.
/// The JWT from slskd typically lasts 7 days, so caching for the process lifetime is safe.
static SLSKD_TOKEN_CACHE: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();

pub async fn send_slskd_request(
    request: reqwest::RequestBuilder,
    config: &SlskdConnectionConfig,
) -> Result<reqwest::Response, String> {
    if let Some(api_key) = present(&config.api_key) {
        return request
            .header("X-API-Key", api_key)
            .send()
            .await
            .map_err(|error| error.to_string());
    }

    let authorization = cached_slskd_authorization(config).await?;
    request
        .header(reqwest::header::AUTHORIZATION, &authorization)
        .send()
        .await
        .map_err(|error| error.to_string())
}

async fn cached_slskd_authorization(config: &SlskdConnectionConfig) -> Result<String, String> {
    let cache = SLSKD_TOKEN_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let map = cache.lock().await;
    if let Some(token) = map.get(&config.url) {
        return Ok(token.clone());
    }
    drop(map);

    let authorization = slskd_session_authorization(config).await?;
    let mut map = cache.lock().await;
    map.insert(config.url.clone(), authorization.clone());
    Ok(authorization)
}

pub async fn slskd_session_authorization(
    config: &SlskdConnectionConfig,
) -> Result<String, String> {
    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/api/v0/session", config.url))
        .json(&serde_json::json!({
            "username": config.username,
            "password": config.password,
        }))
        .send()
        .await
        .map_err(|error| error.to_string())?;

    if !response.status().is_success() {
        return Err(format!("slskd session returned HTTP {}", response.status()));
    }

    let body = response.json::<Value>().await.map_err(|error| error.to_string())?;
    let token_type = body.get("tokenType").and_then(Value::as_str).unwrap_or("Bearer");
    let Some(token) = body.get("token").and_then(Value::as_str) else {
        return Err("slskd session did not return a token.".to_string());
    };

    Ok(format!("{token_type} {token}"))
}

pub fn build_query(artist: &str, title: &str, album: Option<&str>) -> String {
    match album.filter(|value| !value.trim().is_empty()) {
        Some(album) => format!("{artist} {title} {album}"),
        None => format!("{artist} {title}"),
    }
}

pub fn normalize_text(value: &str) -> String {
    value
        .chars()
        .map(|ch| if ch.is_alphanumeric() { ch.to_ascii_lowercase() } else { ' ' })
        .collect::<String>()
}

pub fn normalized_terms(value: &str) -> Vec<String> {
    normalize_text(value)
        .split_whitespace()
        .filter(|term| !term.is_empty())
        .map(ToString::to_string)
        .collect()
}

pub fn count_matching_terms(normalized: &str, terms: &[String]) -> usize {
    terms
        .iter()
        .filter(|term| normalized.contains(term.as_str()))
        .count()
}

pub fn is_non_audio_path(filename: &str) -> bool {
    let extension = std::path::Path::new(filename)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    matches!(
        extension.as_str(),
        "lrc" | "cue" | "jpg" | "jpeg" | "png" | "gif" | "log" | "m3u" | "nfo" | "txt"
    )
}

fn present(value: &Option<String>) -> Option<&str> {
    value.as_deref().map(str::trim).filter(|value| !value.is_empty())
}

fn dedupe_artists(artists: &mut Vec<DownloadArtistResult>) {
    let mut seen = HashMap::<String, ()>::new();
    artists.retain(|artist| {
        let key = format!("{}::{}", artist.source, artist.id);
        seen.insert(key, ()).is_none()
    });
}

fn dedupe_albums(albums: &mut Vec<DownloadAlbumResult>) {
    let mut seen = HashMap::<String, ()>::new();
    albums.retain(|album| {
        let key = format!("{}::{}", album.source, album.id);
        seen.insert(key, ()).is_none()
    });
}

fn qobuz_artists_from_search(body: &Value) -> Vec<DownloadArtistResult> {
    body.get("artists")
        .and_then(|value| value.get("items"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(qobuz_artist_from_value)
                .filter(|artist| !artist.name.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn qobuz_albums_from_search(body: &Value) -> Vec<DownloadAlbumResult> {
    body.get("albums")
        .and_then(|value| value.get("items"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(qobuz_album_from_value)
                .filter(|album| !album.title.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn qobuz_artist_from_value(value: &Value) -> DownloadArtistResult {
    DownloadArtistResult {
        id: string_field(value, "id"),
        name: string_field(value, "name"),
        sort_name: value.get("name").and_then(Value::as_str).map(ToString::to_string),
        disambiguation: value.get("slug").and_then(Value::as_str).map(ToString::to_string),
        image_url: nested_string_field(value, &["image", "large"]),
        source: "qobuz".to_string(),
        artist_mbid: nested_string_field(value, &["artist", "id"]),
        ..DownloadArtistResult::default()
    }
}

fn qobuz_album_from_value(value: &Value) -> DownloadAlbumResult {
    DownloadAlbumResult {
        id: string_field(value, "id"),
        title: string_field(value, "title"),
        artist: value
            .get("artist")
            .and_then(|artist| artist.get("name"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        artist_mbid: nested_string_field(value, &["artist", "id"]),
        year: value.get("year").and_then(Value::as_i64).map(|year| year as i32),
        release_type: value
            .get("product_type")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        track_count: value
            .get("tracks_count")
            .and_then(Value::as_u64)
            .map(|count| count as u32),
        cover_url: nested_string_field(value, &["image", "large"]),
        source: "qobuz".to_string(),
        mbid: value.get("upc").and_then(Value::as_str).map(ToString::to_string),
        ..DownloadAlbumResult::default()
    }
}

fn deezer_artist_from_value(value: &Value) -> DownloadArtistResult {
    DownloadArtistResult {
        id: string_field(value, "id"),
        name: string_field(value, "name"),
        image_url: value
            .get("picture_xl")
            .or_else(|| value.get("picture_big"))
            .and_then(Value::as_str)
            .map(ToString::to_string),
        source: "deezer".to_string(),
        ..DownloadArtistResult::default()
    }
}

fn deezer_album_from_value(value: &Value) -> DownloadAlbumResult {
    DownloadAlbumResult {
        id: string_field(value, "id"),
        title: string_field(value, "title"),
        artist: value
            .get("artist")
            .and_then(|artist| artist.get("name"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        year: value
            .get("release_date")
            .and_then(Value::as_str)
            .and_then(|date| date.split('-').next())
            .and_then(|year| year.parse::<i32>().ok()),
        release_type: value
            .get("record_type")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        cover_url: value
            .get("cover_xl")
            .or_else(|| value.get("cover_big"))
            .and_then(Value::as_str)
            .map(ToString::to_string),
        source: "deezer".to_string(),
        ..DownloadAlbumResult::default()
    }
}

fn spotify_artist_from_value(value: &Value) -> DownloadArtistResult {
    DownloadArtistResult {
        id: string_field(value, "id"),
        name: string_field(value, "name"),
        image_url: value
            .get("images")
            .and_then(Value::as_array)
            .and_then(|images| images.first())
            .and_then(|image| image.get("url"))
            .and_then(Value::as_str)
            .map(ToString::to_string),
        source: "spotify".to_string(),
        ..DownloadArtistResult::default()
    }
}

fn spotify_album_from_value(value: &Value) -> DownloadAlbumResult {
    DownloadAlbumResult {
        id: string_field(value, "id"),
        title: string_field(value, "name"),
        artist: value
            .get("artists")
            .and_then(Value::as_array)
            .and_then(|artists| artists.first())
            .and_then(|artist| artist.get("name"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        year: value
            .get("release_date")
            .and_then(Value::as_str)
            .and_then(|date| date.split('-').next())
            .and_then(|year| year.parse::<i32>().ok()),
        release_type: value.get("album_type").and_then(Value::as_str).map(ToString::to_string),
        track_count: value
            .get("total_tracks")
            .and_then(Value::as_u64)
            .map(|count| count as u32),
        cover_url: value
            .get("images")
            .and_then(Value::as_array)
            .and_then(|images| images.first())
            .and_then(|image| image.get("url"))
            .and_then(Value::as_str)
            .map(ToString::to_string),
        source: "spotify".to_string(),
        ..DownloadAlbumResult::default()
    }
}

fn string_field(value: &Value, key: &str) -> String {
    value
        .get(key)
        .map(|item| {
            item.as_str()
                .map(ToString::to_string)
                .unwrap_or_else(|| item.to_string())
        })
        .unwrap_or_default()
}

fn nested_string_field(value: &Value, path: &[&str]) -> Option<String> {
    let mut current = value;
    for segment in path {
        current = current.get(*segment)?;
    }
    current.as_str().map(ToString::to_string)
}

// ---------------------------------------------------------------------------
// Deezer gateway (private API) — full-track download support
// ---------------------------------------------------------------------------

const DEEZER_GW_URL: &str = "https://www.deezer.com/ajax/gw-light.php";
const DEEZER_MEDIA_URL: &str = "https://media.deezer.com/v1/get_url";

/// Session data returned by `deezer.getUserData`.
#[derive(Debug, Clone)]
pub struct DeezerSession {
    pub api_token: String,
    pub license_token: String,
    pub user_id: u64,
}

/// Track data returned by `song.getData`.
#[derive(Debug, Clone)]
pub struct DeezerTrackData {
    pub track_id: String,
    pub track_token: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_secs: u64,
    pub file_size_flac: Option<u64>,
    pub file_size_320: Option<u64>,
    pub file_size_128: Option<u64>,
}

/// Call `deezer.getUserData` to obtain session tokens.
pub async fn deezer_get_user_data(client: &reqwest::Client) -> Result<DeezerSession, String> {
    let response = client
        .post(DEEZER_GW_URL)
        .query(&[("method", "deezer.getUserData"), ("input", "3"), ("api_version", "1.0"), ("api_token", "")])
        .header(reqwest::header::CONTENT_LENGTH, "0")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(format!("getUserData HTTP {}", response.status()));
    }
    let body: Value = response.json().await.map_err(|e| e.to_string())?;
    let results = body.get("results").ok_or("no results in getUserData response")?;
    let api_token = results
        .get("checkForm")
        .and_then(Value::as_str)
        .ok_or("missing checkForm (api_token)")?
        .to_string();
    let user = results.get("USER").ok_or("missing USER object")?;
    let license_token = user
        .pointer("/OPTIONS/license_token")
        .and_then(Value::as_str)
        .ok_or("missing license_token")?
        .to_string();
    let user_id = user
        .get("USER_ID")
        .and_then(|v| v.as_u64().or_else(|| v.as_str().and_then(|s| s.parse().ok())))
        .ok_or("missing USER_ID")?;
    if user_id == 0 {
        return Err("ARL invalid or expired — USER_ID is 0".to_string());
    }
    Ok(DeezerSession { api_token, license_token, user_id })
}

/// Call `song.getData` to obtain track token and metadata.
pub async fn deezer_get_track_data(
    client: &reqwest::Client,
    api_token: &str,
    track_id: &str,
) -> Result<DeezerTrackData, String> {
    let response = client
        .post(DEEZER_GW_URL)
        .query(&[
            ("method", "song.getData"),
            ("input", "3"),
            ("api_version", "1.0"),
            ("api_token", api_token),
        ])
        .json(&serde_json::json!({ "sng_id": track_id, "SNG_ID": track_id }))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(format!("song.getData HTTP {}", response.status()));
    }
    let body: Value = response.json().await.map_err(|e| e.to_string())?;
    let results = body.get("results").ok_or("no results in song.getData")?;
    let track_token = find_string_field(results, "TRACK_TOKEN");
    let sng_id = find_string_field(results, "SNG_ID").unwrap_or_else(|| track_id.to_string());
    let title = find_string_field(results, "SNG_TITLE").unwrap_or_default();
    let artist = find_string_field(results, "ART_NAME").unwrap_or_default();
    let album = find_string_field(results, "ALB_TITLE").unwrap_or_default();
    let duration_secs = find_u64_field(results, "DURATION").unwrap_or(0);
    let file_size_flac = find_u64_field(results, "FILESIZE_FLAC");
    let file_size_320 = find_u64_field(results, "FILESIZE_MP3_320");
    let file_size_128 = find_u64_field(results, "FILESIZE_MP3_128");

    if let Some(track_token) = track_token {
        return Ok(DeezerTrackData {
            track_id: sng_id,
            track_token,
            title,
            artist,
            album,
            duration_secs,
            file_size_flac,
            file_size_320,
            file_size_128,
        });
    }

    // song.getData can return an empty object for some sessions/cookies.
    // Fall back to the public track API, which includes track_token for acquire.
    let public_response = client
        .get(format!("https://api.deezer.com/track/{track_id}"))
        .send()
        .await
        .map_err(|error| error.to_string())?;
    if !public_response.status().is_success() {
        return Err(format!(
            "missing TRACK_TOKEN in song.getData response: {}; public track API HTTP {}",
            summarize_value(results),
            public_response.status()
        ));
    }
    let public_body = public_response
        .json::<Value>()
        .await
        .map_err(|error| error.to_string())?;
    if let Some(track_token) = public_body.get("track_token").and_then(Value::as_str) {
        let public_track_id = public_body
            .get("id")
            .and_then(|value| value.as_u64().map(|id| id.to_string()).or_else(|| value.as_str().map(ToString::to_string)))
            .unwrap_or_else(|| track_id.to_string());
        let title = public_body
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let artist = public_body
            .get("artist")
            .and_then(|value| value.get("name"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let album = public_body
            .get("album")
            .and_then(|value| value.get("title"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let duration_secs = public_body
            .get("duration")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        return Ok(DeezerTrackData {
            track_id: public_track_id,
            track_token: track_token.to_string(),
            title,
            artist,
            album,
            duration_secs,
            file_size_flac: None,
            file_size_320: None,
            file_size_128: None,
        });
    }

    Err(format!(
        "missing TRACK_TOKEN in song.getData response: {}; public fallback payload: {}",
        summarize_value(results),
        summarize_value(&public_body)
    ))
}

/// Request a media CDN URL from Deezer.
///
/// Tries FLAC first, then MP3_320, then MP3_128.
pub async fn deezer_get_media_url(
    client: &reqwest::Client,
    license_token: &str,
    track_token: &str,
) -> Result<(String, String), String> {
    // Format codes Deezer accepts
    let formats = [
        ("FLAC", "flac"),
        ("MP3_320", "mp3"),
        ("MP3_128", "mp3"),
    ];

    let mut last_error_message = None::<String>;

    for (format_code, extension) in &formats {
        let payload = serde_json::json!({
            "license_token": license_token,
            "media": [{
                "type": "FULL",
                "formats": [{
                    "cipher": "BF_CBC_STRIPE",
                    "format": format_code,
                }]
            }],
            "track_tokens": [track_token],
        });

        let response = client
            .post(DEEZER_MEDIA_URL)
            .json(&payload)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if !response.status().is_success() {
            let status = response.status();
            let error_body = response
                .json::<Value>()
                .await
                .unwrap_or(Value::Null);
            let rights_message = error_body
                .pointer("/errors/0/message")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if !rights_message.is_empty() {
                last_error_message = Some(format!("{rights_message} (HTTP {status})"));
            }
            continue;
        }

        let body: Value = response.json().await.map_err(|e| e.to_string())?;
        if let Some(url) = body
            .pointer("/data/0/media/0/sources/0/url")
            .and_then(Value::as_str)
        {
            if !url.is_empty() {
                return Ok((url.to_string(), extension.to_string()));
            }
        }
    }

    Err(last_error_message.unwrap_or_else(|| "no media URL available for any quality tier".to_string()))
}

fn find_string_field(value: &Value, key: &str) -> Option<String> {
    match value {
        Value::Object(map) => {
            if let Some(found) = map.get(key) {
                if let Some(as_str) = found.as_str() {
                    if !as_str.trim().is_empty() {
                        return Some(as_str.to_string());
                    }
                }
                if let Some(as_u64) = found.as_u64() {
                    return Some(as_u64.to_string());
                }
            }
            map.values().find_map(|nested| find_string_field(nested, key))
        }
        Value::Array(items) => items.iter().find_map(|item| find_string_field(item, key)),
        _ => None,
    }
}

fn find_u64_field(value: &Value, key: &str) -> Option<u64> {
    match value {
        Value::Object(map) => {
            if let Some(found) = map.get(key) {
                if let Some(as_u64) = found.as_u64() {
                    return Some(as_u64);
                }
                if let Some(as_str) = found.as_str() {
                    if let Ok(parsed) = as_str.parse::<u64>() {
                        return Some(parsed);
                    }
                }
            }
            map.values().find_map(|nested| find_u64_field(nested, key))
        }
        Value::Array(items) => items.iter().find_map(|item| find_u64_field(item, key)),
        _ => None,
    }
}

fn summarize_value(value: &Value) -> String {
    let rendered = serde_json::to_string(value).unwrap_or_else(|_| "<unrenderable>".to_string());
    rendered.chars().take(400).collect()
}
