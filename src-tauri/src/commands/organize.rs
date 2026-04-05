use crate::state::AppState;
use cassette_core::db::TrackPathUpdate;
use cassette_core::library::organizer::{
    self, DuplicateGroup, FileMove,
};
use cassette_core::metadata::{self, TagFix};
use serde::Serialize;
use tauri::State;

// ── Library Organization ─────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct OrganizeReport {
    pub moved: Vec<FileMove>,
    pub skipped: usize,
    pub errors: Vec<String>,
}

fn librarian_db_path() -> Result<std::path::PathBuf, String> {
    let app_data = std::env::var("APPDATA").map_err(|e| e.to_string())?;
    Ok(std::path::PathBuf::from(app_data)
        .join("dev.cassette.app")
        .join("cassette_librarian.db"))
}

#[tauri::command]
pub fn organize_library(
    state: State<'_, AppState>,
    dry_run: Option<bool>,
) -> Result<OrganizeReport, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let library_base = db.get_setting("library_base")
        .map_err(|e| e.to_string())?
        .unwrap_or_else(|| state.download_config.library_base.clone());
    let tracks = db.get_all_tracks_unfiltered().map_err(|e| e.to_string())?;
    drop(db);

    let result = organizer::organize_tracks(&library_base, &tracks, dry_run.unwrap_or(true));

    // If not dry run, update DB paths
    if !dry_run.unwrap_or(true) {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        let sidecar_db_path = librarian_db_path()?;
        let updates = result
            .moved
            .iter()
            .map(|mv| TrackPathUpdate {
                track_id: mv.track_id,
                old_path: mv.old_path.clone(),
                new_path: mv.new_path.clone(),
            })
            .collect::<Vec<_>>();
        if let Err(e) = db.apply_track_path_updates(&sidecar_db_path, &updates) {
            tracing::warn!("[organize] failed to converge app and sidecar paths: {e}");
        }
    }

    Ok(OrganizeReport {
        moved: result.moved,
        skipped: result.skipped.len(),
        errors: result.errors,
    })
}

// ── Duplicate Detection ──────────────────────────────────────────────────────

#[tauri::command]
pub fn find_duplicates(state: State<'_, AppState>) -> Result<Vec<DuplicateGroup>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let tracks = db.get_all_tracks_unfiltered().map_err(|e| e.to_string())?;
    Ok(organizer::find_duplicates(&tracks))
}

#[tauri::command]
pub fn resolve_duplicate(
    state: State<'_, AppState>,
    keep_track_id: i64,
    remove_track_ids: Vec<i64>,
    delete_files: Option<bool>,
) -> Result<usize, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let mut removed = 0;

    for id in &remove_track_ids {
        if *id == keep_track_id { continue; }

        if delete_files.unwrap_or(false) {
            if let Ok(Some(track)) = db.get_track_by_id(*id) {
                let path = std::path::Path::new(&track.path);
                if path.exists() {
                    std::fs::remove_file(path)
                        .map_err(|e| format!("failed to delete file {}: {e}", path.display()))?;
                }
            }
        }

        db.delete_track(*id)
            .map_err(|e| format!("failed to delete track {id} from DB: {e}"))?;
        removed += 1;
    }

    Ok(removed)
}

// ── Prune Missing ────────────────────────────────────────────────────────────

#[tauri::command]
pub fn prune_missing_tracks(state: State<'_, AppState>) -> Result<usize, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.prune_missing_tracks().map_err(|e| e.to_string())
}

// ── Metadata Fixer (MusicBrainz) ─────────────────────────────────────────────

#[tauri::command]
pub async fn propose_tag_fixes(
    state: State<'_, AppState>,
    artist: String,
    album: String,
) -> Result<Vec<TagFix>, String> {
    let tracks = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db.get_album_tracks(&artist, &album).map_err(|e| e.to_string())?
    };

    let svc = metadata::MetadataService::new().map_err(|e| e.to_string())?;
    svc.propose_tag_fixes(&artist, &album, &tracks)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn apply_tag_fixes(
    state: State<'_, AppState>,
    fixes: Vec<TagFix>,
) -> Result<usize, String> {
    let mut applied = 0;
    for fix in &fixes {
        match metadata::apply_tag_fix(fix) {
            Ok(()) => applied += 1,
            Err(e) => eprintln!("[tag-fix] Failed to apply fix to {}: {e}", fix.path),
        }
    }

    // Re-scan the affected files to update DB
    let db = state.db.lock().map_err(|e| e.to_string())?;
    for fix in &fixes {
        if let Ok(track) = cassette_core::library::read_track_metadata(std::path::Path::new(&fix.path)) {
            if let Err(e) = db.upsert_track(&track) {
                tracing::warn!("[tag-fix] failed to upsert track after tag fix {}: {e}", fix.path);
            }
        }
    }

    Ok(applied)
}

// ── Staging Ingest ───────────────────────────────────────────────────────────

#[tauri::command]
pub fn ingest_staging(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let staging = db.get_setting("staging_folder")
        .map_err(|e| e.to_string())?
        .unwrap_or_else(|| state.download_config.staging_folder.clone());
    let library_base = db.get_setting("library_base")
        .map_err(|e| e.to_string())?
        .unwrap_or_else(|| state.download_config.library_base.clone());
    drop(db);

    let ingested = organizer::ingest_staging(&staging, &library_base)
        .map_err(|e| e.to_string())?;

    // Scan ingested files into DB
    let db = state.db.lock().map_err(|e| e.to_string())?;
    for path in &ingested {
        match cassette_core::library::read_track_metadata(std::path::Path::new(path)) {
            Ok(track) => {
                if let Err(e) = db.upsert_track(&track) {
                    tracing::warn!("[ingest_staging] failed to upsert {path}: {e}");
                }
            }
            Err(e) => tracing::warn!("[ingest_staging] failed to read metadata from {path}: {e}"),
        }
    }

    Ok(ingested)
}
