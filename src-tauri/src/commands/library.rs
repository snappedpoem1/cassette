use crate::state::AppState;
use cassette_core::{
    library::Scanner,
    models::{Album, Artist, LibraryRoot, ScanProgress, Track},
};
use tauri::{Emitter, State, Window};

#[tauri::command]
pub fn get_library_roots(state: State<'_, AppState>) -> Vec<LibraryRoot> {
    state.db.lock().unwrap().get_library_roots().unwrap_or_default()
}

#[tauri::command]
pub fn add_library_root(state: State<'_, AppState>, path: String) {
    let _ = state.db.lock().unwrap().add_library_root(&path);
}

#[tauri::command]
pub fn remove_library_root(state: State<'_, AppState>, path: String) {
    let _ = state.db.lock().unwrap().remove_library_root(&path);
}

#[tauri::command]
pub async fn scan_library(window: Window, state: State<'_, AppState>) -> Result<u64, String> {
    let roots = state
        .db.lock().unwrap()
        .get_library_roots()
        .unwrap_or_default()
        .into_iter()
        .filter(|r| r.enabled)
        .map(|r| r.path)
        .collect::<Vec<_>>();

    let (tx, mut rx) = tokio::sync::mpsc::channel::<ScanProgress>(256);
    let db = std::sync::Arc::clone(&state.db);
    let w = window.clone();

    // Forward progress events to the frontend
    tokio::spawn(async move {
        while let Some(p) = rx.recv().await {
            let _ = w.emit("scan-progress", &p);
        }
    });

    let scanner = Scanner::new(db);
    scanner.scan_roots(roots, tx).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_tracks(state: State<'_, AppState>, limit: Option<i64>, offset: Option<i64>) -> Vec<Track> {
    state.db.lock().unwrap()
        .get_tracks(limit.unwrap_or(500), offset.unwrap_or(0))
        .unwrap_or_default()
}

#[tauri::command]
pub fn search_tracks(state: State<'_, AppState>, query: String) -> Vec<Track> {
    state.db.lock().unwrap().search_tracks(&query).unwrap_or_default()
}

#[tauri::command]
pub fn get_albums(state: State<'_, AppState>) -> Vec<Album> {
    state.db.lock().unwrap().get_albums().unwrap_or_default()
}

#[tauri::command]
pub fn get_album_tracks(state: State<'_, AppState>, artist: String, album: String) -> Vec<Track> {
    state.db.lock().unwrap().get_album_tracks(&artist, &album).unwrap_or_default()
}

#[tauri::command]
pub fn get_artists(state: State<'_, AppState>) -> Vec<Artist> {
    state.db.lock().unwrap().get_artists().unwrap_or_default()
}

#[tauri::command]
pub fn get_track_count(state: State<'_, AppState>) -> i64 {
    state.db.lock().unwrap().get_track_count().unwrap_or(0)
}
