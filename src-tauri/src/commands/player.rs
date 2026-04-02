use crate::now_playing::{
    base_now_playing_context, parse_lastfm_album_info, parse_lastfm_artist_info,
    LastfmAlbumInfo, LastfmArtistInfo, LrclibResult, LASTFM_API_KEY,
};
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
    let db = state.db.lock().unwrap();
    let queue = db.get_queue().unwrap_or_default();
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
            if let Err(e) = db.increment_play_count(track.id) {
                tracing::warn!("[player_next] failed to increment play count for track {}: {e}", track.id);
            }
        }
    }
}

#[tauri::command]
pub fn player_prev(state: State<'_, AppState>) {
    let queue = state.db.lock().unwrap().get_queue().unwrap_or_default();
    let prev_pos = {
        let ps = state.playback_state.lock().unwrap();
        if ps.queue_position == 0 {
            return;
        }
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
    state: State<'_, AppState>,
    artist: String,
    title: String,
    album: Option<String>,
) -> Result<NowPlayingContext, String> {
    let client = state.http_client.clone();

    let mut ctx = base_now_playing_context(&artist, album.clone());

    let lastfm_key = {
        let db = state.db.lock().unwrap();
        db.get_setting("lastfm_api_key")
            .ok()
            .flatten()
            .filter(|v| !v.trim().is_empty())
    }
    .or_else(|| std::env::var("LASTFM_API_KEY").ok().filter(|v| !v.trim().is_empty()))
    .unwrap_or_else(|| LASTFM_API_KEY.to_string());

    let lastfm_artist = fetch_lastfm_artist(&client, &artist, &lastfm_key).await;
    if let Some(info) = lastfm_artist {
        ctx.artist_summary = info.summary;
        ctx.artist_tags = info.tags;
        ctx.listeners = info.listeners;
    }

    if let Some(ref alb) = album {
        let lastfm_album = fetch_lastfm_album(&client, &artist, alb, &lastfm_key).await;
        if let Some(info) = lastfm_album {
            ctx.album_summary = info.summary;
            ctx.album_art_url = info.image_url;
        }
    }

    let lyrics_result = fetch_lrclib_lyrics(&client, &artist, &title, album.as_deref()).await;
    if let Some(lr) = lyrics_result {
        ctx.lyrics = lr.plain;
        ctx.synced_lyrics = lr.synced;
        ctx.lyrics_source = Some("LRCLIB".into());
    }

    Ok(ctx)
}

async fn fetch_lastfm_artist(
    client: &reqwest::Client,
    artist: &str,
    api_key: &str,
) -> Option<LastfmArtistInfo> {
    let resp = client
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

    let json: serde_json::Value = resp.json().await.ok()?;
    parse_lastfm_artist_info(&json)
}

async fn fetch_lastfm_album(
    client: &reqwest::Client,
    artist: &str,
    album: &str,
    api_key: &str,
) -> Option<LastfmAlbumInfo> {
    let resp = client
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

    let json: serde_json::Value = resp.json().await.ok()?;
    parse_lastfm_album_info(&json)
}

async fn fetch_lrclib_lyrics(
    client: &reqwest::Client,
    artist: &str,
    title: &str,
    album: Option<&str>,
) -> Option<LrclibResult> {
    let mut query = vec![("artist_name", artist), ("track_name", title)];
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

    let plain = json
        .get("plainLyrics")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty());

    let synced = json
        .get("syncedLyrics")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty());

    if plain.is_none() && synced.is_none() {
        return None;
    }

    Some(LrclibResult { plain, synced })
}
