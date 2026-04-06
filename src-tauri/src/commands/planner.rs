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

#[derive(Debug, Serialize)]
pub struct PlannedAcquisitionResult {
    pub request: AcquisitionRequestRow,
    pub identity_lane: PlannerIdentityLane,
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
    pub timeline: Vec<cassette_core::librarian::models::AcquisitionRequestEvent>,
    pub candidate_set: Option<StoredCandidateSetSummary>,
    pub provider_searches: Vec<StoredProviderSearchRecord>,
    pub candidate_review: Vec<CandidateReviewItem>,
    pub provider_memory: Vec<StoredProviderMemory>,
    pub provider_response_cache: Vec<StoredProviderResponseCache>,
    pub identity_resolution_evidence: Vec<StoredIdentityResolutionEvidence>,
    pub source_aliases: Vec<StoredSourceAlias>,
    pub execution: Option<cassette_core::db::TaskExecutionSummary>,
    pub provenance: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PersistedProviderResponseEnvelope {
    #[serde(default)]
    candidate_records: Vec<CandidateRecord>,
}

#[tauri::command]
pub async fn plan_acquisition(
    state: State<'_, AppState>,
    mut request: AcquisitionRequest,
) -> Result<PlannedAcquisitionResult, String> {
    if request.request_signature.is_none() {
        request.request_signature = Some(request.request_fingerprint());
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
    let Some(task_id) = request.task_id.as_deref() else {
        return Ok(serde_json::json!({
            "request": request,
            "candidate_set": null,
            "provider_searches": [],
            "candidate_review": [],
        }));
    };

    let db = state.db.lock().map_err(|error| error.to_string())?;
    Ok(serde_json::json!({
        "request": request,
        "candidate_set": db.get_candidate_set_summary(task_id).map_err(|error| error.to_string())?,
        "provider_searches": db.get_provider_search_records(task_id).map_err(|error| error.to_string())?,
        "candidate_review": db.get_candidate_review(task_id).map_err(|error| error.to_string())?,
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

    Ok(RequestRationale {
        identity_lane: planner_identity_lane_from_row(&request),
        request,
        timeline,
        candidate_set,
        provider_searches,
        candidate_review,
        provider_memory,
        provider_response_cache,
        identity_resolution_evidence,
        source_aliases,
        execution,
        provenance,
    })
}

#[tauri::command]
pub async fn approve_planned_request(
    state: State<'_, AppState>,
    request_id: i64,
    note: Option<String>,
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

    let task = track_task_from_request_row(&request)?;

    let payload = serde_json::json!({
        "request_id": request_id,
        "review_action": "approved",
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

    let payload = serde_json::json!({
        "request_id": request_id,
        "review_action": "rejected",
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
        request,
        provider_order,
        cached_provider_ids,
        summary,
        provider_searches,
        candidate_review,
    })
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
        apply_edition_policy_filter, cache_is_fresh, parse_acquisition_strategy,
        parse_track_task_source, planner_identity_lane_from_request,
    };
    use cassette_core::acquisition::{
        AcquisitionRequest, AcquisitionRequestStatus, AcquisitionScope, ConfirmationPolicy,
    };
    use cassette_core::director::models::{
        AcquisitionStrategy, ProviderSearchCandidate, TrackTaskSource,
    };
    use chrono::Utc;

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
}
