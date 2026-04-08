use crate::state::{build_runtime_provider_stack, AppState};
use cassette_core::acquisition::{AcquisitionRequest, AcquisitionRequestStatus};
use cassette_core::db::{
    CandidateReviewItem, StoredCandidateSetSummary, StoredIdentityResolutionEvidence,
    StoredProviderMemory, StoredProviderResponseCache, StoredProviderSearchRecord,
    StoredSourceAlias,
};
use cassette_core::director::error::ProviderError;
use cassette_core::director::models::{
    AcquisitionStrategy, CandidateRecord, NormalizedTrack, ProviderSearchCandidate,
    ProviderSearchRecord, TrackTask, TrackTaskSource,
};
use cassette_core::director::provider::Provider;
use cassette_core::director::strategy::{StrategyPlan, StrategyPlanner};
use cassette_core::director::DirectorProgress;
use cassette_core::librarian::models::AcquisitionRequestRow;
use chrono::{NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::State;

const EDITION_MARKERS: &[&str] = &[
    "deluxe",
    "expanded",
    "anniversary",
    "collector",
    "special edition",
    "bonus",
    "remaster",
    "live",
];
const PLANNER_EXCLUSION_MEMORY_PREFIX: &str = "planner.exclusion_memory.v1";
const PLANNER_IDENTITY_ENVELOPE_PREFIX: &str = "planner.identity_envelope.v1";
const LOW_TRUST_APPROVAL_TOKEN: &str = "approve_low_trust";
const LOW_TRUST_PROVIDER_IDS: &[&str] = &["yt_dlp", "real_debrid", "tpb_apibay"];
const PREFLIGHT_REASON_MISSING_CANDIDATE_SET: &str = "missing_candidate_set";
const PREFLIGHT_REASON_NO_CANDIDATES: &str = "no_candidates";
const PREFLIGHT_REASON_NO_PROVIDER_SEARCH_RECORDS: &str = "no_provider_search_records";
const PREFLIGHT_REASON_NO_SELECTED_CANDIDATE: &str = "no_selected_candidate";
const PREFLIGHT_VALIDATION_ERROR: &str = "validation_error:preflight_failed";

#[derive(Debug, Serialize)]
pub struct PlannedAcquisitionResult {
    pub request: AcquisitionRequestRow,
    pub identity_lane: PlannerIdentityLane,
    pub edition: Option<EditionContext>,
    pub provider_order: Vec<String>,
    pub cached_provider_ids: Vec<String>,
    pub summary: Option<StoredCandidateSetSummary>,
    pub provider_searches: Vec<StoredProviderSearchRecord>,
    pub candidate_review: Vec<CandidateReviewItem>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PlannerIdentityLane {
    pub scope: String,
    pub musicbrainz_release_group_id: Option<String>,
    pub musicbrainz_release_id: Option<String>,
    pub musicbrainz_recording_id: Option<String>,
    pub canonical_artist_id: Option<i64>,
    pub canonical_release_id: Option<i64>,
    pub quality_policy: Option<String>,
    pub edition_policy: Option<String>,
    pub confirmation_policy: String,
}

#[derive(Debug, Serialize)]
pub struct RequestRationale {
    pub request: AcquisitionRequestRow,
    pub identity_lane: PlannerIdentityLane,
    pub identity_confidence: String,
    pub edition_match_outcome: String,
    pub candidate_count_considered: usize,
    pub edition: Option<EditionContext>,
    pub timeline: Vec<cassette_core::librarian::models::AcquisitionRequestEvent>,
    pub candidate_set: Option<StoredCandidateSetSummary>,
    pub provider_searches: Vec<StoredProviderSearchRecord>,
    pub candidate_review: Vec<CandidateReviewItem>,
    pub provider_memory: Vec<StoredProviderMemory>,
    pub provider_response_cache: Vec<StoredProviderResponseCache>,
    pub identity_resolution_evidence: Vec<StoredIdentityResolutionEvidence>,
    pub source_aliases: Vec<StoredSourceAlias>,
    pub preflight: ReviewPreflightResult,
    pub execution: Option<cassette_core::db::TaskExecutionSummary>,
    pub provenance: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ReviewContract {
    pub request: AcquisitionRequestRow,
    pub identity_lane: PlannerIdentityLane,
    pub edition: Option<EditionContext>,
    pub candidate_set: Option<StoredCandidateSetSummary>,
    pub provider_searches: Vec<StoredProviderSearchRecord>,
    pub candidate_review: Vec<CandidateReviewItem>,
    pub preflight: ReviewPreflightResult,
    pub approval: ReviewApprovalPolicy,
}

#[derive(Debug, Serialize)]
pub struct ReviewApprovalPolicy {
    pub required: bool,
    pub token: Option<String>,
    pub low_trust_selected_providers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewPreflightResult {
    pub passed: bool,
    pub checked_at: String,
    pub reason_codes: Vec<String>,
    pub selected_candidate_count: usize,
    pub provider_search_count: usize,
    pub provider_success_count: usize,
    pub candidate_count: usize,
}

#[derive(Debug, Deserialize)]
struct PersistedProviderResponseEnvelope {
    #[serde(default)]
    candidate_records: Vec<CandidateRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EditionContext {
    pub policy: Option<String>,
    pub markers: EditionMarkers,
    pub evidence: EditionEvidence,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EditionMarkers {
    pub is_live: bool,
    pub is_deluxe: bool,
    pub is_remaster: bool,
    pub country: Option<String>,
    pub label: Option<String>,
    pub catalog_number: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EditionEvidence {
    pub source: String,
    pub confidence: String,
}

#[derive(Debug, Clone, Deserialize)]
struct SelectedAlbumsPayload {
    targets: SelectedAlbumTargets,
}

#[derive(Debug, Clone, Deserialize)]
struct SelectedAlbumTargets {
    #[serde(default)]
    include: Vec<AlbumTarget>,
    #[serde(default)]
    exclude: Vec<AlbumTarget>,
}

#[derive(Debug, Clone, Deserialize)]
struct AlbumTarget {
    #[serde(default)]
    artist: Option<String>,
    #[serde(default)]
    album: Option<String>,
    #[serde(default)]
    release_group_id: Option<String>,
}

#[tauri::command]
pub async fn plan_acquisition(
    state: State<'_, AppState>,
    mut request: AcquisitionRequest,
) -> Result<PlannedAcquisitionResult, String> {
    augment_request_payload_with_edition_context(&mut request)?;
    augment_request_payload_with_identity_envelope(&mut request)?;
    validate_selected_albums_grammar(&request)?;
    apply_exclusion_memory(&state, &mut request)?;

    if request.request_signature.is_none() {
        request.request_signature = Some(planner_identity_envelope_from_request(&request));
    }
    if request.task_id.is_none() {
        request.task_id = Some(request.effective_task_id());
    }
    request.status = AcquisitionRequestStatus::Pending;

    let request_signature = request
        .request_signature
        .clone()
        .ok_or_else(|| "missing request signature".to_string())?;
    let task = request.to_track_task();

    let row = match state
        .control_db
        .get_acquisition_request_by_signature(&request_signature)
        .await
        .map_err(|error| error.to_string())?
    {
        Some(existing) => {
            state
                .control_db
                .append_acquisition_request_event(
                    existing.id,
                    request.task_id.as_deref(),
                    "planning_requested",
                    existing.status.as_str(),
                    Some("planner refresh requested"),
                    request.raw_payload_json.as_deref(),
                )
                .await
                .map_err(|error| error.to_string())?;
            existing
        }
        None => state
            .control_db
            .create_acquisition_request(&request)
            .await
            .map_err(|error| error.to_string())?,
    };

    let (config, providers, cached_rows) = {
        let db = state.db.lock().map_err(|error| error.to_string())?;
        let (config, providers) = build_runtime_provider_stack(&db, &state.download_config, None);
        let cached_rows = db
            .get_provider_response_cache(&request_signature)
            .map_err(|error| error.to_string())?;
        (config, providers, cached_rows)
    };

    let (plan, provider_searches, candidate_records, cached_provider_ids) =
        search_planner_candidates(
            &config,
            &providers,
            &task,
            &cached_rows,
            request.edition_policy.as_deref(),
        )
        .await?;

    {
        let db = state.db.lock().map_err(|error| error.to_string())?;
        db.record_request_identity_snapshot(&task, &request_signature)
            .map_err(|error| error.to_string())?;
        db.record_request_source_aliases(&request, &request_signature)
            .map_err(|error| error.to_string())?;
        db.persist_planned_candidate_set(
            &task,
            request.strategy_name(),
            &provider_searches,
            &candidate_records,
        )
        .map_err(|error| error.to_string())?;
    }

    state
        .control_db
        .update_acquisition_request_status_by_task_id(
            &task.task_id,
            AcquisitionRequestStatus::Reviewing.as_str(),
            "planning_completed",
            Some("planner candidate set persisted"),
            Some(
                &serde_json::json!({
                    "provider_order": plan.provider_order,
                    "cached_provider_ids": cached_provider_ids,
                    "candidate_count": candidate_records.len(),
                    "provider_count": provider_searches.len(),
                    "identity_lane": planner_identity_lane_from_request(&request),
                })
                .to_string(),
            ),
        )
        .await
        .map_err(|error| error.to_string())?;

    let preflight = {
        let db = state.db.lock().map_err(|error| error.to_string())?;
        let candidate_set = db
            .get_candidate_set_summary(&task.task_id)
            .map_err(|error| error.to_string())?;
        let provider_searches = db
            .get_provider_search_records(&task.task_id)
            .map_err(|error| error.to_string())?;
        let candidate_review = db
            .get_candidate_review(&task.task_id)
            .map_err(|error| error.to_string())?;
        build_review_preflight_result(&candidate_set, &provider_searches, &candidate_review)
    };
    let preflight_payload = serde_json::to_string(&preflight).map_err(|error| error.to_string())?;
    state
        .control_db
        .append_acquisition_request_event(
            row.id,
            request.task_id.as_deref(),
            "review_preflight",
            AcquisitionRequestStatus::Reviewing.as_str(),
            Some("review preflight evaluated"),
            Some(preflight_payload.as_str()),
        )
        .await
        .map_err(|error| error.to_string())?;

    load_planned_acquisition_result(&state, row.id, plan.provider_order, cached_provider_ids).await
}

#[tauri::command]
pub async fn get_candidate_set(
    state: State<'_, AppState>,
    request_id: i64,
) -> Result<serde_json::Value, String> {
    let request = state
        .control_db
        .get_acquisition_request(request_id)
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("request {request_id} not found"))?;
    let edition = edition_context_from_payload(request.raw_payload_json.as_deref());
    let Some(task_id) = request.task_id.as_deref() else {
        return Ok(serde_json::json!({
            "request": request,
            "edition": edition,
            "preflight": null,
            "candidate_set": null,
            "provider_searches": [],
            "candidate_review": [],
        }));
    };

    let db = state.db.lock().map_err(|error| error.to_string())?;
    let candidate_set = db
        .get_candidate_set_summary(task_id)
        .map_err(|error| error.to_string())?;
    let provider_searches = db
        .get_provider_search_records(task_id)
        .map_err(|error| error.to_string())?;
    let candidate_review = db
        .get_candidate_review(task_id)
        .map_err(|error| error.to_string())?;
    let preflight = build_review_preflight_result(&candidate_set, &provider_searches, &candidate_review);

    Ok(serde_json::json!({
        "request": request,
        "edition": edition,
        "preflight": preflight,
        "candidate_set": candidate_set,
        "provider_searches": provider_searches,
        "candidate_review": candidate_review,
    }))
}

#[tauri::command]
pub async fn get_request_rationale(
    state: State<'_, AppState>,
    request_id: i64,
) -> Result<RequestRationale, String> {
    let request = state
        .control_db
        .get_acquisition_request(request_id)
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("request {request_id} not found"))?;
    let timeline = state
        .control_db
        .get_acquisition_request_timeline(request_id)
        .await
        .map_err(|error| error.to_string())?;

    let db = state.db.lock().map_err(|error| error.to_string())?;
    let (candidate_set, provider_searches, candidate_review, execution, provenance) =
        if let Some(task_id) = request.task_id.as_deref() {
            (
                db.get_candidate_set_summary(task_id)
                    .map_err(|error| error.to_string())?,
                db.get_provider_search_records(task_id)
                    .map_err(|error| error.to_string())?,
                db.get_candidate_review(task_id)
                    .map_err(|error| error.to_string())?,
                db.get_task_execution_summary(task_id)
                    .map_err(|error| error.to_string())?,
                db.get_task_provenance(task_id)
                    .map_err(|error| error.to_string())?,
            )
        } else {
            (None, Vec::new(), Vec::new(), None, None)
        };

    let provider_memory = db
        .get_director_provider_memory(&request.request_signature)
        .map_err(|error| error.to_string())?;
    let provider_response_cache = db
        .get_provider_response_cache(&request.request_signature)
        .map_err(|error| error.to_string())?;
    let identity_resolution_evidence = db
        .get_identity_resolution_evidence_for_request(&request.request_signature)
        .map_err(|error| error.to_string())?;
    let source_aliases = db
        .get_source_aliases_for_entity("request_signature", &request.request_signature)
        .map_err(|error| error.to_string())?;
    let rationale_preflight = latest_preflight_from_timeline(&timeline).unwrap_or_else(|| {
        build_review_preflight_result(&candidate_set, &provider_searches, &candidate_review)
    });

    Ok(RequestRationale {
        identity_lane: planner_identity_lane_from_row(&request),
        identity_confidence: derive_identity_confidence(&request),
        edition_match_outcome: derive_edition_match_outcome(
            request.edition_policy.as_deref(),
            &candidate_set,
            &provider_searches,
        ),
        candidate_count_considered: candidate_set
            .as_ref()
            .map(|summary| summary.candidate_count)
            .unwrap_or(0),
        edition: edition_context_from_payload(request.raw_payload_json.as_deref()),
        request,
        timeline,
        candidate_set,
        provider_searches,
        candidate_review,
        provider_memory,
        provider_response_cache,
        identity_resolution_evidence,
        source_aliases,
        preflight: rationale_preflight,
        execution,
        provenance,
    })
}

#[tauri::command]
pub async fn get_review_contract(
    state: State<'_, AppState>,
    request_id: i64,
) -> Result<ReviewContract, String> {
    let request = state
        .control_db
        .get_acquisition_request(request_id)
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("request {request_id} not found"))?;

    let (candidate_set, provider_searches, candidate_review) =
        if let Some(task_id) = request.task_id.as_deref() {
            let db = state.db.lock().map_err(|error| error.to_string())?;
            (
                db.get_candidate_set_summary(task_id)
                    .map_err(|error| error.to_string())?,
                db.get_provider_search_records(task_id)
                    .map_err(|error| error.to_string())?,
                db.get_candidate_review(task_id)
                    .map_err(|error| error.to_string())?,
            )
        } else {
            (None, Vec::new(), Vec::new())
        };

    let low_trust_selected_providers = selected_low_trust_provider_ids(&candidate_review);
    let timeline = state
        .control_db
        .get_acquisition_request_timeline(request_id)
        .await
        .map_err(|error| error.to_string())?;
    let preflight = latest_preflight_from_timeline(&timeline).unwrap_or_else(|| {
        build_review_preflight_result(&candidate_set, &provider_searches, &candidate_review)
    });

    Ok(ReviewContract {
        identity_lane: planner_identity_lane_from_row(&request),
        edition: edition_context_from_payload(request.raw_payload_json.as_deref()),
        request,
        candidate_set,
        provider_searches,
        candidate_review,
        preflight,
        approval: ReviewApprovalPolicy {
            required: !low_trust_selected_providers.is_empty(),
            token: if low_trust_selected_providers.is_empty() {
                None
            } else {
                Some(LOW_TRUST_APPROVAL_TOKEN.to_string())
            },
            low_trust_selected_providers,
        },
    })
}

#[tauri::command]
pub async fn approve_planned_request(
    state: State<'_, AppState>,
    request_id: i64,
    note: Option<String>,
    excluded_provider_ids: Option<Vec<String>>,
) -> Result<AcquisitionRequestRow, String> {
    let request = state
        .control_db
        .get_acquisition_request(request_id)
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("request {request_id} not found"))?;
    let task_id = request
        .task_id
        .clone()
        .ok_or_else(|| format!("request {request_id} is missing task_id"))?;

    if request.status != AcquisitionRequestStatus::Reviewing.as_str() {
        return Err(format!(
            "request {request_id} has status '{}' (expected '{}')",
            request.status,
            AcquisitionRequestStatus::Reviewing.as_str()
        ));
    }

    let (candidate_set, provider_searches, review) = {
        let db = state.db.lock().map_err(|error| error.to_string())?;
        (
            db.get_candidate_set_summary(&task_id)
                .map_err(|error| error.to_string())?,
            db.get_provider_search_records(&task_id)
                .map_err(|error| error.to_string())?,
            db.get_candidate_review(&task_id)
                .map_err(|error| error.to_string())?,
        )
    };
    let preflight = build_review_preflight_result(&candidate_set, &provider_searches, &review);
    let preflight_payload = serde_json::to_string(&preflight).map_err(|error| error.to_string())?;
    state
        .control_db
        .append_acquisition_request_event(
            request.id,
            Some(&task_id),
            "review_preflight",
            request.status.as_str(),
            Some("review preflight evaluated before approval"),
            Some(preflight_payload.as_str()),
        )
        .await
        .map_err(|error| error.to_string())?;

    if !preflight.passed {
        let reasons = if preflight.reason_codes.is_empty() {
            "unknown".to_string()
        } else {
            preflight.reason_codes.join(",")
        };
        return Err(format!(
            "{PREFLIGHT_VALIDATION_ERROR}: approval blocked; reason_codes=[{reasons}]"
        ));
    }

    let selected_low_trust = {
        selected_low_trust_provider_ids(&review)
    };

    validate_low_trust_approval(note.as_deref(), &selected_low_trust)?;

    let remembered = remember_exclusions_for_request(
        &state,
        &request,
        &task_id,
        excluded_provider_ids.as_deref(),
    )?;
    let memory_key = planner_exclusion_memory_key_from_row(&request);

    let task = track_task_from_request_row(&request)?;

    let payload = serde_json::json!({
        "request_id": request_id,
        "review_action": "approved",
        "remembered_excluded_providers": remembered,
        "memory_key": memory_key,
        "low_trust_selected_providers": selected_low_trust,
        "low_trust_approval_artifact": has_low_trust_approval_artifact(note.as_deref()),
    })
    .to_string();
    state
        .control_db
        .update_acquisition_request_status_by_task_id(
            &task_id,
            AcquisitionRequestStatus::Queued.as_str(),
            "review_approved",
            note.as_deref().or(Some("planner review approved")),
            Some(payload.as_str()),
        )
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("request {request_id} not found for task {task_id}"))?;

    state
        .persist_pending_task(&task, DirectorProgress::Queued)
        .map_err(|error| error.to_string())?;
    let task_payload = serde_json::to_string(&task).ok();
    let _ = state
        .control_db
        .update_acquisition_request_status_by_task_id(
            &task_id,
            AcquisitionRequestStatus::Queued.as_str(),
            "runtime_queued",
            Some("queued for director submission"),
            task_payload.as_deref(),
        )
        .await;

    match state.submit_director_task(task).await {
        Ok(()) => state
            .control_db
            .update_acquisition_request_status_by_task_id(
                &task_id,
                AcquisitionRequestStatus::Submitted.as_str(),
                "director_submitted",
                Some("submitted to director"),
                None,
            )
            .await
            .map_err(|error| error.to_string())?
            .ok_or_else(|| format!("request {request_id} not found for task {task_id}")),
        Err(error) => {
            let _ = state.delete_pending_task(&task_id);
            let _ = state
                .control_db
                .update_acquisition_request_status_by_task_id(
                    &task_id,
                    AcquisitionRequestStatus::Failed.as_str(),
                    "director_submit_failed",
                    Some(&error.to_string()),
                    None,
                )
                .await;
            Err(error.to_string())
        }
    }
}

#[tauri::command]
pub async fn reject_planned_request(
    state: State<'_, AppState>,
    request_id: i64,
    reason: Option<String>,
    excluded_provider_ids: Option<Vec<String>>,
) -> Result<AcquisitionRequestRow, String> {
    let request = state
        .control_db
        .get_acquisition_request(request_id)
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("request {request_id} not found"))?;
    let task_id = request
        .task_id
        .clone()
        .ok_or_else(|| format!("request {request_id} is missing task_id"))?;

    if request.status != AcquisitionRequestStatus::Reviewing.as_str() {
        return Err(format!(
            "request {request_id} has status '{}' (expected '{}')",
            request.status,
            AcquisitionRequestStatus::Reviewing.as_str()
        ));
    }

    let remembered = remember_exclusions_for_request(
        &state,
        &request,
        &task_id,
        excluded_provider_ids.as_deref(),
    )?;
    let memory_key = planner_exclusion_memory_key_from_row(&request);

    let payload = serde_json::json!({
        "request_id": request_id,
        "review_action": "rejected",
        "remembered_excluded_providers": remembered,
        "memory_key": memory_key,
    })
    .to_string();
    let updated = state
        .control_db
        .update_acquisition_request_status_by_task_id(
            &task_id,
            AcquisitionRequestStatus::Cancelled.as_str(),
            "review_rejected",
            reason.as_deref().or(Some("planner review rejected")),
            Some(payload.as_str()),
        )
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("request {request_id} not found for task {task_id}"))?;

    Ok(updated)
}

fn track_task_from_request_row(request: &AcquisitionRequestRow) -> Result<TrackTask, String> {
    let task_id = request
        .task_id
        .clone()
        .ok_or_else(|| format!("request {} is missing task_id", request.id))?;
    let source = parse_track_task_source(&request.source_name)?;
    let strategy = parse_acquisition_strategy(&request.strategy)?;

    let track_number = request
        .track_number
        .and_then(|value| u32::try_from(value).ok());
    let disc_number = request
        .disc_number
        .and_then(|value| u32::try_from(value).ok());
    let year = request.year.and_then(|value| i32::try_from(value).ok());

    Ok(TrackTask {
        task_id,
        source,
        desired_track_id: request.desired_track_id,
        source_operation_id: request.source_operation_id.clone(),
        target: NormalizedTrack {
            spotify_track_id: request.source_track_id.clone(),
            source_album_id: request.source_album_id.clone(),
            source_artist_id: request.source_artist_id.clone(),
            source_playlist: None,
            artist: request.artist.clone(),
            album_artist: Some(request.artist.clone()),
            title: request.title.clone(),
            album: request.album.clone(),
            track_number,
            disc_number,
            year,
            duration_secs: request.duration_secs,
            isrc: request.isrc.clone(),
            musicbrainz_recording_id: request.musicbrainz_recording_id.clone(),
            musicbrainz_release_group_id: request.musicbrainz_release_group_id.clone(),
            musicbrainz_release_id: request.musicbrainz_release_id.clone(),
            canonical_artist_id: request.canonical_artist_id,
            canonical_release_id: request.canonical_release_id,
        },
        strategy,
    })
}

fn apply_exclusion_memory(
    state: &State<'_, AppState>,
    request: &mut AcquisitionRequest,
) -> Result<(), String> {
    let key = planner_exclusion_memory_key_from_request(request);
    let remembered = {
        let db = state.db.lock().map_err(|error| error.to_string())?;
        db.get_setting(&key)
            .map_err(|error| error.to_string())?
            .map(|value| decode_provider_list(&value))
            .unwrap_or_default()
    };

    if remembered.is_empty() {
        return Ok(());
    }

    request.excluded_providers = merge_excluded_providers(&request.excluded_providers, &remembered);
    Ok(())
}

fn remember_exclusions_for_request(
    state: &State<'_, AppState>,
    request: &AcquisitionRequestRow,
    task_id: &str,
    explicit_provider_ids: Option<&[String]>,
) -> Result<Vec<String>, String> {
    let remembered = {
        let db = state.db.lock().map_err(|error| error.to_string())?;
        let existing = request
            .excluded_providers_json
            .as_deref()
            .map(decode_provider_list)
            .unwrap_or_default();
        let reviewed = if let Some(explicit) = explicit_provider_ids {
            explicit.to_vec()
        } else {
            db.get_candidate_review(task_id)
                .map_err(|error| error.to_string())?
                .into_iter()
                .filter(|item| !item.is_selected)
                .map(|item| item.provider_id)
                .collect::<Vec<_>>()
        };

        let merged = merge_excluded_providers(&existing, &reviewed);
        if !merged.is_empty() {
            let payload = serde_json::to_string(&merged).map_err(|error| error.to_string())?;
            db.set_setting(&planner_exclusion_memory_key_from_row(request), &payload)
                .map_err(|error| error.to_string())?;
        }
        merged
    };

    Ok(remembered)
}

fn planner_exclusion_memory_key_from_request(request: &AcquisitionRequest) -> String {
    planner_exclusion_memory_key(
        planner_identity_envelope_from_request(request),
        request.quality_policy.as_deref(),
        request.edition_policy.as_deref(),
        Some(request.confirmation_policy.as_str()),
    )
}

fn planner_exclusion_memory_key_from_row(request: &AcquisitionRequestRow) -> String {
    planner_exclusion_memory_key(
        planner_identity_envelope_from_row(request),
        request.quality_policy.as_deref(),
        request.edition_policy.as_deref(),
        Some(request.confirmation_policy.as_str()),
    )
}

fn planner_exclusion_memory_key(
    identity_envelope_key: String,
    quality_policy: Option<&str>,
    edition_policy: Option<&str>,
    confirmation_policy: Option<&str>,
) -> String {
    [
        PLANNER_EXCLUSION_MEMORY_PREFIX.to_string(),
        identity_envelope_key,
        normalize_provider_value(quality_policy),
        normalize_provider_value(edition_policy),
        normalize_provider_value(confirmation_policy),
    ]
    .join("::")
}

fn normalize_provider_value(value: Option<&str>) -> String {
    value
        .map(|segment| segment.trim().to_ascii_lowercase())
        .filter(|segment| !segment.is_empty())
        .unwrap_or_default()
}

fn decode_provider_list(value: &str) -> Vec<String> {
    let parsed = serde_json::from_str::<Vec<String>>(value).unwrap_or_default();
    normalize_provider_ids(&parsed)
}

fn normalize_provider_ids(values: &[String]) -> Vec<String> {
    let mut normalized = Vec::new();
    for value in values {
        let candidate = value.trim().to_ascii_lowercase();
        if candidate.is_empty() || normalized.contains(&candidate) {
            continue;
        }
        normalized.push(candidate);
    }
    normalized
}

fn merge_excluded_providers(existing: &[String], remembered: &[String]) -> Vec<String> {
    let mut merged = normalize_provider_ids(existing);
    for provider in normalize_provider_ids(remembered) {
        if !merged.contains(&provider) {
            merged.push(provider);
        }
    }
    merged
}

fn selected_low_trust_provider_ids(candidate_review: &[CandidateReviewItem]) -> Vec<String> {
    let mut selected = Vec::new();
    for item in candidate_review {
        if !item.is_selected {
            continue;
        }

        let provider_id = item.provider_id.trim().to_ascii_lowercase();
        if provider_id.is_empty() || selected.contains(&provider_id) {
            continue;
        }

        if LOW_TRUST_PROVIDER_IDS
            .iter()
            .any(|known| provider_id == *known)
        {
            selected.push(provider_id);
        }
    }
    selected
}

fn has_low_trust_approval_artifact(note: Option<&str>) -> bool {
    note.map(str::to_ascii_lowercase)
        .map(|value| value.contains(LOW_TRUST_APPROVAL_TOKEN))
        .unwrap_or(false)
}

fn validate_low_trust_approval(
    note: Option<&str>,
    selected_low_trust: &[String],
) -> Result<(), String> {
    if selected_low_trust.is_empty() || has_low_trust_approval_artifact(note) {
        return Ok(());
    }

    Err(format!(
        "validation_error:low_trust_approval_required: explicit '{}' token required for providers [{}]",
        LOW_TRUST_APPROVAL_TOKEN,
        selected_low_trust.join(",")
    ))
}

fn latest_preflight_from_timeline(
    timeline: &[cassette_core::librarian::models::AcquisitionRequestEvent],
) -> Option<ReviewPreflightResult> {
    timeline.iter().rev().find_map(|event| {
        if event.event_type != "review_preflight" {
            return None;
        }
        event
            .payload_json
            .as_deref()
            .and_then(|payload| serde_json::from_str::<ReviewPreflightResult>(payload).ok())
    })
}

fn build_review_preflight_result(
    candidate_set: &Option<StoredCandidateSetSummary>,
    provider_searches: &[StoredProviderSearchRecord],
    candidate_review: &[CandidateReviewItem],
) -> ReviewPreflightResult {
    let selected_candidate_count = candidate_review.iter().filter(|item| item.is_selected).count();
    let provider_success_count = provider_searches
        .iter()
        .filter(|entry| entry.candidate_count > 0)
        .count();
    let candidate_count = candidate_set
        .as_ref()
        .map(|summary| summary.candidate_count)
        .unwrap_or(0);

    let mut reason_codes = Vec::new();
    if candidate_set.is_none() {
        reason_codes.push(PREFLIGHT_REASON_MISSING_CANDIDATE_SET.to_string());
    }
    if candidate_count == 0 {
        reason_codes.push(PREFLIGHT_REASON_NO_CANDIDATES.to_string());
    }
    if provider_searches.is_empty() {
        reason_codes.push(PREFLIGHT_REASON_NO_PROVIDER_SEARCH_RECORDS.to_string());
    }
    if selected_candidate_count == 0 {
        reason_codes.push(PREFLIGHT_REASON_NO_SELECTED_CANDIDATE.to_string());
    }

    ReviewPreflightResult {
        passed: reason_codes.is_empty(),
        checked_at: Utc::now().to_rfc3339(),
        reason_codes,
        selected_candidate_count,
        provider_search_count: provider_searches.len(),
        provider_success_count,
        candidate_count,
    }
}

fn parse_track_task_source(source_name: &str) -> Result<TrackTaskSource, String> {
    match source_name {
        "manual" => Ok(TrackTaskSource::Manual),
        "spotify_library" => Ok(TrackTaskSource::SpotifyLibrary),
        "spotify_history" => Ok(TrackTaskSource::SpotifyHistory),
        "spotify_playlist" => Ok(TrackTaskSource::SpotifyPlaylist {
            playlist_id: "unknown".to_string(),
        }),
        _ => Err(format!("unsupported request source_name '{source_name}'")),
    }
}

fn parse_acquisition_strategy(strategy: &str) -> Result<AcquisitionStrategy, String> {
    match strategy {
        "standard" => Ok(AcquisitionStrategy::Standard),
        "high_quality_only" => Ok(AcquisitionStrategy::HighQualityOnly),
        "obscure_fallback_heavy" => Ok(AcquisitionStrategy::ObscureFallbackHeavy),
        "discography_batch" => Ok(AcquisitionStrategy::DiscographyBatch),
        "single_track_priority" => Ok(AcquisitionStrategy::SingleTrackPriority),
        "metadata_repair_only" => Ok(AcquisitionStrategy::MetadataRepairOnly),
        "redownload_replace_if_better" => Ok(AcquisitionStrategy::RedownloadReplaceIfBetter),
        _ => Err(format!("unsupported request strategy '{strategy}'")),
    }
}

async fn load_planned_acquisition_result(
    state: &State<'_, AppState>,
    request_id: i64,
    provider_order: Vec<String>,
    cached_provider_ids: Vec<String>,
) -> Result<PlannedAcquisitionResult, String> {
    let request = state
        .control_db
        .get_acquisition_request(request_id)
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("request {request_id} not found"))?;
    let (summary, provider_searches, candidate_review) =
        if let Some(task_id) = request.task_id.as_deref() {
            let db = state.db.lock().map_err(|error| error.to_string())?;
            (
                db.get_candidate_set_summary(task_id)
                    .map_err(|error| error.to_string())?,
                db.get_provider_search_records(task_id)
                    .map_err(|error| error.to_string())?,
                db.get_candidate_review(task_id)
                    .map_err(|error| error.to_string())?,
            )
        } else {
            (None, Vec::new(), Vec::new())
        };

    Ok(PlannedAcquisitionResult {
        identity_lane: planner_identity_lane_from_row(&request),
        edition: edition_context_from_payload(request.raw_payload_json.as_deref()),
        request,
        provider_order,
        cached_provider_ids,
        summary,
        provider_searches,
        candidate_review,
    })
}

fn derive_identity_confidence(request: &AcquisitionRequestRow) -> String {
    if request.musicbrainz_recording_id.is_some()
        || request.musicbrainz_release_id.is_some()
        || request.musicbrainz_release_group_id.is_some()
    {
        return "high".to_string();
    }

    if request.canonical_artist_id.is_some() || request.canonical_release_id.is_some() {
        return "medium".to_string();
    }

    "low".to_string()
}

fn derive_edition_match_outcome(
    edition_policy: Option<&str>,
    candidate_set: &Option<StoredCandidateSetSummary>,
    provider_searches: &[StoredProviderSearchRecord],
) -> String {
    if edition_policy
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .is_none()
    {
        return "insufficient_evidence".to_string();
    }

    if provider_searches
        .iter()
        .any(|entry| entry.outcome == "no_results_after_policy_filter")
    {
        return "mismatch".to_string();
    }

    if candidate_set
        .as_ref()
        .map(|summary| summary.candidate_count > 0)
        .unwrap_or(false)
    {
        return "match".to_string();
    }

    "insufficient_evidence".to_string()
}

fn planner_identity_envelope_from_request(request: &AcquisitionRequest) -> String {
    planner_identity_envelope_key(
        request.scope.as_str(),
        normalize_provider_value(Some(request.artist.as_str())),
        normalize_provider_value(request.album.as_deref()),
        normalize_provider_value(Some(request.title.as_str())),
        request.track_number,
        request.disc_number,
        request.musicbrainz_recording_id.as_deref(),
        request.musicbrainz_release_group_id.as_deref(),
        request.musicbrainz_release_id.as_deref(),
        request.canonical_artist_id,
        request.canonical_release_id,
    )
}

fn planner_identity_envelope_from_row(request: &AcquisitionRequestRow) -> String {
    planner_identity_envelope_key(
        request.scope.as_str(),
        normalize_provider_value(Some(request.normalized_artist.as_str())),
        normalize_provider_value(request.normalized_album.as_deref()),
        normalize_provider_value(Some(request.normalized_title.as_str())),
        request.track_number.and_then(|value| u32::try_from(value).ok()),
        request.disc_number.and_then(|value| u32::try_from(value).ok()),
        request.musicbrainz_recording_id.as_deref(),
        request.musicbrainz_release_group_id.as_deref(),
        request.musicbrainz_release_id.as_deref(),
        request.canonical_artist_id,
        request.canonical_release_id,
    )
}

#[allow(clippy::too_many_arguments)]
fn planner_identity_envelope_key(
    scope: &str,
    artist: String,
    album: String,
    title: String,
    track_number: Option<u32>,
    disc_number: Option<u32>,
    recording_id: Option<&str>,
    release_group_id: Option<&str>,
    release_id: Option<&str>,
    canonical_artist_id: Option<i64>,
    canonical_release_id: Option<i64>,
) -> String {
    let mbid_segment = if let Some(value) = normalize_optional_identity(release_group_id) {
        format!("release_group:{value}")
    } else if let Some(value) = normalize_optional_identity(release_id) {
        format!("release:{value}")
    } else if let Some(value) = normalize_optional_identity(recording_id) {
        format!("recording:{value}")
    } else {
        "".to_string()
    };

    let canonical_segment = match (canonical_artist_id, canonical_release_id) {
        (Some(artist_id), Some(release_id)) => format!("canon:{artist_id}:{release_id}"),
        (Some(artist_id), None) => format!("canon_artist:{artist_id}"),
        (None, Some(release_id)) => format!("canon_release:{release_id}"),
        (None, None) => String::new(),
    };

    [
        PLANNER_IDENTITY_ENVELOPE_PREFIX.to_string(),
        normalize_provider_value(Some(scope)),
        mbid_segment,
        canonical_segment,
        artist,
        album,
        title,
        track_number.map(|value| value.to_string()).unwrap_or_default(),
        disc_number.map(|value| value.to_string()).unwrap_or_default(),
    ]
    .join("::")
}

fn normalize_optional_identity(value: Option<&str>) -> Option<String> {
    value
        .map(|segment| segment.trim().to_ascii_lowercase())
        .filter(|segment| !segment.is_empty())
}

fn augment_request_payload_with_edition_context(
    request: &mut AcquisitionRequest,
) -> Result<(), String> {
    let mut payload = parse_or_initialize_payload(request.raw_payload_json.as_deref());
    payload["edition"] = serde_json::to_value(edition_context_from_request(request))
        .map_err(|error| error.to_string())?;
    request.raw_payload_json = Some(payload.to_string());
    Ok(())
}

fn augment_request_payload_with_identity_envelope(
    request: &mut AcquisitionRequest,
) -> Result<(), String> {
    let mut payload = parse_or_initialize_payload(request.raw_payload_json.as_deref());
    payload["identity_envelope_key"] = serde_json::to_value(planner_identity_envelope_from_request(request))
        .map_err(|error| error.to_string())?;
    request.raw_payload_json = Some(payload.to_string());
    Ok(())
}

fn parse_or_initialize_payload(raw_payload_json: Option<&str>) -> Value {
    let Some(raw) = raw_payload_json else {
        return json!({});
    };

    let Ok(parsed) = serde_json::from_str::<Value>(raw) else {
        return json!({ "legacy_payload": raw });
    };

    if parsed.is_object() {
        parsed
    } else {
        json!({ "legacy_payload": parsed })
    }
}

fn edition_context_from_request(request: &AcquisitionRequest) -> EditionContext {
    let album = request.album.as_deref().unwrap_or_default();
    let title = request.title.as_str();
    let combined = format!("{album} {title}").to_ascii_lowercase();

    let evidence_source = if request.musicbrainz_release_id.is_some()
        || request.musicbrainz_release_group_id.is_some()
    {
        "musicbrainz"
    } else {
        "inferred"
    };

    let confidence = if evidence_source == "musicbrainz" {
        "high"
    } else {
        "low"
    };

    EditionContext {
        policy: request.edition_policy.clone(),
        markers: EditionMarkers {
            is_live: combined.contains("live"),
            is_deluxe: combined.contains("deluxe") || combined.contains("expanded"),
            is_remaster: combined.contains("remaster"),
            country: None,
            label: None,
            catalog_number: None,
        },
        evidence: EditionEvidence {
            source: evidence_source.to_string(),
            confidence: confidence.to_string(),
        },
    }
}

fn edition_context_from_payload(raw_payload_json: Option<&str>) -> Option<EditionContext> {
    let raw = raw_payload_json?;
    let parsed = serde_json::from_str::<Value>(raw).ok()?;
    let edition_value = parsed.get("edition")?;
    serde_json::from_value::<EditionContext>(edition_value.clone()).ok()
}

fn validate_selected_albums_grammar(request: &AcquisitionRequest) -> Result<(), String> {
    if request.scope != cassette_core::acquisition::AcquisitionScope::SelectedAlbums {
        return Ok(());
    }

    let payload = request
        .raw_payload_json
        .as_deref()
        .ok_or_else(|| "validation_error:include_empty:selected_albums requires targets.include".to_string())?;

    let parsed = serde_json::from_str::<SelectedAlbumsPayload>(payload).map_err(|_| {
        "validation_error:ambiguous_album_identity:selected_albums payload is invalid".to_string()
    })?;

    if parsed.targets.include.is_empty() {
        return Err("validation_error:include_empty:selected_albums include list is empty".to_string());
    }

    let mut include_keys = Vec::with_capacity(parsed.targets.include.len());
    for entry in &parsed.targets.include {
        include_keys.push(normalized_album_target_key(entry)?);
    }

    for entry in &parsed.targets.exclude {
        let key = normalized_album_target_key(entry)?;
        if include_keys.contains(&key) {
            return Err(
                "validation_error:include_exclude_conflict:entry exists in both include and exclude"
                    .to_string(),
            );
        }
    }

    Ok(())
}

fn normalized_album_target_key(entry: &AlbumTarget) -> Result<String, String> {
    let release_group_id = entry
        .release_group_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase());

    if let Some(release_group_id) = release_group_id {
        return Ok(format!("rg::{release_group_id}"));
    }

    let artist = entry
        .artist
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase());
    let album = entry
        .album
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase());

    match (artist, album) {
        (Some(artist), Some(album)) => Ok(format!("aa::{artist}::{album}")),
        _ => Err(
            "validation_error:ambiguous_album_identity:album target requires release_group_id or artist+album"
                .to_string(),
        ),
    }
}

fn planner_identity_lane_from_request(request: &AcquisitionRequest) -> PlannerIdentityLane {
    PlannerIdentityLane {
        scope: request.scope.as_str().to_string(),
        musicbrainz_release_group_id: request.musicbrainz_release_group_id.clone(),
        musicbrainz_release_id: request.musicbrainz_release_id.clone(),
        musicbrainz_recording_id: request.musicbrainz_recording_id.clone(),
        canonical_artist_id: request.canonical_artist_id,
        canonical_release_id: request.canonical_release_id,
        quality_policy: request.quality_policy.clone(),
        edition_policy: request.edition_policy.clone(),
        confirmation_policy: request.confirmation_policy.as_str().to_string(),
    }
}

fn planner_identity_lane_from_row(request: &AcquisitionRequestRow) -> PlannerIdentityLane {
    PlannerIdentityLane {
        scope: request.scope.clone(),
        musicbrainz_release_group_id: request.musicbrainz_release_group_id.clone(),
        musicbrainz_release_id: request.musicbrainz_release_id.clone(),
        musicbrainz_recording_id: request.musicbrainz_recording_id.clone(),
        canonical_artist_id: request.canonical_artist_id,
        canonical_release_id: request.canonical_release_id,
        quality_policy: request.quality_policy.clone(),
        edition_policy: request.edition_policy.clone(),
        confirmation_policy: request.confirmation_policy.clone(),
    }
}

async fn search_planner_candidates(
    config: &cassette_core::director::DirectorConfig,
    providers: &[Arc<dyn Provider>],
    task: &cassette_core::director::TrackTask,
    cached_rows: &[StoredProviderResponseCache],
    edition_policy: Option<&str>,
) -> Result<
    (
        StrategyPlan,
        Vec<ProviderSearchRecord>,
        Vec<CandidateRecord>,
        Vec<String>,
    ),
    String,
> {
    let planner = StrategyPlanner;
    let descriptors = providers
        .iter()
        .map(|provider| provider.descriptor())
        .collect::<Vec<_>>();
    let plan = planner.plan(task, &descriptors, config);
    let provider_map = providers
        .iter()
        .map(|provider| (provider.descriptor().id.clone(), Arc::clone(provider)))
        .collect::<HashMap<_, _>>();
    let cached_map = cached_rows
        .iter()
        .map(|row| (row.provider_id.clone(), row))
        .collect::<HashMap<_, _>>();

    let mut provider_searches = Vec::new();
    let mut candidate_records = Vec::new();
    let mut cached_provider_ids = Vec::new();

    for (provider_order_index, provider_id) in plan.provider_order.iter().enumerate() {
        let Some(provider) = provider_map.get(provider_id) else {
            continue;
        };
        let descriptor = provider.descriptor();

        if let Some(cached_row) = cached_map.get(provider_id) {
            if cache_is_fresh(
                cached_row.updated_at.as_str(),
                config.provider_response_cache_max_age_secs,
            ) {
                let cached_candidates =
                    decode_cached_candidates(cached_row, &descriptor, provider_order_index);
                let cached_count = cached_candidates.len();
                let (filtered_candidates, filtered_count) = apply_edition_policy_filter_to_records(
                    cached_candidates,
                    edition_policy,
                    task.target.album.as_deref(),
                );
                cached_provider_ids.push(provider_id.clone());
                provider_searches.push(ProviderSearchRecord {
                    provider_id: descriptor.id.clone(),
                    provider_display_name: descriptor.display_name.clone(),
                    provider_trust_rank: descriptor.trust_rank,
                    provider_order_index,
                    outcome: if filtered_count > 0 {
                        format!("cache_hit:{}:edition_filtered", cached_row.outcome)
                    } else {
                        format!("cache_hit:{}", cached_row.outcome)
                    },
                    candidate_count: cached_count.saturating_sub(filtered_count),
                    error: None,
                    retryable: false,
                });
                candidate_records.extend(filtered_candidates);
                continue;
            }
        }

        if !descriptor.capabilities.supports_search {
            provider_searches.push(ProviderSearchRecord {
                provider_id: descriptor.id.clone(),
                provider_display_name: descriptor.display_name.clone(),
                provider_trust_rank: descriptor.trust_rank,
                provider_order_index,
                outcome: "search_not_supported".to_string(),
                candidate_count: 0,
                error: None,
                retryable: false,
            });
            continue;
        }

        match provider.search(task, &plan).await {
            Ok(candidates) => {
                let (filtered_candidates, filtered_count) = apply_edition_policy_filter(
                    candidates,
                    edition_policy,
                    task.target.album.as_deref(),
                );
                provider_searches.push(ProviderSearchRecord {
                    provider_id: descriptor.id.clone(),
                    provider_display_name: descriptor.display_name.clone(),
                    provider_trust_rank: descriptor.trust_rank,
                    provider_order_index,
                    outcome: if filtered_candidates.is_empty() {
                        if filtered_count > 0 {
                            "no_results_after_policy_filter".to_string()
                        } else {
                            "no_results".to_string()
                        }
                    } else if filtered_count > 0 {
                        "planned_search_edition_filtered".to_string()
                    } else {
                        "planned_search".to_string()
                    },
                    candidate_count: filtered_candidates.len(),
                    error: None,
                    retryable: false,
                });
                candidate_records.extend(candidate_records_from_candidates(
                    &descriptor,
                    provider_order_index,
                    filtered_candidates,
                    "planned",
                ));
            }
            Err(error) => {
                provider_searches.push(ProviderSearchRecord {
                    provider_id: descriptor.id.clone(),
                    provider_display_name: descriptor.display_name.clone(),
                    provider_trust_rank: descriptor.trust_rank,
                    provider_order_index,
                    outcome: provider_error_outcome(&error),
                    candidate_count: 0,
                    error: Some(error.to_string()),
                    retryable: error.retryable(),
                });
            }
        }
    }

    Ok((
        plan,
        provider_searches,
        candidate_records,
        cached_provider_ids,
    ))
}

fn apply_edition_policy_filter(
    candidates: Vec<ProviderSearchCandidate>,
    edition_policy: Option<&str>,
    requested_album: Option<&str>,
) -> (Vec<ProviderSearchCandidate>, usize) {
    let Some(policy) = edition_policy
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase())
    else {
        return (candidates, 0);
    };

    let requested_album_has_edition_markers = requested_album
        .map(contains_edition_marker)
        .unwrap_or(false);
    let requested_album_has_live_marker =
        requested_album.map(contains_live_marker).unwrap_or(false);

    let mut kept = Vec::with_capacity(candidates.len());
    let mut filtered = 0usize;
    for candidate in candidates {
        let candidate_album = candidate.album.as_deref();
        let candidate_has_edition_markers = candidate_album
            .map(contains_edition_marker)
            .unwrap_or(false);
        let candidate_has_live_marker = candidate_album.map(contains_live_marker).unwrap_or(false);

        let reject = match policy.as_str() {
            "standard_only" => {
                if requested_album_has_edition_markers {
                    false
                } else {
                    candidate_has_edition_markers
                }
            }
            "no_live" => {
                if requested_album_has_live_marker {
                    false
                } else {
                    candidate_has_live_marker
                }
            }
            _ => false,
        };

        if reject {
            filtered += 1;
        } else {
            kept.push(candidate);
        }
    }

    (kept, filtered)
}

fn apply_edition_policy_filter_to_records(
    candidates: Vec<CandidateRecord>,
    edition_policy: Option<&str>,
    requested_album: Option<&str>,
) -> (Vec<CandidateRecord>, usize) {
    let Some(policy) = edition_policy
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase())
    else {
        return (candidates, 0);
    };

    let requested_album_has_edition_markers = requested_album
        .map(contains_edition_marker)
        .unwrap_or(false);
    let requested_album_has_live_marker =
        requested_album.map(contains_live_marker).unwrap_or(false);

    let mut kept = Vec::with_capacity(candidates.len());
    let mut filtered = 0usize;
    for candidate in candidates {
        let candidate_album = candidate.candidate.album.as_deref();
        let candidate_has_edition_markers = candidate_album
            .map(contains_edition_marker)
            .unwrap_or(false);
        let candidate_has_live_marker = candidate_album.map(contains_live_marker).unwrap_or(false);

        let reject = match policy.as_str() {
            "standard_only" => {
                if requested_album_has_edition_markers {
                    false
                } else {
                    candidate_has_edition_markers
                }
            }
            "no_live" => {
                if requested_album_has_live_marker {
                    false
                } else {
                    candidate_has_live_marker
                }
            }
            _ => false,
        };

        if reject {
            filtered += 1;
        } else {
            kept.push(candidate);
        }
    }

    (kept, filtered)
}

fn contains_edition_marker(value: &str) -> bool {
    let normalized = value.to_ascii_lowercase();
    EDITION_MARKERS
        .iter()
        .any(|marker| normalized.contains(marker))
}

fn contains_live_marker(value: &str) -> bool {
    value.to_ascii_lowercase().contains("live")
}

fn candidate_records_from_candidates(
    descriptor: &cassette_core::director::ProviderDescriptor,
    provider_order_index: usize,
    candidates: Vec<ProviderSearchCandidate>,
    outcome: &str,
) -> Vec<CandidateRecord> {
    candidates
        .into_iter()
        .enumerate()
        .map(|(search_rank, candidate)| CandidateRecord {
            provider_id: descriptor.id.clone(),
            provider_display_name: descriptor.display_name.clone(),
            provider_trust_rank: descriptor.trust_rank,
            provider_order_index,
            search_rank,
            candidate,
            acquisition_temp_path: None,
            validation: None,
            score: None,
            score_reason: None,
            outcome: outcome.to_string(),
            rejection_reason: None,
        })
        .collect()
}

fn decode_cached_candidates(
    row: &StoredProviderResponseCache,
    descriptor: &cassette_core::director::ProviderDescriptor,
    provider_order_index: usize,
) -> Vec<CandidateRecord> {
    let Ok(envelope) =
        serde_json::from_str::<PersistedProviderResponseEnvelope>(&row.response_json)
    else {
        return Vec::new();
    };

    let candidates = envelope
        .candidate_records
        .into_iter()
        .filter(|record| record.provider_id == row.provider_id)
        .map(|record| record.candidate)
        .collect::<Vec<_>>();
    candidate_records_from_candidates(
        descriptor,
        provider_order_index,
        candidates,
        "planned_cached",
    )
}

fn cache_is_fresh(updated_at: &str, max_age_secs: i64) -> bool {
    let Ok(parsed) = NaiveDateTime::parse_from_str(updated_at, "%Y-%m-%d %H:%M:%S") else {
        return false;
    };
    let age = Utc::now().naive_utc() - parsed;
    age.num_seconds() <= max_age_secs.max(1)
}

fn provider_error_outcome(error: &ProviderError) -> String {
    match error {
        ProviderError::AuthFailed { .. } => "auth_failed".to_string(),
        ProviderError::RateLimited { .. } => "rate_limited".to_string(),
        ProviderError::TimedOut { .. } => "timed_out".to_string(),
        ProviderError::NotFound { .. } => "not_found".to_string(),
        ProviderError::Network { .. } => "network_error".to_string(),
        ProviderError::UnsupportedContent { .. } => "unsupported_content".to_string(),
        ProviderError::MetadataMismatch { .. } => "metadata_mismatch".to_string(),
        ProviderError::InvalidAudio { .. } => "invalid_audio".to_string(),
        ProviderError::TemporaryOutage { .. } => "temporary_outage".to_string(),
        ProviderError::Other { .. } => "other_failure".to_string(),
        ProviderError::ProviderBusy { .. } => "provider_busy".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        apply_edition_policy_filter, augment_request_payload_with_edition_context,
        augment_request_payload_with_identity_envelope, cache_is_fresh,
        build_review_preflight_result,
        CandidateReviewItem,
        derive_edition_match_outcome, derive_identity_confidence, edition_context_from_payload,
        has_low_trust_approval_artifact, merge_excluded_providers, parse_acquisition_strategy,
        latest_preflight_from_timeline,
        planner_identity_envelope_from_request,
        parse_track_task_source,
        planner_exclusion_memory_key_from_request, planner_identity_lane_from_request,
        ReviewPreflightResult, validate_low_trust_approval,
        selected_low_trust_provider_ids,
        validate_selected_albums_grammar,
    };
    use cassette_core::acquisition::{
        AcquisitionRequest, AcquisitionRequestStatus, AcquisitionScope, ConfirmationPolicy,
    };
    use cassette_core::librarian::models::AcquisitionRequestEvent;
    use cassette_core::db::{StoredCandidateSetSummary, StoredProviderSearchRecord};
    use cassette_core::director::models::{
        AcquisitionStrategy, ProviderSearchCandidate, TrackTaskSource,
    };
    use chrono::Utc;
    use serde_json::json;

    #[test]
    fn fresh_cache_rows_are_accepted() {
        let fresh = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
        assert!(cache_is_fresh(&fresh, 60));
    }

    #[test]
    fn stale_cache_rows_are_rejected() {
        assert!(!cache_is_fresh("2001-01-01 00:00:00", 60));
    }

    #[test]
    fn strategy_parser_recognizes_known_values() {
        let parsed = parse_acquisition_strategy("discography_batch").expect("known strategy");
        assert_eq!(parsed, AcquisitionStrategy::DiscographyBatch);
    }

    #[test]
    fn source_parser_rejects_unknown_values() {
        let parsed = parse_track_task_source("not-a-real-source");
        assert!(parsed.is_err());

        let manual = parse_track_task_source("manual").expect("manual source");
        assert!(matches!(manual, TrackTaskSource::Manual));
    }

    #[test]
    fn planner_identity_lane_includes_release_group_and_policy_fields() {
        let request = AcquisitionRequest {
            id: None,
            scope: AcquisitionScope::Album,
            source: TrackTaskSource::Manual,
            source_name: "manual".to_string(),
            source_track_id: Some("spotify:track:1".to_string()),
            source_album_id: Some("spotify:album:1".to_string()),
            source_artist_id: Some("spotify:artist:1".to_string()),
            artist: "Artist".to_string(),
            album: Some("Album".to_string()),
            title: "Song".to_string(),
            track_number: Some(1),
            disc_number: Some(1),
            year: Some(2024),
            duration_secs: Some(180.0),
            isrc: Some("US1234567890".to_string()),
            musicbrainz_recording_id: Some("mb-recording-1".to_string()),
            musicbrainz_release_group_id: Some("mb-release-group-1".to_string()),
            musicbrainz_release_id: Some("mb-release-1".to_string()),
            canonical_artist_id: Some(10),
            canonical_release_id: Some(20),
            strategy: AcquisitionStrategy::Standard,
            quality_policy: Some("lossless_preferred".to_string()),
            excluded_providers: vec!["yt_dlp".to_string()],
            edition_policy: Some("standard_only".to_string()),
            confirmation_policy: ConfirmationPolicy::ManualReview,
            desired_track_id: Some(33),
            source_operation_id: Some("op-1".to_string()),
            task_id: Some("task-1".to_string()),
            request_signature: Some("sig-1".to_string()),
            status: AcquisitionRequestStatus::Pending,
            raw_payload_json: None,
        };

        let lane = planner_identity_lane_from_request(&request);
        assert_eq!(lane.scope, "album");
        assert_eq!(
            lane.musicbrainz_release_group_id.as_deref(),
            Some("mb-release-group-1")
        );
        assert_eq!(lane.musicbrainz_release_id.as_deref(), Some("mb-release-1"));
        assert_eq!(lane.quality_policy.as_deref(), Some("lossless_preferred"));
        assert_eq!(lane.edition_policy.as_deref(), Some("standard_only"));
        assert_eq!(lane.confirmation_policy, "manual_review");
    }

    #[test]
    fn edition_policy_standard_only_filters_edition_variants() {
        let candidates = vec![
            ProviderSearchCandidate {
                provider_id: "p".to_string(),
                provider_candidate_id: "1".to_string(),
                artist: "Artist".to_string(),
                title: "Song".to_string(),
                album: Some("Album".to_string()),
                duration_secs: Some(180.0),
                extension_hint: None,
                bitrate_kbps: None,
                cover_art_url: None,
                metadata_confidence: 0.9,
            },
            ProviderSearchCandidate {
                provider_id: "p".to_string(),
                provider_candidate_id: "2".to_string(),
                artist: "Artist".to_string(),
                title: "Song".to_string(),
                album: Some("Album (Deluxe Edition)".to_string()),
                duration_secs: Some(180.0),
                extension_hint: None,
                bitrate_kbps: None,
                cover_art_url: None,
                metadata_confidence: 0.9,
            },
        ];

        let (kept, filtered) =
            apply_edition_policy_filter(candidates, Some("standard_only"), Some("Album"));
        assert_eq!(kept.len(), 1);
        assert_eq!(filtered, 1);
        assert_eq!(kept[0].provider_candidate_id, "1");
    }

    #[test]
    fn edition_policy_no_live_keeps_requested_live_album() {
        let candidates = vec![ProviderSearchCandidate {
            provider_id: "p".to_string(),
            provider_candidate_id: "1".to_string(),
            artist: "Artist".to_string(),
            title: "Song".to_string(),
            album: Some("Album (Live)".to_string()),
            duration_secs: Some(180.0),
            extension_hint: None,
            bitrate_kbps: None,
            cover_art_url: None,
            metadata_confidence: 0.9,
        }];

        let (kept, filtered) =
            apply_edition_policy_filter(candidates, Some("no_live"), Some("Album Live"));
        assert_eq!(kept.len(), 1);
        assert_eq!(filtered, 0);
    }

    #[test]
    fn merge_excluded_providers_dedupes_and_normalizes() {
        let existing = vec!["Yt_Dlp".to_string(), "  ".to_string()];
        let remembered = vec!["usenet".to_string(), "yt_dlp".to_string()];
        let merged = merge_excluded_providers(&existing, &remembered);
        assert_eq!(merged, vec!["yt_dlp".to_string(), "usenet".to_string()]);
    }

    #[test]
    fn exclusion_memory_key_is_stable_and_normalized() {
        let request = AcquisitionRequest {
            id: None,
            scope: AcquisitionScope::Track,
            source: TrackTaskSource::Manual,
            source_name: "manual".to_string(),
            source_track_id: None,
            source_album_id: None,
            source_artist_id: None,
            artist: "  Test Artist  ".to_string(),
            album: Some("Album".to_string()),
            title: "Song".to_string(),
            track_number: Some(7),
            disc_number: Some(1),
            year: None,
            duration_secs: None,
            isrc: None,
            musicbrainz_recording_id: Some("MB-REC".to_string()),
            musicbrainz_release_group_id: None,
            musicbrainz_release_id: None,
            canonical_artist_id: Some(10),
            canonical_release_id: Some(11),
            strategy: AcquisitionStrategy::Standard,
            quality_policy: Some("lossless".to_string()),
            excluded_providers: Vec::new(),
            edition_policy: Some("standard_only".to_string()),
            confirmation_policy: ConfirmationPolicy::ManualReview,
            desired_track_id: None,
            source_operation_id: None,
            task_id: Some("task-1".to_string()),
            request_signature: Some("sig-1".to_string()),
            status: AcquisitionRequestStatus::Pending,
            raw_payload_json: None,
        };

        let key = planner_exclusion_memory_key_from_request(&request);
        assert!(key.contains("planner.exclusion_memory.v1"));
        assert!(key.contains("test artist"));
        assert!(key.contains("manual_review"));
    }

    #[test]
    fn selected_albums_grammar_rejects_include_exclude_conflict() {
        let mut request = AcquisitionRequest {
            id: None,
            scope: AcquisitionScope::SelectedAlbums,
            source: TrackTaskSource::Manual,
            source_name: "manual".to_string(),
            source_track_id: None,
            source_album_id: None,
            source_artist_id: None,
            artist: "Artist".to_string(),
            album: Some("Album".to_string()),
            title: "Song".to_string(),
            track_number: None,
            disc_number: None,
            year: None,
            duration_secs: None,
            isrc: None,
            musicbrainz_recording_id: None,
            musicbrainz_release_group_id: None,
            musicbrainz_release_id: None,
            canonical_artist_id: None,
            canonical_release_id: None,
            strategy: AcquisitionStrategy::DiscographyBatch,
            quality_policy: None,
            excluded_providers: Vec::new(),
            edition_policy: Some("standard_only".to_string()),
            confirmation_policy: ConfirmationPolicy::ManualReview,
            desired_track_id: None,
            source_operation_id: None,
            task_id: Some("task-selected-albums".to_string()),
            request_signature: None,
            status: AcquisitionRequestStatus::Pending,
            raw_payload_json: Some(
                json!({
                    "targets": {
                        "include": [{"artist":"A","album":"B"}],
                        "exclude": [{"artist":"A","album":"B"}]
                    }
                })
                .to_string(),
            ),
        };

        augment_request_payload_with_edition_context(&mut request)
            .expect("edition context should serialize");
        let error = validate_selected_albums_grammar(&request).expect_err("must fail conflict");
        assert!(error.contains("validation_error:include_exclude_conflict"));
    }

    #[test]
    fn selected_albums_grammar_accepts_release_group_targets() {
        let mut request = AcquisitionRequest {
            id: None,
            scope: AcquisitionScope::SelectedAlbums,
            source: TrackTaskSource::Manual,
            source_name: "manual".to_string(),
            source_track_id: None,
            source_album_id: None,
            source_artist_id: None,
            artist: "Artist".to_string(),
            album: Some("Album".to_string()),
            title: "Song".to_string(),
            track_number: None,
            disc_number: None,
            year: None,
            duration_secs: None,
            isrc: None,
            musicbrainz_recording_id: None,
            musicbrainz_release_group_id: None,
            musicbrainz_release_id: None,
            canonical_artist_id: None,
            canonical_release_id: None,
            strategy: AcquisitionStrategy::DiscographyBatch,
            quality_policy: None,
            excluded_providers: Vec::new(),
            edition_policy: Some("standard_only".to_string()),
            confirmation_policy: ConfirmationPolicy::ManualReview,
            desired_track_id: None,
            source_operation_id: None,
            task_id: Some("task-selected-albums-ok".to_string()),
            request_signature: None,
            status: AcquisitionRequestStatus::Pending,
            raw_payload_json: Some(
                json!({
                    "targets": {
                        "include": [{"release_group_id":"rg-1"}],
                        "exclude": []
                    }
                })
                .to_string(),
            ),
        };

        augment_request_payload_with_edition_context(&mut request)
            .expect("edition context should serialize");
        validate_selected_albums_grammar(&request).expect("valid grammar should pass");
    }

    #[test]
    fn request_payload_round_trips_edition_context() {
        let mut request = AcquisitionRequest {
            id: None,
            scope: AcquisitionScope::Album,
            source: TrackTaskSource::Manual,
            source_name: "manual".to_string(),
            source_track_id: None,
            source_album_id: None,
            source_artist_id: None,
            artist: "Artist".to_string(),
            album: Some("Album (Deluxe)".to_string()),
            title: "Song".to_string(),
            track_number: None,
            disc_number: None,
            year: None,
            duration_secs: None,
            isrc: None,
            musicbrainz_recording_id: None,
            musicbrainz_release_group_id: Some("mb-rg-1".to_string()),
            musicbrainz_release_id: None,
            canonical_artist_id: None,
            canonical_release_id: None,
            strategy: AcquisitionStrategy::Standard,
            quality_policy: None,
            excluded_providers: Vec::new(),
            edition_policy: Some("standard_only".to_string()),
            confirmation_policy: ConfirmationPolicy::ManualReview,
            desired_track_id: None,
            source_operation_id: None,
            task_id: Some("task-edition-ctx".to_string()),
            request_signature: None,
            status: AcquisitionRequestStatus::Pending,
            raw_payload_json: None,
        };

        augment_request_payload_with_edition_context(&mut request)
            .expect("edition context should serialize");
        let edition = edition_context_from_payload(request.raw_payload_json.as_deref())
            .expect("edition context should parse back");
        assert_eq!(edition.policy.as_deref(), Some("standard_only"));
        assert!(edition.markers.is_deluxe);
        assert_eq!(edition.evidence.source, "musicbrainz");
    }

    #[test]
    fn identity_confidence_prefers_musicbrainz_ids() {
        let row = cassette_core::librarian::models::AcquisitionRequestRow {
            id: 1,
            scope: "track".to_string(),
            source_name: "manual".to_string(),
            source_track_id: None,
            source_album_id: None,
            source_artist_id: None,
            artist: "Artist".to_string(),
            album: None,
            title: "Song".to_string(),
            normalized_artist: "artist".to_string(),
            normalized_album: None,
            normalized_title: "song".to_string(),
            track_number: None,
            disc_number: None,
            year: None,
            duration_secs: None,
            isrc: None,
            musicbrainz_recording_id: Some("mb-rec-1".to_string()),
            musicbrainz_release_group_id: None,
            musicbrainz_release_id: None,
            canonical_artist_id: None,
            canonical_release_id: None,
            strategy: "standard".to_string(),
            quality_policy: None,
            excluded_providers_json: None,
            edition_policy: None,
            confirmation_policy: "manual_review".to_string(),
            desired_track_id: None,
            source_operation_id: None,
            task_id: Some("task-1".to_string()),
            request_signature: "sig-1".to_string(),
            status: "reviewing".to_string(),
            raw_payload_json: None,
            created_at: "2026-04-07 00:00:00".to_string(),
            updated_at: "2026-04-07 00:00:00".to_string(),
        };

        assert_eq!(derive_identity_confidence(&row), "high");
    }

    #[test]
    fn edition_match_outcome_reports_mismatch_after_policy_filter() {
        let provider_searches = vec![StoredProviderSearchRecord {
            provider_id: "qobuz".to_string(),
            provider_display_name: "Qobuz".to_string(),
            provider_trust_rank: 10,
            provider_order_index: 0,
            outcome: "no_results_after_policy_filter".to_string(),
            candidate_count: 0,
            error: None,
            retryable: false,
            recorded_at: "2026-04-07 00:00:00".to_string(),
        }];
        let candidate_set = Some(StoredCandidateSetSummary {
            task_id: "task-1".to_string(),
            request_signature: Some("sig-1".to_string()),
            request_strategy: Some("standard".to_string()),
            disposition: "reviewing".to_string(),
            selected_provider: None,
            candidate_count: 0,
            provider_count: 1,
            updated_at: "2026-04-07 00:00:00".to_string(),
        });

        assert_eq!(
            derive_edition_match_outcome(Some("standard_only"), &candidate_set, &provider_searches),
            "mismatch"
        );
    }

    #[test]
    fn selected_low_trust_provider_ids_only_returns_selected_low_trust() {
        let review = vec![
            CandidateReviewItem {
                task_id: "task-1".to_string(),
                provider_id: "yt_dlp".to_string(),
                provider_display_name: "yt-dlp".to_string(),
                provider_trust_rank: 50,
                provider_candidate_id: "cand-1".to_string(),
                outcome: "planned".to_string(),
                rejection_reason: None,
                is_selected: true,
                score_total: None,
                candidate_json: "{}".to_string(),
                validation_json: None,
                score_reason_json: None,
            },
            CandidateReviewItem {
                task_id: "task-1".to_string(),
                provider_id: "qobuz".to_string(),
                provider_display_name: "Qobuz".to_string(),
                provider_trust_rank: 10,
                provider_candidate_id: "cand-2".to_string(),
                outcome: "planned".to_string(),
                rejection_reason: None,
                is_selected: true,
                score_total: None,
                candidate_json: "{}".to_string(),
                validation_json: None,
                score_reason_json: None,
            },
            CandidateReviewItem {
                task_id: "task-1".to_string(),
                provider_id: "real_debrid".to_string(),
                provider_display_name: "Real-Debrid".to_string(),
                provider_trust_rank: 80,
                provider_candidate_id: "cand-3".to_string(),
                outcome: "planned".to_string(),
                rejection_reason: None,
                is_selected: false,
                score_total: None,
                candidate_json: "{}".to_string(),
                validation_json: None,
                score_reason_json: None,
            },
        ];

        let selected = selected_low_trust_provider_ids(&review);
        assert_eq!(selected, vec!["yt_dlp".to_string()]);
    }

    #[test]
    fn low_trust_approval_artifact_token_is_required() {
        assert!(!has_low_trust_approval_artifact(None));
        assert!(!has_low_trust_approval_artifact(Some("looks good")));
        assert!(has_low_trust_approval_artifact(Some(
            "manual review approve_low_trust"
        )));
    }

    #[test]
    fn low_trust_approval_requires_explicit_token_for_selected_low_trust_providers() {
        let selected = vec!["yt_dlp".to_string(), "real_debrid".to_string()];

        let err = validate_low_trust_approval(Some("looks good"), &selected)
            .expect_err("missing approval token should be rejected");
        assert!(err.contains("approve_low_trust"));
        assert!(err.contains("yt_dlp"));

        assert!(validate_low_trust_approval(
            Some("approved with approve_low_trust token"),
            &selected
        )
        .is_ok());
        assert!(validate_low_trust_approval(None, &[]).is_ok());
    }

    #[test]
    fn identity_envelope_is_stable_across_source_aliases() {
        let mut request_a = AcquisitionRequest {
            id: None,
            scope: AcquisitionScope::Track,
            source: TrackTaskSource::SpotifyLibrary,
            source_name: "spotify_library".to_string(),
            source_track_id: Some("spotify:track:123".to_string()),
            source_album_id: Some("spotify:album:321".to_string()),
            source_artist_id: Some("spotify:artist:456".to_string()),
            artist: "The Artist".to_string(),
            album: Some("The Album".to_string()),
            title: "The Song".to_string(),
            track_number: Some(1),
            disc_number: Some(1),
            year: Some(2020),
            duration_secs: Some(240.0),
            isrc: None,
            musicbrainz_recording_id: None,
            musicbrainz_release_group_id: Some("mb-rg-123".to_string()),
            musicbrainz_release_id: None,
            canonical_artist_id: Some(11),
            canonical_release_id: Some(22),
            strategy: AcquisitionStrategy::Standard,
            quality_policy: Some("balanced".to_string()),
            excluded_providers: Vec::new(),
            edition_policy: Some("standard_only".to_string()),
            confirmation_policy: ConfirmationPolicy::ManualReview,
            desired_track_id: None,
            source_operation_id: Some("op-a".to_string()),
            task_id: None,
            request_signature: None,
            status: AcquisitionRequestStatus::Pending,
            raw_payload_json: None,
        };
        let mut request_b = request_a.clone();
        request_b.source = TrackTaskSource::SpotifyHistory;
        request_b.source_name = "spotify_history".to_string();
        request_b.source_track_id = Some("history:track:999".to_string());
        request_b.source_album_id = Some("history:album:999".to_string());
        request_b.source_artist_id = Some("history:artist:999".to_string());
        request_b.source_operation_id = Some("op-b".to_string());

        let envelope_a = planner_identity_envelope_from_request(&request_a);
        let envelope_b = planner_identity_envelope_from_request(&request_b);
        assert_eq!(envelope_a, envelope_b);

        augment_request_payload_with_identity_envelope(&mut request_a)
            .expect("identity envelope should serialize");
        let payload = serde_json::from_str::<serde_json::Value>(
            request_a.raw_payload_json.as_deref().expect("payload should exist"),
        )
        .expect("payload should parse");
        assert_eq!(
            payload
                .get("identity_envelope_key")
                .and_then(|value| value.as_str()),
            Some(envelope_a.as_str())
        );
    }

    #[test]
    fn review_preflight_fails_when_no_selection_exists() {
        let candidate_set = Some(StoredCandidateSetSummary {
            task_id: "task-1".to_string(),
            request_signature: Some("sig-1".to_string()),
            request_strategy: Some("standard".to_string()),
            disposition: "reviewing".to_string(),
            selected_provider: None,
            candidate_count: 2,
            provider_count: 1,
            updated_at: "2026-04-07 00:00:00".to_string(),
        });
        let provider_searches = vec![StoredProviderSearchRecord {
            provider_id: "qobuz".to_string(),
            provider_display_name: "Qobuz".to_string(),
            provider_trust_rank: 10,
            provider_order_index: 0,
            outcome: "planned".to_string(),
            candidate_count: 2,
            error: None,
            retryable: false,
            recorded_at: "2026-04-07 00:00:00".to_string(),
        }];
        let candidate_review = vec![CandidateReviewItem {
            task_id: "task-1".to_string(),
            provider_id: "qobuz".to_string(),
            provider_display_name: "Qobuz".to_string(),
            provider_trust_rank: 10,
            provider_candidate_id: "cand-1".to_string(),
            outcome: "planned".to_string(),
            rejection_reason: None,
            is_selected: false,
            score_total: Some(90),
            candidate_json: "{}".to_string(),
            validation_json: None,
            score_reason_json: None,
        }];

        let preflight =
            build_review_preflight_result(&candidate_set, &provider_searches, &candidate_review);
        assert!(!preflight.passed);
        assert!(preflight.reason_codes.contains(&"no_selected_candidate".to_string()));
    }

    #[test]
    fn review_preflight_passes_with_candidate_selection() {
        let candidate_set = Some(StoredCandidateSetSummary {
            task_id: "task-2".to_string(),
            request_signature: Some("sig-2".to_string()),
            request_strategy: Some("standard".to_string()),
            disposition: "reviewing".to_string(),
            selected_provider: Some("qobuz".to_string()),
            candidate_count: 1,
            provider_count: 1,
            updated_at: "2026-04-07 00:00:00".to_string(),
        });
        let provider_searches = vec![StoredProviderSearchRecord {
            provider_id: "qobuz".to_string(),
            provider_display_name: "Qobuz".to_string(),
            provider_trust_rank: 10,
            provider_order_index: 0,
            outcome: "planned".to_string(),
            candidate_count: 1,
            error: None,
            retryable: false,
            recorded_at: "2026-04-07 00:00:00".to_string(),
        }];
        let candidate_review = vec![CandidateReviewItem {
            task_id: "task-2".to_string(),
            provider_id: "qobuz".to_string(),
            provider_display_name: "Qobuz".to_string(),
            provider_trust_rank: 10,
            provider_candidate_id: "cand-1".to_string(),
            outcome: "planned".to_string(),
            rejection_reason: None,
            is_selected: true,
            score_total: Some(92),
            candidate_json: "{}".to_string(),
            validation_json: None,
            score_reason_json: None,
        }];

        let preflight =
            build_review_preflight_result(&candidate_set, &provider_searches, &candidate_review);
        assert!(preflight.passed);
        assert!(preflight.reason_codes.is_empty());
    }

    #[test]
    fn latest_preflight_is_loaded_from_timeline_payload() {
        let persisted = ReviewPreflightResult {
            passed: true,
            checked_at: "2026-04-07T00:00:00Z".to_string(),
            reason_codes: Vec::new(),
            selected_candidate_count: 1,
            provider_search_count: 2,
            provider_success_count: 1,
            candidate_count: 1,
        };
        let timeline = vec![
            AcquisitionRequestEvent {
                id: 1,
                request_id: 1,
                task_id: Some("task-1".to_string()),
                event_type: "planning_completed".to_string(),
                status: "reviewing".to_string(),
                message: Some("planning done".to_string()),
                payload_json: None,
                created_at: "2026-04-07 00:00:00".to_string(),
            },
            AcquisitionRequestEvent {
                id: 2,
                request_id: 1,
                task_id: Some("task-1".to_string()),
                event_type: "review_preflight".to_string(),
                status: "reviewing".to_string(),
                message: Some("preflight".to_string()),
                payload_json: Some(serde_json::to_string(&persisted).expect("serialize")),
                created_at: "2026-04-07 00:00:01".to_string(),
            },
        ];

        let loaded = latest_preflight_from_timeline(&timeline).expect("preflight must exist");
        assert!(loaded.passed);
        assert_eq!(loaded.selected_candidate_count, 1);
    }
}
