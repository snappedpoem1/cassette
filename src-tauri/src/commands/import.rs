use crate::state::AppState;
use crate::commands::downloads::queue_album_tracks;
use cassette_core::director::{AcquisitionStrategy, TrackTaskSource};
use cassette_core::models::SpotifyAlbumHistory;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tauri::State;

#[derive(Debug, Deserialize)]
struct SpotifyStreamEntry {
    ms_played: Option<u64>,
    master_metadata_album_artist_name: Option<String>,
    master_metadata_album_album_name: Option<String>,
    #[serde(default)]
    skipped: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpotifyAlbumSummary {
    pub artist: String,
    pub album: String,
    pub total_ms: u64,
    pub play_count: u32,
    pub skip_count: u32,
    pub in_library: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct SpotifyImportResult {
    pub albums: Vec<SpotifyAlbumSummary>,
    pub total_streams: usize,
    pub unique_albums: usize,
    pub already_in_library: usize,
}

fn collect_spotify_history_files(path: &Path) -> Result<Vec<PathBuf>, String> {
    if path.is_dir() {
        let files = std::fs::read_dir(path)
            .map_err(|e| format!("Cannot read directory: {e}"))?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.starts_with("Streaming_History_Audio") && n.ends_with(".json"))
                    .unwrap_or(false)
            })
            .map(|e| e.path())
            .collect::<Vec<_>>();
        return Ok(files);
    }

    if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("json") {
        return Ok(vec![path.to_path_buf()]);
    }

    Err("Path must be a directory containing Spotify history JSON files, or a single JSON file".into())
}

fn parse_spotify_entries(json_files: &[PathBuf]) -> Result<Vec<SpotifyStreamEntry>, String> {
    if json_files.is_empty() {
        return Err("No Streaming_History_Audio*.json files found in directory".into());
    }

    let mut all_entries: Vec<SpotifyStreamEntry> = Vec::new();
    for file_path in json_files {
        let data = std::fs::read_to_string(file_path)
            .map_err(|e| format!("Cannot read {}: {e}", file_path.display()))?;
        let entries: Vec<SpotifyStreamEntry> = serde_json::from_str(&data)
            .map_err(|e| format!("JSON parse error in {}: {e}", file_path.display()))?;
        all_entries.extend(entries);
    }
    Ok(all_entries)
}

fn summarize_spotify_albums(
    all_entries: &[SpotifyStreamEntry],
    library_set: &HashMap<(String, String), bool>,
) -> Vec<SpotifyAlbumSummary> {
    let mut album_map: HashMap<(String, String), SpotifyAlbumSummary> = HashMap::new();

    for entry in all_entries {
        let artist = match &entry.master_metadata_album_artist_name {
            Some(a) if !a.is_empty() => a.clone(),
            _ => continue,
        };
        let album = match &entry.master_metadata_album_album_name {
            Some(a) if !a.is_empty() => a.clone(),
            _ => continue,
        };
        let ms = entry.ms_played.unwrap_or(0);
        let skipped = entry.skipped.unwrap_or(false);

        let key = (artist.to_lowercase(), album.to_lowercase());
        let summary = album_map.entry(key).or_insert_with(|| SpotifyAlbumSummary {
            artist: artist.clone(),
            album: album.clone(),
            total_ms: 0,
            play_count: 0,
            skip_count: 0,
            in_library: false,
        });
        summary.total_ms += ms;
        if skipped {
            summary.skip_count += 1;
        } else {
            summary.play_count += 1;
        }
    }

    let mut albums: Vec<SpotifyAlbumSummary> = album_map
        .into_values()
        .map(|mut s| {
            s.in_library = library_set.contains_key(&(s.artist.to_lowercase(), s.album.to_lowercase()));
            s
        })
        .collect();

    albums.sort_by(|a, b| b.total_ms.cmp(&a.total_ms));
    albums
}

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
    let library_set: HashMap<(String, String), bool> = library_albums
        .iter()
        .map(|a| ((a.artist.to_lowercase(), a.title.to_lowercase()), true))
        .collect();
    let albums = summarize_spotify_albums(&all_entries, &library_set);

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

#[cfg(test)]
mod tests {
    use super::*;
    use cassette_core::db::Db;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be valid")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("cassette-{name}-{nanos}"));
        fs::create_dir_all(&dir).expect("temp dir should be created");
        dir
    }

    #[test]
    fn spotify_history_parser_summarizes_and_sorts() {
        let dir = temp_dir("spotify-history");
        let file = dir.join("Streaming_History_Audio_2026.json");
        let payload = r#"
[
  {
    "master_metadata_album_artist_name": "The National",
    "master_metadata_album_album_name": "Boxer",
    "ms_played": 200000,
    "skipped": false
  },
  {
    "master_metadata_album_artist_name": "The National",
    "master_metadata_album_album_name": "Boxer",
    "ms_played": 100000,
    "skipped": true
  },
  {
    "master_metadata_album_artist_name": "Interpol",
    "master_metadata_album_album_name": "Turn on the Bright Lights",
    "ms_played": 250000,
    "skipped": false
  }
]
"#;
        fs::write(&file, payload).expect("fixture should be written");

        let files = collect_spotify_history_files(&dir).expect("history files should be found");
        assert_eq!(files.len(), 1);
        let entries = parse_spotify_entries(&files).expect("entries should parse");
        assert_eq!(entries.len(), 3);

        let mut library_set = HashMap::new();
        library_set.insert(("the national".to_string(), "boxer".to_string()), true);
        let albums = summarize_spotify_albums(&entries, &library_set);

        assert_eq!(albums.len(), 2);
        assert_eq!(albums[0].artist, "The National");
        assert_eq!(albums[0].album, "Boxer");
        assert_eq!(albums[0].total_ms, 300000);
        assert_eq!(albums[0].play_count, 1);
        assert_eq!(albums[0].skip_count, 1);
        assert!(albums[0].in_library);

        assert_eq!(albums[1].artist, "Interpol");
        assert_eq!(albums[1].play_count, 1);
        assert_eq!(albums[1].skip_count, 0);
        assert!(!albums[1].in_library);

        let _ = fs::remove_file(&file);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn spotify_import_pipeline_persists_to_db() {
        let dir = temp_dir("spotify-import-pipeline");
        let file = dir.join("Streaming_History_Audio_2026.json");
        let payload = r#"
[
  {
    "master_metadata_album_artist_name": "Frightened Rabbit",
    "master_metadata_album_album_name": "The Midnight Organ Fight",
    "ms_played": 123000,
    "skipped": false
  },
  {
    "master_metadata_album_artist_name": "Frightened Rabbit",
    "master_metadata_album_album_name": "The Midnight Organ Fight",
    "ms_played": 110000,
    "skipped": true
  }
]
"#;
        fs::write(&file, payload).expect("fixture should be written");

        let db_path = dir.join("cassette-test.db");
        let db = Db::open(&db_path).expect("db should open");

        let files = collect_spotify_history_files(&dir).expect("history files should be found");
        let entries = parse_spotify_entries(&files).expect("entries should parse");
        let library_set = HashMap::new();
        let albums = summarize_spotify_albums(&entries, &library_set);

        let rows = albums
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
        db.replace_spotify_album_history(&rows)
            .expect("history rows should persist");

        let count = db
            .get_spotify_album_history_count()
            .expect("history count should be readable");
        assert_eq!(count, 1);

        let _ = fs::remove_file(&file);
        let _ = fs::remove_file(&db_path);
        let _ = fs::remove_dir_all(&dir);
    }
}
