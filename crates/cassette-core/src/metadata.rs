use crate::Result;
use anyhow::anyhow;
use governor::{
    clock::DefaultClock,
    state::{direct::NotKeyed, InMemoryState},
    Quota, RateLimiter,
};
use moka::sync::Cache;
use serde::{Deserialize, Serialize};
use std::num::NonZeroU32;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration as StdDuration;

const MB_BASE: &str = "https://musicbrainz.org/ws/2";
const MB_USER_AGENT: &str = "CassettePlayer/0.1 (https://github.com/cassette-music)";
const ITUNES_SEARCH_BASE: &str = "https://itunes.apple.com";
const SPOTIFY_TOKEN_URL: &str = "https://accounts.spotify.com/api/token";
const SPOTIFY_API_BASE: &str = "https://api.spotify.com/v1";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MbRelease {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub year: Option<i32>,
    pub track_count: Option<u32>,
    pub release_group_type: Option<String>,
    pub label: Option<String>,
    pub country: Option<String>,
    pub barcode: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MbTrack {
    pub title: String,
    pub artist: String,
    pub track_number: u32,
    pub disc_number: u32,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MbReleaseWithTracks {
    pub release: MbRelease,
    pub tracks: Vec<MbTrack>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagFix {
    pub path: String,
    pub field: String,
    pub old_value: String,
    pub new_value: String,
    pub applied: bool,
}

pub struct MetadataService {
    client: reqwest::Client,
    rate_limiter: Arc<RateLimiter<NotKeyed, InMemoryState, DefaultClock>>,
    release_search_cache: Cache<String, Vec<MbRelease>>,
    parent_album_cache: Cache<String, Option<MbRelease>>,
    release_tracks_cache: Cache<String, MbReleaseWithTracks>,
    spotify_client_id: Option<String>,
    spotify_client_secret: Option<String>,
    spotify_token: Arc<tokio::sync::Mutex<Option<String>>>,
}

impl MetadataService {
    pub fn new() -> Result<Self> {
        Self::with_spotify(None, None)
    }

    pub fn with_spotify(client_id: Option<String>, client_secret: Option<String>) -> Result<Self> {
        let client = reqwest::Client::builder()
            .user_agent(MB_USER_AGENT)
            .timeout(std::time::Duration::from_secs(15))
            .build()?;

        let quota = Quota::per_second(NonZeroU32::new(1).expect("non-zero quota"));

        Ok(Self {
            client,
            rate_limiter: Arc::new(RateLimiter::direct(quota)),
            release_search_cache: Cache::builder()
                .max_capacity(1_000)
                .time_to_live(StdDuration::from_secs(15 * 60))
                .build(),
            parent_album_cache: Cache::builder()
                .max_capacity(1_000)
                .time_to_live(StdDuration::from_secs(15 * 60))
                .build(),
            release_tracks_cache: Cache::builder()
                .max_capacity(1_000)
                .time_to_live(StdDuration::from_secs(15 * 60))
                .build(),
            spotify_client_id: client_id,
            spotify_client_secret: client_secret,
            spotify_token: Arc::new(tokio::sync::Mutex::new(None)),
        })
    }

    /// Execute a MusicBrainz request with retry on 429/503 (up to 3 attempts with backoff).
    async fn mb_request_with_retry(
        &self,
        request_builder: impl Fn() -> reqwest::RequestBuilder,
    ) -> Result<serde_json::Value> {
        let max_attempts = 3u32;
        for attempt in 1..=max_attempts {
            self.rate_limiter.until_ready().await;
            let resp = request_builder().send().await?;
            let status = resp.status();

            if status.is_success() {
                return Ok(resp.json().await?);
            }

            // Retry on 429 (rate limited) and 503 (service unavailable)
            if (status == reqwest::StatusCode::TOO_MANY_REQUESTS
                || status == reqwest::StatusCode::SERVICE_UNAVAILABLE)
                && attempt < max_attempts
            {
                let backoff = StdDuration::from_millis(1500 * (1u64 << (attempt - 1)));
                tokio::time::sleep(backoff).await;
                continue;
            }

            return Err(anyhow!("MusicBrainz returned HTTP {}", status));
        }
        Err(anyhow!("MusicBrainz request failed after {max_attempts} attempts"))
    }

    /// Search MusicBrainz for a release matching artist + album.
    pub async fn search_release(&self, artist: &str, album: &str) -> Result<Vec<MbRelease>> {
        let cache_key = cache_key_pair(artist, album);
        if let Some(cached) = self.release_search_cache.get(&cache_key) {
            return Ok(cached);
        }

        let query = format!("artist:\"{}\" AND release:\"{}\"", artist, album);
        let body = self
            .mb_request_with_retry(|| {
                self.client
                    .get(format!("{MB_BASE}/release"))
                    .query(&[("query", query.as_str()), ("fmt", "json"), ("limit", "5")])
            })
            .await?;

        let releases: Vec<MbRelease> = body
            .get("releases")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().map(mb_release_from_value).collect())
            .unwrap_or_default();

        self.release_search_cache
            .insert(cache_key, releases.clone());

        Ok(releases)
    }

    /// Search MusicBrainz recordings by artist + track title and return their primary releases.
    /// Used to find the parent album for single tracks (e.g. "Closer" -> "Collage EP").
    /// Prefers Album/EP primary types over Single or Compilation.
    pub async fn find_parent_album(
        &self,
        artist: &str,
        track_title: &str,
    ) -> Result<Option<MbRelease>> {
        let cache_key = cache_key_pair(artist, track_title);
        if let Some(cached) = self.parent_album_cache.get(&cache_key) {
            return Ok(cached);
        }

        let query = format!("recording:\"{}\" AND artist:\"{}\"", track_title, artist);
        let body = self
            .mb_request_with_retry(|| {
                self.client
                    .get(format!("{MB_BASE}/recording"))
                    .query(&[
                        ("query", query.as_str()),
                        ("fmt", "json"),
                        ("limit", "5"),
                        ("inc", "releases+release-groups"),
                    ])
            })
            .await?;
        let recordings = body
            .get("recordings")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let mut best: Option<MbRelease> = None;
        for rec in &recordings {
            let releases = rec
                .get("releases")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            for rel in &releases {
                let rtype = rel
                    .pointer("/release-group/primary-type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown");
                let candidate = mb_release_from_value(rel);
                if best.is_none() {
                    best = Some(candidate.clone());
                }
                if matches!(rtype, "Album" | "EP") {
                    best = Some(candidate);
                    break;
                }
            }
            if best
                .as_ref()
                .map_or(false, |r| matches!(r.release_group_type.as_deref(), Some("Album") | Some("EP")))
            {
                break;
            }
        }

        self.parent_album_cache.insert(cache_key, best.clone());
        Ok(best)
    }

    /// Fetch full release details including track listing.
    pub async fn get_release_tracks(&self, release_id: &str) -> Result<MbReleaseWithTracks> {
        let cache_key = normalize_key(release_id);
        if let Some(cached) = self.release_tracks_cache.get(&cache_key) {
            return Ok(cached);
        }

        let release_url = format!("{MB_BASE}/release/{release_id}");
        let body = self
            .mb_request_with_retry(|| {
                self.client
                    .get(&release_url)
                    .query(&[("inc", "recordings+artist-credits"), ("fmt", "json")])
            })
            .await?;
        let release = mb_release_from_value(&body);

        let mut tracks = Vec::new();
        if let Some(media) = body.get("media").and_then(|v| v.as_array()) {
            for (disc_idx, disc) in media.iter().enumerate() {
                let disc_num = disc
                    .get("position")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(disc_idx as u64 + 1) as u32;

                if let Some(track_list) = disc.get("tracks").and_then(|v| v.as_array()) {
                    for t in track_list {
                        let track_num = t
                            .get("position")
                            .or_else(|| t.get("number"))
                            .and_then(|v| v.as_u64().or_else(|| v.as_str().and_then(|s| s.parse().ok())))
                            .unwrap_or(0) as u32;

                        let title = t
                            .get("title")
                            .and_then(|v| v.as_str())
                            .unwrap_or_default()
                            .to_string();

                        let artist = t
                            .get("artist-credit")
                            .and_then(|v| v.as_array())
                            .and_then(|arr| arr.first())
                            .and_then(|ac| ac.get("name"))
                            .or_else(|| t.pointer("/recording/artist-credit/0/name"))
                            .and_then(|v| v.as_str())
                            .unwrap_or_default()
                            .to_string();

                        let duration_ms = t
                            .get("length")
                            .or_else(|| t.pointer("/recording/length"))
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0);

                        tracks.push(MbTrack {
                            title,
                            artist,
                            track_number: track_num,
                            disc_number: disc_num,
                            duration_ms,
                        });
                    }
                }
            }
        }

        let result = MbReleaseWithTracks { release, tracks };
        self.release_tracks_cache
            .insert(cache_key, result.clone());
        Ok(result)
    }

    /// Resolve a release with full tracklist, trying MusicBrainz → iTunes → Spotify in order.
    pub async fn resolve_release_with_tracks(
        &self,
        artist: &str,
        album: &str,
    ) -> Result<MbReleaseWithTracks> {
        // Try MusicBrainz first
        match self.resolve_via_mb(artist, album).await {
            Ok(result) => return Ok(result),
            Err(e) => tracing::warn!("MusicBrainz failed for '{artist} - {album}': {e}; trying iTunes"),
        }

        // iTunes fallback
        match self.resolve_via_itunes(artist, album).await {
            Ok(result) => return Ok(result),
            Err(e) => tracing::warn!("iTunes fallback failed for '{artist} - {album}': {e}; trying Spotify"),
        }

        // Spotify fallback (only if credentials are configured)
        if self.spotify_client_id.is_some() && self.spotify_client_secret.is_some() {
            match self.resolve_via_spotify(artist, album).await {
                Ok(result) => return Ok(result),
                Err(e) => tracing::warn!("Spotify fallback failed for '{artist} - {album}': {e}"),
            }
        }

        Err(anyhow!("All metadata sources exhausted for '{artist} - {album}'"))
    }

    /// MusicBrainz: search for release then fetch tracklist.
    async fn resolve_via_mb(&self, artist: &str, album: &str) -> Result<MbReleaseWithTracks> {
        let releases = self.search_release(artist, album).await?;
        let best = releases
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("No MusicBrainz results for '{artist} - {album}'"))?;
        self.get_release_tracks(&best.id).await
    }

    /// iTunes Search API: search for album, then fetch tracklist.
    async fn resolve_via_itunes(&self, artist: &str, album: &str) -> Result<MbReleaseWithTracks> {
        // Search for the album collection
        let term = format!("{} {}", artist, album);
        let search_url = format!("{ITUNES_SEARCH_BASE}/search");
        let resp = self
            .client
            .get(&search_url)
            .query(&[
                ("term", term.as_str()),
                ("entity", "album"),
                ("limit", "5"),
            ])
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(anyhow!("iTunes search returned HTTP {}", resp.status()));
        }

        let body: serde_json::Value = resp.json().await?;
        let results = body
            .get("results")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        // Find the best matching collection
        let album_lower = album.to_ascii_lowercase();
        let artist_lower = artist.to_ascii_lowercase();
        let collection = results
            .iter()
            .find(|r| {
                let cname = r
                    .get("collectionName")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_ascii_lowercase();
                let aname = r
                    .get("artistName")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_ascii_lowercase();
                cname.contains(&album_lower) && aname.contains(&artist_lower)
            })
            .or_else(|| results.first())
            .ok_or_else(|| anyhow!("No iTunes results for '{artist} - {album}'"))?;

        let collection_id = collection
            .get("collectionId")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| anyhow!("iTunes result missing collectionId"))?;

        let collection_name = collection
            .get("collectionName")
            .and_then(|v| v.as_str())
            .unwrap_or(album)
            .to_string();

        let artist_name = collection
            .get("artistName")
            .and_then(|v| v.as_str())
            .unwrap_or(artist)
            .to_string();

        let year = collection
            .get("releaseDate")
            .and_then(|v| v.as_str())
            .and_then(|d| d.split('-').next())
            .and_then(|y| y.parse().ok());

        // Fetch track listing
        let lookup_url = format!("{ITUNES_SEARCH_BASE}/lookup");
        let resp = self
            .client
            .get(&lookup_url)
            .query(&[
                ("id", collection_id.to_string().as_str()),
                ("entity", "song"),
            ])
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(anyhow!("iTunes lookup returned HTTP {}", resp.status()));
        }

        let body: serde_json::Value = resp.json().await?;
        let items = body
            .get("results")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let mut tracks: Vec<MbTrack> = items
            .iter()
            .filter(|r| {
                r.get("wrapperType")
                    .and_then(|v| v.as_str())
                    .map_or(false, |t| t == "track")
            })
            .map(|r| {
                let track_num = r
                    .get("trackNumber")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32;
                let disc_num = r
                    .get("discNumber")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(1) as u32;
                let title = r
                    .get("trackName")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                let track_artist = r
                    .get("artistName")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&artist_name)
                    .to_string();
                let duration_ms = r
                    .get("trackTimeMillis")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                MbTrack {
                    title,
                    artist: track_artist,
                    track_number: track_num,
                    disc_number: disc_num,
                    duration_ms,
                }
            })
            .collect();

        tracks.sort_by_key(|t| (t.disc_number, t.track_number));

        if tracks.is_empty() {
            return Err(anyhow!("iTunes returned no tracks for '{artist} - {album}'"));
        }

        let release = MbRelease {
            id: format!("itunes:{}", collection_id),
            title: collection_name,
            artist: artist_name,
            year,
            track_count: Some(tracks.len() as u32),
            release_group_type: Some("Album".into()),
            label: None,
            country: None,
            barcode: None,
        };

        Ok(MbReleaseWithTracks { release, tracks })
    }

    /// Fetch (or refresh) a Spotify client-credentials token.
    async fn get_spotify_token(&self) -> Result<String> {
        let mut guard = self.spotify_token.lock().await;
        if let Some(token) = guard.as_deref() {
            return Ok(token.to_string());
        }

        let client_id = self
            .spotify_client_id
            .as_deref()
            .ok_or_else(|| anyhow!("Spotify client_id not configured"))?;
        let client_secret = self
            .spotify_client_secret
            .as_deref()
            .ok_or_else(|| anyhow!("Spotify client_secret not configured"))?;

        let resp = self
            .client
            .post(SPOTIFY_TOKEN_URL)
            .basic_auth(client_id, Some(client_secret))
            .form(&[("grant_type", "client_credentials")])
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(anyhow!("Spotify token request returned HTTP {}", resp.status()));
        }

        let body: serde_json::Value = resp.json().await?;
        let token = body
            .get("access_token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Spotify token response missing access_token"))?
            .to_string();

        *guard = Some(token.clone());
        Ok(token)
    }

    /// Spotify: search for album, then fetch tracklist.
    async fn resolve_via_spotify(&self, artist: &str, album: &str) -> Result<MbReleaseWithTracks> {
        let token = self.get_spotify_token().await?;

        // Search for the album
        let query = format!("album:{} artist:{}", album, artist);
        let resp = self
            .client
            .get(format!("{SPOTIFY_API_BASE}/search"))
            .bearer_auth(&token)
            .query(&[("q", query.as_str()), ("type", "album"), ("limit", "5")])
            .send()
            .await?;

        if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            // Token may have expired; clear it and retry once
            *self.spotify_token.lock().await = None;
            return Err(anyhow!("Spotify token expired"));
        }

        if !resp.status().is_success() {
            return Err(anyhow!("Spotify search returned HTTP {}", resp.status()));
        }

        let body: serde_json::Value = resp.json().await?;
        let items = body
            .pointer("/albums/items")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let album_lower = album.to_ascii_lowercase();
        let artist_lower = artist.to_ascii_lowercase();
        let sp_album = items
            .iter()
            .find(|a| {
                let name = a
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_ascii_lowercase();
                let aname = a
                    .pointer("/artists/0/name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_ascii_lowercase();
                name.contains(&album_lower) && aname.contains(&artist_lower)
            })
            .or_else(|| items.first())
            .ok_or_else(|| anyhow!("No Spotify results for '{artist} - {album}'"))?;

        let album_id = sp_album
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Spotify album missing id"))?;

        let album_name = sp_album
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or(album)
            .to_string();

        let artist_name = sp_album
            .pointer("/artists/0/name")
            .and_then(|v| v.as_str())
            .unwrap_or(artist)
            .to_string();

        let year = sp_album
            .get("release_date")
            .and_then(|v| v.as_str())
            .and_then(|d| d.split('-').next())
            .and_then(|y| y.parse().ok());

        // Fetch tracks for this album
        let tracks_resp = self
            .client
            .get(format!("{SPOTIFY_API_BASE}/albums/{album_id}/tracks"))
            .bearer_auth(&token)
            .query(&[("limit", "50")])
            .send()
            .await?;

        if !tracks_resp.status().is_success() {
            return Err(anyhow!("Spotify tracks returned HTTP {}", tracks_resp.status()));
        }

        let tracks_body: serde_json::Value = tracks_resp.json().await?;
        let track_items = tracks_body
            .get("items")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let mut tracks: Vec<MbTrack> = track_items
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let track_num = t
                    .get("track_number")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(i as u64 + 1) as u32;
                let disc_num = t
                    .get("disc_number")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(1) as u32;
                let title = t
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                let track_artist = t
                    .pointer("/artists/0/name")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&artist_name)
                    .to_string();
                let duration_ms = t
                    .get("duration_ms")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                MbTrack {
                    title,
                    artist: track_artist,
                    track_number: track_num,
                    disc_number: disc_num,
                    duration_ms,
                }
            })
            .collect();

        tracks.sort_by_key(|t| (t.disc_number, t.track_number));

        if tracks.is_empty() {
            return Err(anyhow!("Spotify returned no tracks for '{artist} - {album}'"));
        }

        let release = MbRelease {
            id: format!("spotify:{}", album_id),
            title: album_name,
            artist: artist_name,
            year,
            track_count: Some(tracks.len() as u32),
            release_group_type: Some("Album".into()),
            label: None,
            country: None,
            barcode: None,
        };

        Ok(MbReleaseWithTracks { release, tracks })
    }

    /// Match local album tracks against MusicBrainz and return proposed fixes.
    pub async fn propose_tag_fixes(
        &self,
        artist: &str,
        album: &str,
        local_tracks: &[crate::models::Track],
    ) -> Result<Vec<TagFix>> {
        let releases = self.search_release(artist, album).await?;
        let Some(best) = releases.first() else {
            return Ok(Vec::new());
        };

        let mb = self.get_release_tracks(&best.id).await?;
        let mut fixes = Vec::new();

        for local in local_tracks {
            let mb_track = mb
                .tracks
                .iter()
                .find(|t| {
                    t.track_number == local.track_number.unwrap_or(0) as u32
                        && t.disc_number == local.disc_number.unwrap_or(1) as u32
                })
                .or_else(|| {
                    let idx = local.track_number.unwrap_or(1).max(1) as usize - 1;
                    mb.tracks.get(idx)
                });

            let Some(mb_t) = mb_track else { continue };

            if !mb_t.title.is_empty() && mb_t.title != local.title {
                fixes.push(TagFix {
                    path: local.path.clone(),
                    field: "title".into(),
                    old_value: local.title.clone(),
                    new_value: mb_t.title.clone(),
                    applied: false,
                });
            }

            if !mb_t.artist.is_empty() && mb_t.artist != local.artist {
                fixes.push(TagFix {
                    path: local.path.clone(),
                    field: "artist".into(),
                    old_value: local.artist.clone(),
                    new_value: mb_t.artist.clone(),
                    applied: false,
                });
            }

            if !mb.release.title.is_empty() && mb.release.title != local.album {
                fixes.push(TagFix {
                    path: local.path.clone(),
                    field: "album".into(),
                    old_value: local.album.clone(),
                    new_value: mb.release.title.clone(),
                    applied: false,
                });
            }

            if let Some(year) = mb.release.year {
                if local.year != Some(year) {
                    fixes.push(TagFix {
                        path: local.path.clone(),
                        field: "year".into(),
                        old_value: local.year.map(|y| y.to_string()).unwrap_or_default(),
                        new_value: year.to_string(),
                        applied: false,
                    });
                }
            }

            if local.track_number != Some(mb_t.track_number as i32) {
                fixes.push(TagFix {
                    path: local.path.clone(),
                    field: "track_number".into(),
                    old_value: local.track_number.map(|n| n.to_string()).unwrap_or_default(),
                    new_value: mb_t.track_number.to_string(),
                    applied: false,
                });
            }
        }

        Ok(fixes)
    }
}

/// Apply a tag fix to the actual file using lofty.
pub fn apply_tag_fix(fix: &TagFix) -> Result<()> {
    use lofty::prelude::*;
    use lofty::probe::Probe;
    use lofty::tag::ItemKey;

    let path = Path::new(&fix.path);
    let mut tagged = Probe::open(path)?.read()?;

    let has_primary = tagged.primary_tag().is_some();
    let tag = if has_primary {
        tagged.primary_tag_mut().unwrap()
    } else {
        tagged
            .first_tag_mut()
            .ok_or_else(|| anyhow!("No tag found in {}", fix.path))?
    };

    match fix.field.as_str() {
        "title" => {
            tag.set_title(fix.new_value.clone());
        }
        "artist" => {
            tag.set_artist(fix.new_value.clone());
        }
        "album" => {
            tag.set_album(fix.new_value.clone());
        }
        "year" => {
            if let Ok(y) = fix.new_value.parse::<u32>() {
                tag.set_year(y);
            }
        }
        "track_number" => {
            if let Ok(n) = fix.new_value.parse::<u32>() {
                tag.set_track(n);
            }
        }
        "album_artist" => {
            tag.insert(lofty::tag::TagItem::new(
                ItemKey::AlbumArtist,
                lofty::tag::ItemValue::Text(fix.new_value.clone()),
            ));
        }
        _ => {}
    }

    tag.save_to_path(path, lofty::config::WriteOptions::default())?;
    Ok(())
}

fn normalize_key(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn cache_key_pair(left: &str, right: &str) -> String {
    format!("{}::{}", normalize_key(left), normalize_key(right))
}

fn mb_release_from_value(v: &serde_json::Value) -> MbRelease {
    MbRelease {
        id: v
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        title: v
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        artist: v
            .get("artist-credit")
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .and_then(|ac| ac.get("name"))
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        year: v
            .get("date")
            .and_then(|v| v.as_str())
            .and_then(|d| d.split('-').next())
            .and_then(|y| y.parse().ok()),
        track_count: v.get("track-count").and_then(|v| v.as_u64()).map(|n| n as u32),
        release_group_type: v
            .pointer("/release-group/primary-type")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        label: v
            .get("label-info")
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .and_then(|li| li.pointer("/label/name"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        country: v.get("country").and_then(|v| v.as_str()).map(|s| s.to_string()),
        barcode: v.get("barcode").and_then(|v| v.as_str()).map(|s| s.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mb_release_from_value_maps_core_fields() {
        let json = serde_json::json!({
            "id": "release-123",
            "title": "Turn on the Bright Lights",
            "artist-credit": [{ "name": "Interpol" }],
            "date": "2002-08-20",
            "track-count": 11,
            "release-group": { "primary-type": "Album" },
            "label-info": [{ "label": { "name": "Matador" } }],
            "country": "US",
            "barcode": "74486105222"
        });

        let release = mb_release_from_value(&json);
        assert_eq!(release.id, "release-123");
        assert_eq!(release.title, "Turn on the Bright Lights");
        assert_eq!(release.artist, "Interpol");
        assert_eq!(release.year, Some(2002));
        assert_eq!(release.track_count, Some(11));
        assert_eq!(release.release_group_type.as_deref(), Some("Album"));
        assert_eq!(release.label.as_deref(), Some("Matador"));
        assert_eq!(release.country.as_deref(), Some("US"));
        assert_eq!(release.barcode.as_deref(), Some("74486105222"));
    }

    #[test]
    fn apply_tag_fix_ignores_unknown_field() {
        let fix = TagFix {
            path: "C:\\no-such-file.flac".to_string(),
            field: "unsupported_field".to_string(),
            old_value: String::new(),
            new_value: "value".to_string(),
            applied: false,
        };

        let result = apply_tag_fix(&fix);
        assert!(result.is_err());
    }
}
