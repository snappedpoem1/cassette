use crate::state::AppState;
use cassette_core::models::{Playlist, PlaylistItem};
use tauri::State;

#[tauri::command]
pub fn get_playlists(state: State<'_, AppState>) -> Vec<Playlist> {
    state.db.lock().unwrap().get_playlists().unwrap_or_default()
}

#[tauri::command]
pub fn get_playlist_items(state: State<'_, AppState>, playlist_id: i64) -> Vec<PlaylistItem> {
    state
        .db
        .lock()
        .unwrap()
        .get_playlist_items(playlist_id)
        .unwrap_or_default()
}

#[tauri::command]
pub fn create_playlist(
    state: State<'_, AppState>,
    name: String,
    description: Option<String>,
    track_ids: Vec<i64>,
) -> i64 {
    state
        .db
        .lock()
        .unwrap()
        .create_playlist(&name, description.as_deref(), &track_ids)
        .unwrap_or(-1)
}

#[tauri::command]
pub fn replace_playlist_tracks(state: State<'_, AppState>, playlist_id: i64, track_ids: Vec<i64>) {
    let _ = state
        .db
        .lock()
        .unwrap()
        .replace_playlist_tracks(playlist_id, &track_ids);
}

#[tauri::command]
pub fn add_track_to_playlist(
    state: State<'_, AppState>,
    playlist_id: i64,
    track_id: i64,
) -> Result<(), String> {
    state
        .db
        .lock()
        .unwrap()
        .add_track_to_playlist(playlist_id, track_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn delete_playlist(state: State<'_, AppState>, playlist_id: i64) {
    let _ = state.db.lock().unwrap().delete_playlist(playlist_id);
}

#[tauri::command]
pub fn play_playlist(state: State<'_, AppState>, playlist_id: i64, start_index: Option<usize>) {
    let items = state
        .db
        .lock()
        .unwrap()
        .get_playlist_items(playlist_id)
        .unwrap_or_default();
    let track_ids: Vec<i64> = items.iter().map(|i| i.track_id).collect();

    // Reuse queue_tracks logic: clear queue, fill with playlist tracks, play from start
    let db = state.db.lock().unwrap();
    let _ = db.clear_queue();
    for (pos, tid) in track_ids.iter().enumerate() {
        let _ = db.add_to_queue(*tid, pos as i64);
    }
    drop(db);

    let start = start_index.unwrap_or(0);
    if let Some(item) = items.get(start) {
        if let Some(ref track) = item.track {
            state.player.load(track.path.clone());
            let mut ps = state.playback_state.lock().unwrap();
            ps.current_track = Some(track.clone());
            ps.queue_position = start;
        }
    }
}
