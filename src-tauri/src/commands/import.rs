use crate::commands::downloads::queue_album_tracks;
use crate::spotify_history::{
    collect_spotify_history_files, parse_spotify_entries, summarize_spotify_albums,
    SpotifyAlbumSummary, SpotifyImportResult,
};
use crate::state::AppState;
use cassette_core::director::{AcquisitionStrategy, TrackTaskSource};
use cassette_core::librarian::import::import_desired_spotify_json;
use cassette_core::models::SpotifyAlbumHistory;
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;
use tauri::State;

/// Parse Spotify Extended Streaming History JSON files from a directory.
/// Returns aggregated album stats sorted by total listen time descending.
#[tauri::command]
pub fn parse_spotify_history(
    state: State<'_, AppState>,
    path: String,
) -> Result<SpotifyImportResult, String> {
    let dir = Path::new(&path);
    let json_files = collect_spotify_history_files(dir)?;
    let all_entries = parse_spotify_entries(&json_files)?;

    let total_streams = all_entries.len();

    // Check which albums are already in the library
    let db = state.db.lock().unwrap();
    let library_albums: Vec<_> = db.get_albums().unwrap_or_default();
    let library_track_counts: HashMap<(String, String), usize> = library_albums
        .iter()
        .map(|a| {
            (
                (a.artist.to_lowercase(), a.title.to_lowercase()),
                a.track_count.max(0) as usize,
            )
        })
        .collect();
    let albums = summarize_spotify_albums(&all_entries, &library_track_counts);

    let history_rows = albums
        .iter()
        .map(|album| SpotifyAlbumHistory {
            artist: album.artist.clone(),
            album: album.album.clone(),
            total_ms: album.total_ms,
            play_count: album.play_count,
            skip_count: album.skip_count,
            in_library: album.in_library,
            imported_at: String::new(),
        })
        .collect::<Vec<_>>();
    let _ = db.replace_spotify_album_history(&history_rows);

    let unique_albums = albums.len();
    let already_in_library = albums.iter().filter(|a| a.in_library).count();

    Ok(SpotifyImportResult {
        albums,
        total_streams,
        unique_albums,
        already_in_library,
    })
}

#[tauri::command]
pub async fn import_spotify_desired_tracks(
    state: State<'_, AppState>,
    path: String,
) -> Result<usize, String> {
    let json = tokio::fs::read_to_string(&path)
        .await
        .map_err(|error| format!("Cannot read Spotify import JSON: {error}"))?;
    import_desired_spotify_json(&state.control_db, &json)
        .await
        .map_err(|error| error.to_string())
}

/// Queue download jobs for Spotify albums not already in the library.
/// Accepts a list of album indices from the parse result to download.
#[tauri::command]
pub async fn queue_spotify_albums(
    state: State<'_, AppState>,
    albums: Vec<SpotifyAlbumSummary>,
) -> Result<usize, String> {
    let completed_keys = state
        .db
        .lock()
        .map_err(|error| error.to_string())?
        .get_completed_task_keys()
        .map_err(|error| error.to_string())?;
    let mut queued = 0;
    for album in &albums {
        if album.in_library {
            continue;
        }
        let job_ids = queue_album_tracks(
            state.clone(),
            album.artist.as_str(),
            album.album.as_str(),
            TrackTaskSource::SpotifyLibrary,
            AcquisitionStrategy::DiscographyBatch,
            &completed_keys,
        )
        .await?;
        queued += job_ids.len();
    }
    Ok(queued)
}

#[derive(Debug, Clone, Serialize)]
pub struct SpotifyImportStatus {
    pub album_rows: i64,
    pub last_imported_at: Option<String>,
}

#[tauri::command]
pub fn get_spotify_import_status(state: State<'_, AppState>) -> Result<SpotifyImportStatus, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let album_rows = db
        .get_spotify_album_history_count()
        .map_err(|e| e.to_string())?;
    let last_imported_at = db
        .get_spotify_album_history_last_imported_at()
        .map_err(|e| e.to_string())?;
    Ok(SpotifyImportStatus {
        album_rows,
        last_imported_at,
    })
}
