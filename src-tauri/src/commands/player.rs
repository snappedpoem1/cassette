use crate::now_playing::{base_now_playing_context, LrclibResult};
use crate::state::AppState;
use cassette_core::librarian::enrich::discogs::DiscogsClient;
use cassette_core::librarian::enrich::lastfm::LastFmClient;
use cassette_core::models::{NowPlayingContext, PlaybackState};
use chrono::{Duration, NaiveDateTime, Utc};
use tauri::{AppHandle, Emitter, State};

const LYRICS_CACHE_TTL_DAYS: i64 = 30;
const LYRICS_PREFETCH_PLAYBACK_LIMIT: usize = 12;
const LYRICS_PREFETCH_FINALIZED_LIMIT: usize = 12;
const LYRICS_PREFETCH_MAX_ITEMS: usize = 4;
const LYRICS_PREFETCH_TIMEOUT_SECS: u64 = 6;

fn emit_playback_state(app: &AppHandle, state: &AppState) {
    let mut ps = state.playback_state.lock().unwrap().clone();
    ps.position_secs = state.player.position_secs();
    ps.duration_secs = state.player.duration_secs();
    ps.is_playing = state.player.is_playing();
    ps.volume = state.player.volume();

    if let Err(error) = app.emit("playback_state_changed", &ps) {
        tracing::warn!("[emit_playback_state] failed to emit playback state: {error}");
    }
}

#[tauri::command]
pub fn player_load(app: AppHandle, state: State<'_, AppState>, path: String) {
    state.player.load(path.clone());
    let db = state.db.lock().unwrap();
    if let Ok(Some(t)) = db.get_track_by_path(&path) {
        let mut ps = state.playback_state.lock().unwrap();
        ps.current_track = Some(t);
    }
    emit_playback_state(&app, &state);
}

#[tauri::command]
pub fn player_play(app: AppHandle, state: State<'_, AppState>) {
    let was_playing = state.player.is_playing();
    state.player.play();

    if !was_playing && state.player.position_secs() <= 1.0 {
        let current_track = {
            let ps = state.playback_state.lock().unwrap();
            ps.current_track.clone()
        };
        if let Some(track) = current_track {
            let db = state.db.lock().unwrap();
            if let Err(error) = db.increment_play_count(track.id) {
                tracing::warn!(
                    "[player_play] failed to increment play count for track {}: {error}",
                    track.id
                );
            }
            if let Err(error) = db.record_play_history_event(
                "local_playback",
                &track.artist,
                &track.title,
                Some(&track.album),
                None,
                Some(track.id),
            ) {
                tracing::warn!(
                    "[player_play] failed to record play-history event for track {}: {error}",
                    track.id
                );
            }
        }

        spawn_lyrics_prefetch_lane(state.clone());
    }

    emit_playback_state(&app, &state);
}

#[tauri::command]
pub fn player_pause(app: AppHandle, state: State<'_, AppState>) {
    state.player.pause();
    emit_playback_state(&app, &state);
}

#[tauri::command]
pub fn player_stop(state: State<'_, AppState>) {
    state.player.stop();
    let mut ps = state.playback_state.lock().unwrap();
    ps.current_track = None;
    ps.queue_position = 0;
}

#[tauri::command]
pub fn player_toggle(app: AppHandle, state: State<'_, AppState>) {
    state.player.toggle();
    emit_playback_state(&app, &state);
}

#[tauri::command]
pub fn player_next(app: AppHandle, state: State<'_, AppState>) {
    let was_playing = state.player.is_playing();
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
            if was_playing {
                if let Err(e) = db.increment_play_count(track.id) {
                    tracing::warn!(
                        "[player_next] failed to increment play count for track {}: {e}",
                        track.id
                    );
                }
                if let Err(e) = db.record_play_history_event(
                    "local_playback",
                    &track.artist,
                    &track.title,
                    Some(&track.album),
                    None,
                    Some(track.id),
                ) {
                    tracing::warn!(
                        "[player_next] failed to record play-history event for track {}: {e}",
                        track.id
                    );
                }
            }
            spawn_lyrics_prefetch_lane(state.clone());
        }
    }
    emit_playback_state(&app, &state);
}

#[tauri::command]
pub fn player_prev(app: AppHandle, state: State<'_, AppState>) {
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
    emit_playback_state(&app, &state);
}

#[tauri::command]
pub fn player_set_volume(app: AppHandle, state: State<'_, AppState>, volume: f32) {
    state.player.set_volume(volume);
    let mut ps = state.playback_state.lock().unwrap();
    ps.volume = volume;
    emit_playback_state(&app, &state);
}

#[tauri::command]
pub fn player_seek(app: AppHandle, state: State<'_, AppState>, secs: f64) {
    state.player.seek(secs);
    emit_playback_state(&app, &state);
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

    let (lastfm_key, discogs_token) = {
        let db = state.db.lock().unwrap();
        (
            db.get_setting("lastfm_api_key")
                .ok()
                .flatten()
                .filter(|value| !value.trim().is_empty()),
            db.get_setting("discogs_token")
                .ok()
                .flatten()
                .filter(|value| !value.trim().is_empty()),
        )
    };

    let cached_lyrics = {
        let db = state.db.lock().unwrap();
        db.get_cached_track_lyrics(&artist, &title, album.as_deref())
            .ok()
            .flatten()
    };

    if let Some(cached_lyrics) = cached_lyrics.clone() {
        ctx.lyrics = cached_lyrics.lyrics;
        ctx.synced_lyrics = cached_lyrics.synced_lyrics;
        ctx.lyrics_source = Some(cached_lyrics.source);
    }

    let lastfm_client = LastFmClient::new(lastfm_key.or_else(|| {
        std::env::var("LASTFM_API_KEY")
            .ok()
            .filter(|value| !value.trim().is_empty())
    }));
    let discogs_client = DiscogsClient::new(discogs_token.or_else(|| {
        std::env::var("DISCOGS_TOKEN")
            .ok()
            .filter(|value| !value.trim().is_empty())
    }));

    if let Some(info) = lastfm_client.fetch_artist_context(&client, &artist).await {
        if ctx.artist_summary.is_none() {
            ctx.artist_summary = info.summary;
        }
        if ctx.listeners.is_none() {
            ctx.listeners = info.listeners;
        }
        merge_unique_tags(&mut ctx.artist_tags, info.tags);
    }

    if let Some(ref alb) = album {
        if let Some(info) = lastfm_client
            .fetch_album_context(&client, &artist, alb)
            .await
        {
            if ctx.album_summary.is_none() {
                ctx.album_summary = info.summary;
            }
            if ctx.album_art_url.is_none() {
                ctx.album_art_url = info.image_url;
            }
        }

        if let Some(release) = discogs_client
            .fetch_release_context(&client, &artist, alb)
            .await
        {
            let mut discogs_tags = release.genres;
            discogs_tags.extend(release.styles);
            merge_unique_tags(&mut ctx.artist_tags, discogs_tags);

            if ctx.album_summary.is_none() {
                let mut parts: Vec<String> = Vec::new();
                if let Some(year) = release.year {
                    parts.push(year.to_string());
                }
                if !release.labels.is_empty() {
                    parts.push(format!("label: {}", release.labels.join(", ")));
                }
                if let Some(country) = release.country {
                    parts.push(format!("country: {country}"));
                }
                if !parts.is_empty() {
                    ctx.album_summary =
                        Some(format!("Discogs release metadata ({})", parts.join(" | ")));
                }
            }
        }
    }

    let needs_lyrics_fetch = cached_lyrics.as_ref().map_or(true, |cached| {
        lyrics_cache_is_stale(&cached.fetched_at)
            || cached.lyrics.is_none()
            || cached.synced_lyrics.is_none()
    });
    if needs_lyrics_fetch {
        let lyrics_result = fetch_lrclib_lyrics(&client, &artist, &title, album.as_deref()).await;
        if let Some(lr) = lyrics_result {
            let plain = lr.plain.clone();
            let synced = lr.synced.clone();
            {
                let db = state.db.lock().unwrap();
                let _ = db.upsert_track_lyrics(
                    None,
                    &artist,
                    &title,
                    album.as_deref(),
                    plain.as_deref(),
                    synced.as_deref(),
                    "LRCLIB",
                );
            }
            ctx.lyrics = plain.or(ctx.lyrics);
            ctx.synced_lyrics = synced.or(ctx.synced_lyrics);
            ctx.lyrics_source = Some("LRCLIB".into());
        }
    }

    Ok(ctx)
}

#[tauri::command]
pub async fn sync_lastfm_history(
    state: State<'_, AppState>,
    username: Option<String>,
    limit: Option<u32>,
) -> Result<usize, String> {
    let (lastfm_key, configured_username) = {
        let db = state.db.lock().unwrap();
        (
            db.get_setting("lastfm_api_key")
                .ok()
                .flatten()
                .filter(|value| !value.trim().is_empty())
                .or_else(|| {
                    std::env::var("LASTFM_API_KEY")
                        .ok()
                        .filter(|value| !value.trim().is_empty())
                }),
            db.get_setting("lastfm_username")
                .ok()
                .flatten()
                .filter(|value| !value.trim().is_empty())
                .or_else(|| {
                    std::env::var("LASTFM_USERNAME")
                        .ok()
                        .filter(|value| !value.trim().is_empty())
                }),
        )
    };

    let api_key = lastfm_key.ok_or_else(|| {
        "Last.fm API key missing. Set lastfm_api_key in settings or LASTFM_API_KEY in environment.".to_string()
    })?;
    let resolved_username = username
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
        .or(configured_username)
        .ok_or_else(|| {
            "Last.fm username missing. Pass username or set lastfm_username / LASTFM_USERNAME."
                .to_string()
        })?;

    let history_limit = limit.unwrap_or(200).clamp(1, 200);
    let lastfm_client = LastFmClient::new(Some(api_key));
    let recent_tracks = lastfm_client
        .fetch_recent_tracks(&state.http_client, &resolved_username, history_limit)
        .await
        .ok_or_else(|| "Last.fm history fetch failed or returned no parsable rows.".to_string())?;

    let mut inserted = 0usize;
    let db = state.db.lock().unwrap();
    for scrobble in recent_tracks {
        let was_inserted = db
            .record_play_history_event(
                "lastfm",
                &scrobble.artist,
                &scrobble.title,
                scrobble.album.as_deref(),
                scrobble.played_at.as_deref(),
                None,
            )
            .map_err(|error| format!("record_play_history_event failed: {error}"))?;
        if !was_inserted {
            continue;
        }
        inserted += 1;
        let _ = db.increment_play_count_by_identity(
            &scrobble.artist,
            &scrobble.title,
            scrobble.played_at.as_deref(),
        );
    }

    Ok(inserted)
}

#[tauri::command]
pub async fn submit_lastfm_scrobble(
    state: State<'_, AppState>,
    track_id: i64,
    artist: String,
    title: String,
    album: Option<String>,
    duration_secs: Option<f64>,
    position_secs: f64,
) -> Result<bool, String> {
    let threshold_secs = lastfm_scrobble_threshold_secs(duration_secs);
    if position_secs < threshold_secs {
        return Ok(false);
    }

    let (api_key, api_secret, session_key) = {
        let db = state.db.lock().unwrap();
        (
            db.get_setting("lastfm_api_key")
                .ok()
                .flatten()
                .filter(|value| !value.trim().is_empty())
                .or_else(|| {
                    std::env::var("LASTFM_API_KEY")
                        .ok()
                        .filter(|value| !value.trim().is_empty())
                }),
            db.get_setting("lastfm_api_secret")
                .ok()
                .flatten()
                .filter(|value| !value.trim().is_empty())
                .or_else(|| {
                    std::env::var("LASTFM_API_SECRET")
                        .ok()
                        .filter(|value| !value.trim().is_empty())
                }),
            db.get_setting("lastfm_session_key")
                .ok()
                .flatten()
                .filter(|value| !value.trim().is_empty())
                .or_else(|| {
                    std::env::var("LASTFM_SESSION_KEY")
                        .ok()
                        .filter(|value| !value.trim().is_empty())
                }),
        )
    };

    let client = LastFmClient::new(api_key).with_scrobble_auth(api_secret, session_key);
    if !client.is_scrobble_configured() {
        tracing::warn!(
            track_id,
            "Last.fm scrobble skipped: missing API key/secret/session key"
        );
        return Ok(false);
    }

    let scrobble_time = (Utc::now() - Duration::seconds(position_secs.max(0.0).floor() as i64))
        .timestamp();
    let scrobbled = client
        .scrobble_track(
            &state.http_client,
            &artist,
            &title,
            album.as_deref(),
            scrobble_time,
            duration_secs
                .and_then(|value| if value > 0.0 { Some(value.round() as u32) } else { None }),
        )
        .await
        .is_some();

    if !scrobbled {
        tracing::warn!(track_id, "Last.fm scrobble request returned no success result");
        return Ok(false);
    }

    {
        let db = state.db.lock().unwrap();
        let _ = db.record_play_history_event(
            "lastfm_scrobble",
            &artist,
            &title,
            album.as_deref(),
            None,
            Some(track_id),
        );
    }

    Ok(true)
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

fn spawn_lyrics_prefetch_lane(state: State<'_, AppState>) {
    let db = std::sync::Arc::clone(&state.db);
    let client = state.http_client.clone();

    tauri::async_runtime::spawn(async move {
        let candidates = {
            let Ok(db) = db.lock() else {
                return;
            };
            db.get_lyrics_prefetch_candidates(
                LYRICS_PREFETCH_PLAYBACK_LIMIT,
                LYRICS_PREFETCH_FINALIZED_LIMIT,
            )
            .unwrap_or_default()
        };

        for candidate in candidates.into_iter().take(LYRICS_PREFETCH_MAX_ITEMS) {
            let is_fresh = {
                let Ok(db) = db.lock() else {
                    return;
                };
                db.get_cached_track_lyrics(
                    &candidate.artist,
                    &candidate.title,
                    candidate.album.as_deref(),
                )
                .ok()
                .flatten()
                .is_some_and(|cached| {
                    !lyrics_cache_is_stale(&cached.fetched_at)
                        && (cached.lyrics.is_some() || cached.synced_lyrics.is_some())
                })
            };

            if is_fresh {
                continue;
            }

            let fetched = tokio::time::timeout(
                std::time::Duration::from_secs(LYRICS_PREFETCH_TIMEOUT_SECS),
                fetch_lrclib_lyrics(
                    &client,
                    &candidate.artist,
                    &candidate.title,
                    candidate.album.as_deref(),
                ),
            )
            .await
            .ok()
            .flatten();

            if let Some(result) = fetched {
                let Ok(db) = db.lock() else {
                    return;
                };
                let _ = db.upsert_track_lyrics(
                    None,
                    &candidate.artist,
                    &candidate.title,
                    candidate.album.as_deref(),
                    result.plain.as_deref(),
                    result.synced.as_deref(),
                    "LRCLIB",
                );
            }
        }
    });
}

fn merge_unique_tags(existing: &mut Vec<String>, incoming: Vec<String>) {
    for tag in incoming {
        let normalized = tag.trim();
        if normalized.is_empty() {
            continue;
        }
        if existing
            .iter()
            .any(|current| current.eq_ignore_ascii_case(normalized))
        {
            continue;
        }
        existing.push(normalized.to_string());
    }
}

fn lastfm_scrobble_threshold_secs(duration_secs: Option<f64>) -> f64 {
    let half_track = duration_secs
        .filter(|value| *value > 0.0)
        .map(|value| value * 0.5)
        .unwrap_or(120.0);
    half_track.min(240.0).max(30.0)
}

fn lyrics_cache_is_stale(fetched_at: &str) -> bool {
    let Ok(parsed) = NaiveDateTime::parse_from_str(fetched_at.trim(), "%Y-%m-%d %H:%M:%S") else {
        return true;
    };
    let age = Utc::now().naive_utc() - parsed;
    age > Duration::days(LYRICS_CACHE_TTL_DAYS)
}

#[cfg(test)]
mod tests {
    use super::{lastfm_scrobble_threshold_secs, lyrics_cache_is_stale};

    #[test]
    fn lyrics_cache_ttl_marks_old_rows_as_stale() {
        assert!(lyrics_cache_is_stale("2024-01-01 00:00:00"));
        assert!(!lyrics_cache_is_stale("2999-01-01 00:00:00"));
        assert!(lyrics_cache_is_stale("not-a-datetime"));
    }

    #[test]
    fn scrobble_threshold_matches_lastfm_rules() {
        assert_eq!(lastfm_scrobble_threshold_secs(Some(60.0)), 30.0);
        assert_eq!(lastfm_scrobble_threshold_secs(Some(600.0)), 240.0);
        assert_eq!(lastfm_scrobble_threshold_secs(Some(300.0)), 150.0);
    }
}
