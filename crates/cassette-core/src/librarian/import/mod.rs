pub mod spotify;

use crate::librarian::db::LibrarianDb;
use crate::librarian::error::{LibrarianError, Result};
use crate::librarian::import::spotify::parse_spotify_payload;
use tracing::info;

pub async fn import_desired_spotify_json(db: &LibrarianDb, json: &str) -> Result<usize> {
    let payload = parse_spotify_payload(json)
        .map_err(|error| LibrarianError::ImportError(error.to_string()))?;

    let source_name = payload
        .source_name
        .as_deref()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or("spotify");

    let mut imported = 0usize;
    for item in payload.tracks {
        let raw_payload = serde_json::to_string(&item).ok();
        db.insert_desired_track(
            source_name,
            item.track_id.as_deref(),
            item.album_id.as_deref(),
            item.artist_id.as_deref(),
            &item.artist_name,
            item.album_title.as_deref(),
            &item.track_title,
            item.track_number,
            item.disc_number,
            item.duration_ms,
            item.isrc.as_deref(),
            raw_payload.as_deref(),
        )
        .await?;
        imported += 1;
    }

    info!(imported, source = source_name, "imported desired-state tracks");
    Ok(imported)
}
