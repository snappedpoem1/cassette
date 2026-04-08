use super::planner;
use crate::state::AppState;
use cassette_core::acquisition::{
    AcquisitionRequest, AcquisitionRequestStatus, AcquisitionScope, ConfirmationPolicy,
};
use cassette_core::db::DeadLetterSummary;
use cassette_core::director::models::{TrackTask, TrackTaskSource};
use tauri::State;

#[tauri::command]
pub async fn get_dead_letter_summary(
    state: State<'_, AppState>,
    recent_limit: Option<usize>,
) -> Result<DeadLetterSummary, String> {
    let limit = recent_limit.unwrap_or(5);
    let db = state.db.lock().map_err(|error| error.to_string())?;
    db.get_dead_letter_summary(limit)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn replay_dead_letter(
    state: State<'_, AppState>,
    task_id: String,
) -> Result<i64, String> {
    let request_json = {
        let db = state.db.lock().map_err(|error| error.to_string())?;
        db.get_task_request_json(&task_id)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| format!("no request_json for task_id {task_id}"))?
    };

    let task: TrackTask =
        serde_json::from_str(&request_json).map_err(|error| format!("invalid request_json: {error}"))?;
    let mut request = request_from_task(&task);

    request.status = AcquisitionRequestStatus::Pending;
    request.task_id = None;
    request.request_signature = None;
    request.raw_payload_json = Some(
        serde_json::json!({
            "replayed_from": task_id,
            "replay_reason": "dead_letter_replay",
            "original_task": task,
        })
        .to_string(),
    );

    let planned = planner::plan_acquisition(state.clone(), request).await?;
    let approved = planner::approve_planned_request(
        state,
        planned.request.id,
        Some("dead_letter_replay approve_low_trust".to_string()),
        None,
    )
    .await?;

    Ok(approved.id)
}

fn request_source_name(source: &TrackTaskSource) -> &'static str {
    match source {
        TrackTaskSource::SpotifyLibrary => "spotify_library",
        TrackTaskSource::SpotifyHistory => "spotify_history",
        TrackTaskSource::SpotifyPlaylist { .. } => "spotify_playlist",
        TrackTaskSource::Manual => "manual",
    }
}

fn request_from_task(task: &TrackTask) -> AcquisitionRequest {
    AcquisitionRequest {
        id: None,
        scope: AcquisitionScope::Track,
        source: task.source.clone(),
        source_name: request_source_name(&task.source).to_string(),
        source_track_id: task.target.spotify_track_id.clone(),
        source_album_id: task
            .target
            .source_album_id
            .clone()
            .or_else(|| task.target.musicbrainz_release_id.clone()),
        source_artist_id: task.target.source_artist_id.clone(),
        artist: task.target.artist.clone(),
        album: task.target.album.clone(),
        title: task.target.title.clone(),
        track_number: task.target.track_number,
        disc_number: task.target.disc_number,
        year: task.target.year,
        duration_secs: task.target.duration_secs,
        isrc: task.target.isrc.clone(),
        musicbrainz_recording_id: task.target.musicbrainz_recording_id.clone(),
        musicbrainz_release_group_id: task.target.musicbrainz_release_group_id.clone(),
        musicbrainz_release_id: task.target.musicbrainz_release_id.clone(),
        canonical_artist_id: task.target.canonical_artist_id,
        canonical_release_id: task.target.canonical_release_id,
        strategy: task.strategy,
        quality_policy: None,
        excluded_providers: Vec::new(),
        edition_policy: None,
        confirmation_policy: ConfirmationPolicy::Automatic,
        desired_track_id: task.desired_track_id,
        source_operation_id: task.source_operation_id.clone(),
        task_id: Some(task.task_id.clone()),
        request_signature: Some(cassette_core::db::director_request_signature(task)),
        status: AcquisitionRequestStatus::Pending,
        raw_payload_json: Some(serde_json::to_string(task).unwrap_or_default()),
    }
}
