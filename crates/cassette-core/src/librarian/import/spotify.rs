use serde::{Deserialize, Serialize};
use serde_json::Value;

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_payload: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SpotifyExportPayload {
    pub source_name: Option<String>,
    pub tracks: Vec<SpotifyExportTrack>,
}

pub fn parse_spotify_payload(json: &str) -> Result<SpotifyExportPayload, serde_json::Error> {
    let value: Value = serde_json::from_str(json)?;

    if let Some(object) = value.as_object() {
        let source_name = object
            .get("source_name")
            .and_then(Value::as_str)
            .map(ToString::to_string)
            .or_else(|| Some("spotify".to_string()));

        if let Some(items) = object.get("tracks").and_then(Value::as_array) {
            return Ok(SpotifyExportPayload {
                source_name,
                tracks: items
                    .iter()
                    .cloned()
                    .filter_map(spotify_track_from_value)
                    .collect(),
            });
        }

        if let Some(items) = object.get("items").and_then(Value::as_array) {
            return Ok(SpotifyExportPayload {
                source_name,
                tracks: items
                    .iter()
                    .cloned()
                    .filter_map(spotify_track_from_value)
                    .collect(),
            });
        }
    }

    if let Some(items) = value.as_array() {
        return Ok(SpotifyExportPayload {
            source_name: Some("spotify".to_string()),
            tracks: items
                .iter()
                .cloned()
                .filter_map(spotify_track_from_value)
                .collect(),
        });
    }

    Ok(SpotifyExportPayload {
        source_name: Some("spotify".to_string()),
        tracks: Vec::new(),
    })
}

fn spotify_track_from_value(value: Value) -> Option<SpotifyExportTrack> {
    let track_obj = value.get("track").cloned().unwrap_or_else(|| value.clone());

    let artist_name = track_obj
        .get("artist_name")
        .and_then(Value::as_str)
        .or_else(|| track_obj.get("artist").and_then(Value::as_str))
        .or_else(|| {
            track_obj
                .get("artists")
                .and_then(Value::as_array)
                .and_then(|artists| artists.first())
                .and_then(|artist| artist.get("name"))
                .and_then(Value::as_str)
        })
        .or_else(|| value.get("master_metadata_album_artist_name").and_then(Value::as_str))?
        .trim()
        .to_string();

    let track_title = track_obj
        .get("track_title")
        .and_then(Value::as_str)
        .or_else(|| track_obj.get("title").and_then(Value::as_str))
        .or_else(|| track_obj.get("name").and_then(Value::as_str))?
        .trim()
        .to_string();

    let album_title = track_obj
        .get("album_title")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .or_else(|| {
            track_obj
                .get("album")
                .and_then(album_title_from_value)
        })
        .or_else(|| {
            value.get("master_metadata_album_album_name")
                .and_then(Value::as_str)
                .map(ToString::to_string)
        });

    let track_id = track_obj
        .get("track_id")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .or_else(|| spotify_uri_like_id(track_obj.get("uri").and_then(Value::as_str), "track"))
        .or_else(|| spotify_prefixed_id(track_obj.get("id").and_then(Value::as_str), "track"));

    let album_id = track_obj
        .get("album_id")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .or_else(|| {
            track_obj
                .get("album")
                .and_then(album_id_from_value)
        });

    let artist_id = track_obj
        .get("artist_id")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .or_else(|| {
            track_obj
                .get("artists")
                .and_then(Value::as_array)
                .and_then(|artists| artists.first())
                .and_then(|artist| spotify_uri_like_id(artist.get("uri").and_then(Value::as_str), "artist"))
                .or_else(|| {
                    track_obj
                        .get("artists")
                        .and_then(Value::as_array)
                        .and_then(|artists| artists.first())
                        .and_then(|artist| spotify_prefixed_id(artist.get("id").and_then(Value::as_str), "artist"))
                })
        });

    let track_number = track_obj
        .get("track_number")
        .and_then(as_i64);
    let disc_number = track_obj
        .get("disc_number")
        .and_then(as_i64);
    let duration_ms = track_obj
        .get("duration_ms")
        .and_then(as_i64)
        .or_else(|| value.get("ms_played").and_then(as_i64))
        .or_else(|| track_obj.get("duration").and_then(as_i64));

    let isrc = track_obj
        .get("isrc")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .or_else(|| {
            track_obj
                .get("external_ids")
                .and_then(|external_ids| external_ids.get("isrc"))
                .and_then(Value::as_str)
                .map(ToString::to_string)
        });

    Some(SpotifyExportTrack {
        track_id,
        album_id,
        artist_id,
        artist_name,
        album_title,
        track_title,
        track_number,
        disc_number,
        duration_ms,
        isrc,
        raw_payload: Some(value),
    })
}

fn album_title_from_value(value: &Value) -> Option<String> {
    if let Some(album) = value.as_str() {
        return Some(album.to_string());
    }

    value
        .get("name")
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn album_id_from_value(value: &Value) -> Option<String> {
    spotify_uri_like_id(value.get("uri").and_then(Value::as_str), "album")
        .or_else(|| spotify_prefixed_id(value.get("id").and_then(Value::as_str), "album"))
}

fn spotify_uri_like_id(value: Option<&str>, kind: &str) -> Option<String> {
    value.and_then(|id| {
        let trimmed = id.trim();
        if trimmed.is_empty() {
            None
        } else if trimmed.starts_with("spotify:") {
            Some(trimmed.to_string())
        } else {
            Some(format!("spotify:{kind}:{trimmed}"))
        }
    })
}

fn spotify_prefixed_id(value: Option<&str>, kind: &str) -> Option<String> {
    spotify_uri_like_id(value, kind)
}

fn as_i64(value: &Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|number| i64::try_from(number).ok()))
        .or_else(|| value.as_str().and_then(|number| number.parse::<i64>().ok()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_structured_payload_and_keeps_raw_payload() {
        let payload = parse_spotify_payload(
            r#"{"source_name":"spotify","tracks":[{"track_id":"spotify:track:1","album_id":"spotify:album:2","artist_id":"spotify:artist:3","artist_name":"Artist","album_title":"Album","track_title":"Song","track_number":1,"disc_number":1,"duration_ms":123000,"isrc":"US1234567890"}]}"#,
        )
        .expect("payload");

        assert_eq!(payload.tracks.len(), 1);
        assert_eq!(payload.tracks[0].track_id.as_deref(), Some("spotify:track:1"));
        assert_eq!(payload.tracks[0].album_id.as_deref(), Some("spotify:album:2"));
        assert_eq!(payload.tracks[0].artist_id.as_deref(), Some("spotify:artist:3"));
        assert!(payload.tracks[0].raw_payload.is_some());
    }

    #[test]
    fn parses_spotify_api_items_payload() {
        let payload = parse_spotify_payload(
            r#"{"items":[{"track":{"id":"track123","name":"Song","duration_ms":200000,"track_number":4,"disc_number":1,"external_ids":{"isrc":"USRC17607839"},"album":{"id":"album123","name":"Album"},"artists":[{"id":"artist123","name":"Artist"}]}}]}"#,
        )
        .expect("payload");

        let track = &payload.tracks[0];
        assert_eq!(track.track_id.as_deref(), Some("spotify:track:track123"));
        assert_eq!(track.album_id.as_deref(), Some("spotify:album:album123"));
        assert_eq!(track.artist_id.as_deref(), Some("spotify:artist:artist123"));
        assert_eq!(track.isrc.as_deref(), Some("USRC17607839"));
    }
}
