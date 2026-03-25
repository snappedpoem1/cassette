use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpotifyExportTrack {
    pub track_id: Option<String>,
    pub album_id: Option<String>,
    pub artist_id: Option<String>,
    pub artist_name: String,
    pub album_title: Option<String>,
    pub track_title: String,
    pub track_number: Option<i64>,
    pub disc_number: Option<i64>,
    pub duration_ms: Option<i64>,
    pub isrc: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SpotifyExportPayload {
    pub source_name: Option<String>,
    pub tracks: Vec<SpotifyExportTrack>,
}

pub fn parse_spotify_payload(json: &str) -> Result<SpotifyExportPayload, serde_json::Error> {
    match serde_json::from_str::<SpotifyExportPayload>(json) {
        Ok(payload) => Ok(payload),
        Err(_) => {
            let tracks = serde_json::from_str::<Vec<SpotifyExportTrack>>(json)?;
            Ok(SpotifyExportPayload {
                source_name: Some("spotify".to_string()),
                tracks,
            })
        }
    }
}
