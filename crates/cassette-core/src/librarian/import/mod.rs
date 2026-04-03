pub mod spotify;

use crate::librarian::db::LibrarianDb;
use crate::librarian::error::{LibrarianError, Result};
use crate::librarian::import::spotify::parse_spotify_payload;
use serde_json::Value;
use tracing::info;

pub async fn import_desired_spotify_json(db: &LibrarianDb, json: &str) -> Result<usize> {
    let payload = parse_spotify_payload(json)
        .map_err(|error| LibrarianError::ImportError(error.to_string()))?;

    let source_name = payload
        .source_name
        .as_deref()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or("spotify");

    db.clear_desired_tracks_for_source(source_name).await?;

    let mut imported = 0usize;
    for item in payload.tracks {
        let raw_payload = item
            .raw_payload
            .as_ref()
            .map(Value::to_string)
            .or_else(|| serde_json::to_string(&item).ok());
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::db::LibrarianDb;
    use crate::librarian::models::{DeltaActionType, NewDeltaQueueItem, NewReconciliationResult, ReconciliationStatus};
    use sqlx::sqlite::SqlitePoolOptions;

    async fn test_db() -> LibrarianDb {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("memory db");
        let db = LibrarianDb::from_pool(pool);
        db.migrate().await.expect("migrate");
        db
    }

    #[tokio::test]
    async fn import_replaces_prior_rows_for_same_source() {
        let db = test_db().await;
        import_desired_spotify_json(
            &db,
            r#"{"source_name":"manual","tracks":[{"track_id":"1","artist_name":"A","track_title":"First"}]}"#,
        )
        .await
        .expect("first import");
        let existing = db.list_desired_tracks().await.expect("first desired rows");
        db.insert_reconciliation_result(&NewReconciliationResult {
            desired_track_id: existing[0].id,
            matched_track_id: None,
            matched_local_file_id: None,
            reconciliation_status: ReconciliationStatus::Missing,
            quality_assessment: None,
            reason: "missing".to_string(),
        })
        .await
        .expect("insert reconciliation");
        db.enqueue_delta(&NewDeltaQueueItem {
            desired_track_id: existing[0].id,
            action_type: DeltaActionType::MissingDownload,
            priority: 100,
            reason: "missing".to_string(),
            target_quality: None,
        })
        .await
        .expect("insert delta");
        import_desired_spotify_json(
            &db,
            r#"{"source_name":"manual","tracks":[{"track_id":"2","artist_name":"B","track_title":"Second"}]}"#,
        )
        .await
        .expect("second import");

        let rows = db.list_desired_tracks().await.expect("desired tracks");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].source_track_id.as_deref(), Some("2"));
        assert_eq!(rows[0].track_title, "Second");
    }
}
