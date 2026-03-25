use crate::state::AppState;
use cassette_core::models::{NowPlayingContext, PlaybackState};
use tauri::State;

#[tauri::command]
pub fn player_load(state: State<'_, AppState>, path: String) {
    state.player.load(path.clone());
    let db = state.db.lock().unwrap();
    if let Ok(Some(t)) = db.get_track_by_path(&path) {
        let mut ps = state.playback_state.lock().unwrap();
        ps.current_track = Some(t);
    }
}

#[tauri::command]
pub fn player_play(state: State<'_, AppState>) {
    state.player.play();
}

#[tauri::command]
pub fn player_pause(state: State<'_, AppState>) {
    state.player.pause();
}

#[tauri::command]
pub fn player_stop(state: State<'_, AppState>) {
    state.player.stop();
    let mut ps = state.playback_state.lock().unwrap();
    ps.current_track = None;
    ps.queue_position = 0;
}

#[tauri::command]
pub fn player_toggle(state: State<'_, AppState>) {
    state.player.toggle();
}

#[tauri::command]
pub fn player_next(state: State<'_, AppState>) {
    let queue = state.db.lock().unwrap().get_queue().unwrap_or_default();
    let next_pos = {
        let ps = state.playback_state.lock().unwrap();
        ps.queue_position + 1
    };
    if let Some(item) = queue.get(next_pos) {
        if let Some(ref track) = item.track {
            state.player.load(track.path.clone());
            let mut ps = state.playback_state.lock().unwrap();
            ps.current_track = Some(track.clone());
            ps.queue_position = next_pos;
            let _ = state.db.lock().unwrap().increment_play_count(track.id);
        }
    }
}

#[tauri::command]
pub fn player_prev(state: State<'_, AppState>) {
    let queue = state.db.lock().unwrap().get_queue().unwrap_or_default();
    let prev_pos = {
        let ps = state.playback_state.lock().unwrap();
        if ps.queue_position == 0 { return; }
        ps.queue_position - 1
    };
    if let Some(item) = queue.get(prev_pos) {
        if let Some(ref track) = item.track {
            state.player.load(track.path.clone());
            let mut ps = state.playback_state.lock().unwrap();
            ps.current_track = Some(track.clone());
            ps.queue_position = prev_pos;
        }
    }
}

#[tauri::command]
pub fn player_set_volume(state: State<'_, AppState>, volume: f32) {
    state.player.set_volume(volume);
    let mut ps = state.playback_state.lock().unwrap();
    ps.volume = volume;
}

#[tauri::command]
pub fn player_seek(state: State<'_, AppState>, secs: f64) {
    state.player.seek(secs);
}

#[tauri::command]
pub fn get_playback_state(state: State<'_, AppState>) -> PlaybackState {
    let mut ps = state.playback_state.lock().unwrap().clone();
    ps.position_secs = state.player.position_secs();
    ps.duration_secs = state.player.duration_secs();
    ps.is_playing = state.player.is_playing();
    ps.volume = state.player.volume();
    ps
}

#[tauri::command]
pub async fn get_now_playing_context(
    _state: State<'_, AppState>,
    artist: String,
    title: String,
    album: Option<String>,
) -> Result<NowPlayingContext, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| e.to_string())?;

    let mut ctx = NowPlayingContext {
        artist_name: artist.clone(),
        album_title: album.clone(),
        ..NowPlayingContext::default()
    };

    // Fetch Last.fm artist info (no API key needed for basic JSON)
    let lastfm_artist = fetch_lastfm_artist(&client, &artist).await;
    if let Some(info) = lastfm_artist {
        ctx.artist_summary = info.summary;
        ctx.artist_tags = info.tags;
        ctx.listeners = info.listeners;
    }

    // Fetch Last.fm album info
    if let Some(ref alb) = album {
        let lastfm_album = fetch_lastfm_album(&client, &artist, alb).await;
        if let Some(info) = lastfm_album {
            ctx.album_summary = info.summary;
            ctx.album_art_url = info.image_url;
        }
    }

    // Fetch lyrics from LRCLIB (free, no key)
    let lyrics_result = fetch_lrclib_lyrics(&client, &artist, &title, album.as_deref()).await;
    if let Some(lr) = lyrics_result {
        ctx.lyrics = lr.plain;
        ctx.synced_lyrics = lr.synced;
        ctx.lyrics_source = Some("LRCLIB".into());
    }

    Ok(ctx)
}

// ── Last.fm ──────────────────────────────────────────────────────────────────

const LASTFM_API_KEY: &str = "b25b959554ed76058ac220b7b2e0a026"; // public read-only key

struct LastfmArtistInfo {
    summary: Option<String>,
    tags: Vec<String>,
    listeners: Option<u64>,
}

async fn fetch_lastfm_artist(client: &reqwest::Client, artist: &str) -> Option<LastfmArtistInfo> {
    let resp = client
        .get("https://ws.audioscrobbler.com/2.0/")
        .query(&[
            ("method", "artist.getinfo"),
            ("artist", artist),
            ("api_key", LASTFM_API_KEY),
            ("format", "json"),
        ])
        .send()
        .await
        .ok()?;

    let json: serde_json::Value = resp.json().await.ok()?;
    parse_lastfm_artist_info(&json)
}

struct LastfmAlbumInfo {
    summary: Option<String>,
    image_url: Option<String>,
}

async fn fetch_lastfm_album(client: &reqwest::Client, artist: &str, album: &str) -> Option<LastfmAlbumInfo> {
    let resp = client
        .get("https://ws.audioscrobbler.com/2.0/")
        .query(&[
            ("method", "album.getinfo"),
            ("artist", artist),
            ("album", album),
            ("api_key", LASTFM_API_KEY),
            ("format", "json"),
        ])
        .send()
        .await
        .ok()?;

    let json: serde_json::Value = resp.json().await.ok()?;
    parse_lastfm_album_info(&json)
}

fn strip_lastfm_html_suffix(text: &str) -> String {
    if let Some(idx) = text.find("<a href=") {
        text[..idx].trim().to_string()
    } else {
        text.trim().to_string()
    }
}

fn parse_lastfm_artist_info(json: &serde_json::Value) -> Option<LastfmArtistInfo> {
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

fn parse_lastfm_album_info(json: &serde_json::Value) -> Option<LastfmAlbumInfo> {
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

// ── LRCLIB ───────────────────────────────────────────────────────────────────

struct LrclibResult {
    plain: Option<String>,
    synced: Option<String>,
}

async fn fetch_lrclib_lyrics(
    client: &reqwest::Client,
    artist: &str,
    title: &str,
    album: Option<&str>,
) -> Option<LrclibResult> {
    let mut query = vec![
        ("artist_name", artist),
        ("track_name", title),
    ];
    if let Some(alb) = album {
        query.push(("album_name", alb));
    }

    let resp = client
        .get("https://lrclib.net/api/get")
        .query(&query)
        .header("User-Agent", "Cassette Music Player v0.1")
        .send()
        .await
        .ok()?;

    if !resp.status().is_success() {
        return None;
    }

    let json: serde_json::Value = resp.json().await.ok()?;

    let plain = json.get("plainLyrics")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty());

    let synced = json.get("syncedLyrics")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty());

    if plain.is_none() && synced.is_none() {
        return None;
    }

    Some(LrclibResult { plain, synced })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_lastfm_artist_info_strips_html_and_reads_tags() {
        let json = serde_json::json!({
            "artist": {
                "bio": { "summary": "Great band text <a href=\"https://example\">Read more</a>" },
                "stats": { "listeners": "12345" },
                "tags": { "tag": [{ "name": "indie" }, { "name": "post-punk" }] }
            }
        });

        let parsed = parse_lastfm_artist_info(&json).expect("artist info should parse");
        assert_eq!(parsed.summary.as_deref(), Some("Great band text"));
        assert_eq!(parsed.listeners, Some(12345));
        assert_eq!(parsed.tags, vec!["indie".to_string(), "post-punk".to_string()]);
    }

    #[test]
    fn parse_lastfm_album_info_prefers_largest_image() {
        let json = serde_json::json!({
            "album": {
                "wiki": { "summary": "Album summary <a href=\"https://example\">read</a>" },
                "image": [
                    { "#text": "small.jpg", "size": "small" },
                    { "#text": "", "size": "large" },
                    { "#text": "mega.jpg", "size": "mega" }
                ]
            }
        });

        let parsed = parse_lastfm_album_info(&json).expect("album info should parse");
        assert_eq!(parsed.summary.as_deref(), Some("Album summary"));
        assert_eq!(parsed.image_url.as_deref(), Some("mega.jpg"));
    }
}
