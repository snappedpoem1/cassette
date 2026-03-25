use crate::state::AppState;
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
        for mv in &result.moved {
            let _ = db.update_track_path(mv.track_id, &mv.new_path);
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
                    let _ = std::fs::remove_file(path);
                }
            }
        }

        let _ = db.delete_track(*id);
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
            let _ = db.upsert_track(&track);
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
        if let Ok(track) = cassette_core::library::read_track_metadata(std::path::Path::new(path)) {
            let _ = db.upsert_track(&track);
        }
    }

    Ok(ingested)
}
