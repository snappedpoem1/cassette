use cassette_core::db::Db;
use cassette_core::models::SpotifyAlbumHistory;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
struct SpotifyStreamEntry {
    ms_played: Option<u64>,
    master_metadata_album_artist_name: Option<String>,
    master_metadata_album_album_name: Option<String>,
    #[serde(default)]
    skipped: Option<bool>,
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

    Err(
        "Path must be a directory containing Spotify history JSON files, or a single JSON file"
            .into(),
    )
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
) -> Vec<SpotifyAlbumHistory> {
    let mut album_map: HashMap<(String, String), SpotifyAlbumHistory> = HashMap::new();

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
        let summary = album_map.entry(key).or_insert_with(|| SpotifyAlbumHistory {
            artist: artist.clone(),
            album: album.clone(),
            total_ms: 0,
            play_count: 0,
            skip_count: 0,
            in_library: false,
            imported_at: String::new(),
        });
        summary.total_ms += ms;
        if skipped {
            summary.skip_count += 1;
        } else {
            summary.play_count += 1;
        }
    }

    let mut albums: Vec<SpotifyAlbumHistory> = album_map
        .into_values()
        .map(|mut s| {
            s.in_library =
                library_set.contains_key(&(s.artist.to_lowercase(), s.album.to_lowercase()));
            s
        })
        .collect();
    albums.sort_by(|a, b| b.total_ms.cmp(&a.total_ms));
    albums
}

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let source = args
        .next()
        .ok_or_else(|| "Usage: spotify_import_cli <spotify_history_dir_or_json>".to_string())?;

    let app_data =
        std::env::var("APPDATA").map_err(|_| "APPDATA env var is not set".to_string())?;
    let db_path = PathBuf::from(app_data)
        .join("dev.cassette.app")
        .join("cassette.db");
    let db = Db::open(&db_path).map_err(|e| e.to_string())?;

    let json_files = collect_spotify_history_files(Path::new(&source))?;
    let entries = parse_spotify_entries(&json_files)?;

    let library_albums = db.get_albums().map_err(|e| e.to_string())?;
    let library_set = library_albums
        .iter()
        .map(|a| ((a.artist.to_lowercase(), a.title.to_lowercase()), true))
        .collect::<HashMap<_, _>>();

    let rows = summarize_spotify_albums(&entries, &library_set);
    db.replace_spotify_album_history(&rows)
        .map_err(|e| e.to_string())?;

    println!(
        "Imported {} album rows into {}",
        rows.len(),
        db_path.display()
    );
    Ok(())
}
