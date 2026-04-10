use crate::state::AppState;
use cassette_core::models::QueueItem;
use tauri::{AppHandle, Emitter, State};

#[tauri::command]
pub fn get_queue(state: State<'_, AppState>) -> Vec<QueueItem> {
    state.db.lock().unwrap().get_queue().unwrap_or_default()
}

#[tauri::command]
pub fn clear_queue(state: State<'_, AppState>) {
    let _ = state.db.lock().unwrap().clear_queue();
    let mut ps = state.playback_state.lock().unwrap();
    ps.queue_position = 0;
}

#[tauri::command]
pub fn add_to_queue(state: State<'_, AppState>, track_id: i64, position: Option<i64>) {
    let db = state.db.lock().unwrap();
    let pos = position.unwrap_or_else(|| db.get_max_queue_position().unwrap_or(-1) + 1);
    let _ = db.add_to_queue(track_id, pos);
}

#[tauri::command]
pub fn queue_tracks(
    app: AppHandle,
    state: State<'_, AppState>,
    track_ids: Vec<i64>,
    start_index: Option<usize>,
) {
    let db = state.db.lock().unwrap();
    let _ = db.clear_queue();
    for (pos, tid) in track_ids.iter().enumerate() {
        let _ = db.add_to_queue(*tid, pos as i64);
    }
    drop(db);

    let start = start_index.unwrap_or(0);
    let queue = state.db.lock().unwrap().get_queue().unwrap_or_default();
    if let Some(item) = queue.get(start) {
        if let Some(ref track) = item.track {
            state.player.load(track.path.clone());
            state.player.play();
            {
                let mut ps = state.playback_state.lock().unwrap();
                ps.current_track = Some(track.clone());
                ps.queue_position = start;
                ps.is_playing = true;
            }
            let _ = state.db.lock().unwrap().increment_play_count(track.id);
        }
    }

    // Emit updated playback state immediately so frontend doesn't wait for the poll cycle
    let mut ps = state.playback_state.lock().unwrap().clone();
    ps.position_secs = state.player.position_secs();
    ps.duration_secs = state.player.duration_secs();
    ps.is_playing = state.player.is_playing();
    ps.volume = state.player.volume();
    if let Err(e) = app.emit("playback_state_changed", &ps) {
        tracing::warn!("[queue_tracks] failed to emit playback state: {e}");
    }
}
