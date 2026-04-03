use crate::director::config::{DirectorConfig, DuplicatePolicy};
use crate::director::error::{DirectorError, ProviderError};
use crate::director::finalize::{finalize_selected_candidate, merge_normalized_track};
use crate::director::metadata::apply_metadata;
use crate::director::models::{
    CandidateDisposition, CandidateQuality, CandidateRecord, CandidateSelection, CandidateSelectionMode,
    DirectorEvent, DirectorProgress, DirectorTaskResult, FinalizedTrack,
    FinalizedTrackDisposition, ProviderAttemptRecord, ProviderDescriptor, ProviderHealthState,
    ProviderHealthStatus, ProviderSearchCandidate, ProviderSearchRecord, ProvenanceRecord,
    TrackTask,
};
use crate::director::provider::Provider;
use crate::director::scoring::score_candidate;
use crate::director::strategy::{StrategyPlan, StrategyPlanner};
use crate::director::temp::{TaskTempContext, TempManager};
use crate::director::validation::validate_candidate;
use crate::db::{director_request_signature, Db, StoredProviderMemory, StoredProviderResponseCache};
use chrono::Utc;
use moka::sync::Cache;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, RwLock, Semaphore};
use tokio::task::JoinSet;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tracing::{info, warn};

#[derive(Debug, Clone, Default)]
struct ProviderRuntimeState {
    unavailable_until: Option<chrono::DateTime<Utc>>,
    unavailable_reason: Option<String>,
    disabled_reason: Option<String>,
    busy_streak: u32,
}

#[derive(Debug, Clone)]
enum ProviderSkipReason {
    HealthDown(ProviderHealthState),
    Disabled { reason: String },
    CoolingDown { until: chrono::DateTime<Utc>, reason: String },
    PersistedCoolingDown { until: chrono::DateTime<Utc>, reason: String },
    PersistedMemory { reason: String },
}

impl ProviderSkipReason {
    fn attempt_outcome(&self) -> String {
        match self {
            Self::HealthDown(state) => format!(
                "skipped: provider health down{}",
                state.message
                    .as_deref()
                    .map(|message| format!(" - {message}"))
                    .unwrap_or_default()
            ),
            Self::Disabled { reason } => format!("skipped: provider unavailable - {reason}"),
            Self::CoolingDown { until, reason } => {
                format!("skipped: provider cooling down until {} - {reason}", until.to_rfc3339())
            }
            Self::PersistedCoolingDown { until, reason } => {
                format!(
                    "skipped: persisted provider cooldown until {} - {reason}",
                    until.to_rfc3339()
                )
            }
            Self::PersistedMemory { reason } => {
                format!("skipped: persisted provider memory - {reason}")
            }
        }
    }

    fn search_outcome(&self) -> &'static str {
        match self {
            Self::HealthDown(_) => "skipped_health_down",
            Self::Disabled { .. } => "skipped_runtime_unavailable",
            Self::CoolingDown { .. } => "skipped_runtime_cooldown",
            Self::PersistedCoolingDown { .. } => "skipped_persisted_cooldown",
            Self::PersistedMemory { .. } => "skipped_persisted_memory",
        }
    }

    fn error_message(&self) -> Option<String> {
        match self {
            Self::HealthDown(state) => state.message.clone(),
            Self::Disabled { reason } => Some(reason.clone()),
            Self::CoolingDown { reason, .. } => Some(reason.clone()),
            Self::PersistedCoolingDown { reason, .. } => Some(reason.clone()),
            Self::PersistedMemory { reason } => Some(reason.clone()),
        }
    }

    fn retryable(&self) -> bool {
        !matches!(self, Self::Disabled { .. } | Self::PersistedMemory { .. })
    }
}

#[derive(Debug, Clone, Default)]
struct PersistedProviderHints {
    skip_reasons: HashMap<String, ProviderSkipReason>,
}

#[derive(Debug, Deserialize)]
struct PersistedProviderResponseEnvelope {
    #[serde(default)]
    candidate_records: Vec<CandidateRecord>,
}

#[derive(Debug, Clone)]
pub struct DirectorSubmission {
    tx: mpsc::Sender<TrackTask>,
}

impl DirectorSubmission {
    pub async fn submit(&self, task: TrackTask) -> Result<(), DirectorError> {
        self.tx
            .send(task)
            .await
            .map_err(|error| DirectorError::Queue(error.to_string()))
    }
}

pub struct DirectorHandle {
    pub submitter: DirectorSubmission,
    pub events: broadcast::Sender<DirectorEvent>,
    pub results: broadcast::Sender<DirectorTaskResult>,
    pub provider_health: broadcast::Sender<ProviderHealthState>,
    cancel_token: CancellationToken,
    task_tokens: Arc<std::sync::Mutex<HashMap<String, CancellationToken>>>,
    manager: tokio::task::JoinHandle<Result<(), DirectorError>>,
}

impl DirectorHandle {
    pub fn subscribe(&self) -> broadcast::Receiver<DirectorEvent> {
        self.events.subscribe()
    }

    pub fn subscribe_results(&self) -> broadcast::Receiver<DirectorTaskResult> {
        self.results.subscribe()
    }

    pub fn subscribe_health(&self) -> broadcast::Receiver<ProviderHealthState> {
        self.provider_health.subscribe()
    }

    pub fn cancel_task(&self, task_id: &str) -> bool {
        self.task_tokens
            .lock()
            .map(|tokens| tokens.get(task_id).cloned())
            .ok()
            .flatten()
            .map(|token| {
                token.cancel();
                true
            })
            .unwrap_or(false)
    }

    pub fn cancel_batch(&self) {
        self.cancel_token.cancel();
    }

    pub async fn shutdown(self) -> Result<(), DirectorError> {
        let DirectorHandle {
            submitter,
            events: _,
            results: _,
            provider_health: _,
            cancel_token,
            task_tokens: _,
            manager,
        } = self;
        cancel_token.cancel();
        drop(submitter);
        manager.await?
    }
}

pub struct Director {
    config: DirectorConfig,
    providers: Vec<Arc<dyn Provider>>,
    planner: StrategyPlanner,
}

impl Director {
    pub fn new(config: DirectorConfig, providers: Vec<Arc<dyn Provider>>) -> Self {
        Self {
            config,
            providers,
            planner: StrategyPlanner,
        }
    }

    pub async fn recover_temp(&self) -> Result<(), DirectorError> {
        let manager = TempManager::new(
            self.config.temp_root.clone(),
            self.config.temp_recovery.clone(),
        );
        let summary = manager.recover_stale().await?;
        info!(
            deleted = summary.deleted_roots.len(),
            preserved = summary.preserved_quarantine.len(),
            "director temp recovery completed"
        );
        Ok(())
    }

    pub fn start(self) -> DirectorHandle {
        let (tx, rx) = mpsc::channel::<TrackTask>(512);
        let (events, _) = broadcast::channel::<DirectorEvent>(1024);
        let (results, _) = broadcast::channel::<DirectorTaskResult>(4096);
        let (provider_health, _) = broadcast::channel::<ProviderHealthState>(64);
        let cancel_token = CancellationToken::new();
        let tracker = Arc::new(TaskTracker::new());
        let task_tokens = Arc::new(std::sync::Mutex::new(HashMap::new()));
        let manager = tokio::spawn(self.run(
            rx,
            events.clone(),
            results.clone(),
            provider_health.clone(),
            cancel_token.clone(),
            Arc::clone(&task_tokens),
            Arc::clone(&tracker),
        ));

        DirectorHandle {
            submitter: DirectorSubmission { tx },
            events,
            results,
            provider_health,
            cancel_token,
            task_tokens,
            manager,
        }
    }

    async fn run(
        self,
        mut rx: mpsc::Receiver<TrackTask>,
        events: broadcast::Sender<DirectorEvent>,
        results: broadcast::Sender<DirectorTaskResult>,
        provider_health: broadcast::Sender<ProviderHealthState>,
        cancel_token: CancellationToken,
        task_tokens: Arc<std::sync::Mutex<HashMap<String, CancellationToken>>>,
        task_tracker: Arc<TaskTracker>,
    ) -> Result<(), DirectorError> {
        self.recover_temp().await?;

        let semaphore = Arc::new(Semaphore::new(self.config.worker_concurrency.max(1)));
        let provider_limits = self.build_provider_limits();
        let provider_health_state = Arc::new(RwLock::new(HashMap::<String, ProviderHealthState>::new()));
        let provider_runtime_state = Arc::new(RwLock::new(HashMap::<String, ProviderRuntimeState>::new()));
        let provider_cache_epochs = Arc::new(RwLock::new(HashMap::<String, u64>::new()));
        let search_cache = Arc::new(
            Cache::builder()
                .max_capacity(self.config.search_cache_capacity.max(1))
                .time_to_live(std::time::Duration::from_secs(
                    self.config.search_cache_ttl_secs.max(1),
                ))
                .build(),
        );
        let health_token = cancel_token.child_token();
        let health_providers = self.providers.clone();
        let health_config = self.config.clone();
        let health_state = Arc::clone(&provider_health_state);
        let health_cache_epochs = Arc::clone(&provider_cache_epochs);
        let health_events = provider_health.clone();
        task_tracker.spawn(async move {
            run_provider_health_loop(
                health_providers,
                health_config,
                health_state,
                health_cache_epochs,
                health_events,
                health_token,
            )
            .await;
        });
        loop {
            tokio::select! {
                _ = cancel_token.cancelled() => {
                    info!("director cancellation requested");
                    break;
                }
                maybe_task = rx.recv() => {
                    let Some(task) = maybe_task else {
                        break;
                    };

                    send_event(&events, &task.task_id, DirectorProgress::Queued, None, "task queued");
                    let task_token = cancel_token.child_token();
                    if let Ok(mut tokens) = task_tokens.lock() {
                        tokens.insert(task.task_id.clone(), task_token.clone());
                    }
                    let permit = tokio::select! {
                        _ = cancel_token.cancelled() => {
                            info!(task_id = %task.task_id, "skipping task scheduling because cancellation was requested");
                            break;
                        }
                        _ = task_token.cancelled() => {
                            send_event(
                                &events,
                                &task.task_id,
                                DirectorProgress::Cancelled,
                                None,
                                "task cancelled while queued",
                            );
                            let _ = results.send(DirectorTaskResult {
                                task_id: task.task_id.clone(),
                                disposition: FinalizedTrackDisposition::Cancelled,
                                finalized: None,
                                attempts: Vec::new(),
                                error: Some("task cancelled".to_string()),
                                candidate_records: Vec::new(),
                                provider_searches: Vec::new(),
                            });
                            unregister_task_token(&task_tokens, &task.task_id);
                            continue;
                        }
                        permit = semaphore.clone().acquire_owned() => permit
                            .map_err(|error| DirectorError::Queue(error.to_string()))?,
                    };

                    if cancel_token.is_cancelled() {
                        break;
                    }

                    let config = self.config.clone();
                    let providers = self.providers.clone();
                    let planner = self.planner.clone();
                    let search_cache_clone = Arc::clone(&search_cache);
                    let provider_cache_epochs_clone = Arc::clone(&provider_cache_epochs);
                    let provider_health_state_clone = Arc::clone(&provider_health_state);
                    let provider_runtime_state_clone = Arc::clone(&provider_runtime_state);
                    let events_clone = events.clone();
                    let results_clone = results.clone();
                    let provider_limits_clone = provider_limits.clone();
                    let task_tokens_clone = Arc::clone(&task_tokens);

                    task_tracker.spawn(async move {
                        let _permit = permit;
                        let _ = process_task(
                            config,
                            providers,
                            planner,
                            search_cache_clone,
                            provider_cache_epochs_clone,
                            provider_health_state_clone,
                            provider_runtime_state_clone,
                            provider_limits_clone,
                            events_clone,
                            results_clone,
                            task_token,
                            task_tokens_clone,
                            task,
                        )
                        .await;
                    });
                }
            }
        }

        task_tracker.close();
        task_tracker.wait().await;

        if cancel_token.is_cancelled() {
            return Ok(());
        }

        Ok(())
    }

    fn build_provider_limits(&self) -> HashMap<String, Arc<Semaphore>> {
        self.providers
            .iter()
            .map(|provider| {
                let descriptor = provider.descriptor();
                let default_concurrency = match descriptor.id.as_str() {
                    "slskd" => 2,       // slskd can handle 2 concurrent searches
                    "qobuz" => 4,       // streaming API, handles concurrency well
                    "deezer" => 4,      // streaming API, handles concurrency well
                    "local_archive" => 8, // filesystem — fast, parallelizes well
                    _ => 3,             // sensible default for network providers
                };
                let limit = self
                    .config
                    .provider_policy(&descriptor.id)
                    .map(|policy| policy.max_concurrency.max(1))
                    .unwrap_or(default_concurrency);
                (descriptor.id, Arc::new(Semaphore::new(limit)))
            })
            .collect()
    }
}

async fn process_task(
    config: DirectorConfig,
    providers: Vec<Arc<dyn Provider>>,
    planner: StrategyPlanner,
    search_cache: Arc<Cache<String, Arc<Vec<ProviderSearchCandidate>>>>,
    provider_cache_epochs: Arc<RwLock<HashMap<String, u64>>>,
    provider_health_state: Arc<RwLock<HashMap<String, ProviderHealthState>>>,
    provider_runtime_state: Arc<RwLock<HashMap<String, ProviderRuntimeState>>>,
    provider_limits: HashMap<String, Arc<Semaphore>>,
    events: broadcast::Sender<DirectorEvent>,
    results: broadcast::Sender<DirectorTaskResult>,
    cancel_token: CancellationToken,
    task_tokens: Arc<std::sync::Mutex<HashMap<String, CancellationToken>>>,
    task: TrackTask,
) -> Result<(), DirectorError> {
    let provider_descriptors = providers
        .iter()
        .map(|provider| provider.descriptor())
        .collect::<Vec<ProviderDescriptor>>();
    let plan = planner.plan(&task, &provider_descriptors, &config);

    send_event(
        &events,
        &task.task_id,
        DirectorProgress::InProgress,
        None,
        &format!("strategy {:?} selected", plan.strategy),
    );

    if cancel_token.is_cancelled() {
        send_event(
            &events,
            &task.task_id,
            DirectorProgress::Cancelled,
            None,
            "task cancelled before execution",
        );
        let _ = results.send(DirectorTaskResult {
            task_id: task.task_id.clone(),
            disposition: FinalizedTrackDisposition::Cancelled,
            finalized: None,
            attempts: Vec::new(),
            error: Some("task cancelled".to_string()),
            candidate_records: Vec::new(),
            provider_searches: Vec::new(),
        });
        unregister_task_token(&task_tokens, &task.task_id);
        return Ok(());
    }

    if matches!(task.strategy, crate::director::models::AcquisitionStrategy::MetadataRepairOnly) {
        send_event(
            &events,
            &task.task_id,
            DirectorProgress::Skipped,
            None,
            "metadata repair only is intentionally stubbed in phase 1",
        );
        let _ = results.send(DirectorTaskResult {
            task_id: task.task_id.clone(),
            disposition: FinalizedTrackDisposition::MetadataOnly,
            finalized: None,
            attempts: Vec::new(),
            error: None,
            candidate_records: Vec::new(),
            provider_searches: Vec::new(),
        });
        unregister_task_token(&task_tokens, &task.task_id);
        return Ok(());
    }

    let temp_manager = TempManager::new(config.temp_root.clone(), config.temp_recovery.clone());
    let temp_context = match tokio::select! {
        _ = cancel_token.cancelled() => {
            send_event(
                &events,
                &task.task_id,
                DirectorProgress::Cancelled,
                None,
                "task cancelled before temp staging",
            );
            let _ = results.send(DirectorTaskResult {
                task_id: task.task_id.clone(),
                disposition: FinalizedTrackDisposition::Cancelled,
                finalized: None,
                attempts: Vec::new(),
                error: Some("task cancelled".to_string()),
                candidate_records: Vec::new(),
                provider_searches: Vec::new(),
            });
            unregister_task_token(&task_tokens, &task.task_id);
            return Ok(());
        }
        ctx = temp_manager.prepare_task(&task.task_id) => ctx
    } {
        Ok(ctx) => ctx,
        Err(error) => {
            let msg = format!("failed to create temp dir: {error}");
            warn!(task_id = task.task_id, error = %error, "director task failed to create temp dir");
            let _ = results.send(DirectorTaskResult {
                task_id: task.task_id.clone(),
                disposition: FinalizedTrackDisposition::Failed,
                finalized: None,
                attempts: Vec::new(),
                error: Some(msg.clone()),
                candidate_records: Vec::new(),
                provider_searches: Vec::new(),
            });
            unregister_task_token(&task_tokens, &task.task_id);
            return Err(DirectorError::Queue(msg));
        }
    };
    let mut attempts = Vec::<ProviderAttemptRecord>::new();
    let mut candidate_records = Vec::<CandidateRecord>::new();
    let mut provider_searches = Vec::<ProviderSearchRecord>::new();
    let result = execute_waterfall(
        &config,
        &providers,
        &provider_limits,
        &search_cache,
        &provider_cache_epochs,
        &provider_health_state,
        &provider_runtime_state,
        &events,
        &task,
        &plan,
        &temp_manager,
        &temp_context,
        &cancel_token,
        &mut attempts,
        &mut candidate_records,
        &mut provider_searches,
    )
    .await;

    match result {
        Ok(finalized) => {
            send_event(
                &events,
                &task.task_id,
                DirectorProgress::Finalized,
                Some(finalized.provenance.selected_provider.clone()),
                &format!("finalized to {}", finalized.path.display()),
            );
            let _ = results.send(DirectorTaskResult {
                task_id: task.task_id.clone(),
                disposition: FinalizedTrackDisposition::Finalized,
                finalized: Some(finalized.clone()),
                attempts,
                error: None,
                candidate_records,
                provider_searches,
            });
            temp_manager.cleanup_task(&temp_context).await?;
            unregister_task_token(&task_tokens, &task.task_id);
            Ok(())
        }
        Err(DirectorError::TaskCancelled) => {
            send_event(
                &events,
                &task.task_id,
                DirectorProgress::Cancelled,
                None,
                "task cancelled",
            );
            let _ = results.send(DirectorTaskResult {
                task_id: task.task_id.clone(),
                disposition: FinalizedTrackDisposition::Cancelled,
                finalized: None,
                attempts,
                error: Some("task cancelled".to_string()),
                candidate_records,
                provider_searches,
            });
            let _ = temp_manager.cleanup_task(&temp_context).await;
            unregister_task_token(&task_tokens, &task.task_id);
            Ok(())
        }
        Err(error) => {
            warn!(task_id = task.task_id, error = %error, "director task failed");
            send_event(
                &events,
                &task.task_id,
                DirectorProgress::Failed,
                None,
                &error.to_string(),
            );
            let disposition = match &error {
                DirectorError::Finalization(crate::director::error::FinalizationError::DestinationExists { .. }) => {
                    FinalizedTrackDisposition::AlreadyPresent
                }
                _ => FinalizedTrackDisposition::Failed,
            };
            let _ = results.send(DirectorTaskResult {
                task_id: task.task_id.clone(),
                disposition,
                finalized: None,
                attempts,
                error: Some(error.to_string()),
                candidate_records,
                provider_searches,
            });
            if !config.temp_recovery.quarantine_failures {
                temp_manager.cleanup_task(&temp_context).await?;
            }
            unregister_task_token(&task_tokens, &task.task_id);
            Err(error)
        }
    }
}

/// Outcome of attempting a single provider in the waterfall.
enum ProviderAttemptOutcome {
    /// Provider was busy (semaphore full) - should be deferred for second pass.
    Busy,
    /// Provider was tried but produced no usable result (error, validation fail, etc.).
    Tried,
    /// Provider produced a valid candidate that was immediately finalized (FirstValidWins).
    Finalized(FinalizedTrack),
}

/// Try a single provider: search → acquire → validate → score.
/// If `blocking` is false, uses try_acquire (non-blocking) on the provider semaphore.
/// Returns `Busy` if the semaphore is full and `blocking` is false.
#[allow(clippy::too_many_arguments)]
async fn try_provider(
    provider: &dyn Provider,
    provider_id: &str,
    provider_order_index: usize,
    limit: &Arc<Semaphore>,
    blocking: bool,
    config: &DirectorConfig,
    search_cache: &Arc<Cache<String, Arc<Vec<ProviderSearchCandidate>>>>,
    provider_cache_epochs: &Arc<RwLock<HashMap<String, u64>>>,
    provider_runtime_state: &Arc<RwLock<HashMap<String, ProviderRuntimeState>>>,
    events: &broadcast::Sender<DirectorEvent>,
    task: &TrackTask,
    plan: &StrategyPlan,
    temp_manager: &TempManager,
    temp_context: &TaskTempContext,
    cancel_token: &CancellationToken,
    valid_candidates: &mut Vec<(CandidateDisposition, ProviderDescriptor)>,
    attempts: &mut Vec<ProviderAttemptRecord>,
    candidate_records: &mut Vec<CandidateRecord>,
    provider_searches: &mut Vec<ProviderSearchRecord>,
) -> Result<ProviderAttemptOutcome, DirectorError> {
    let descriptor = provider.descriptor();
    if !descriptor.capabilities.supports_download {
        attempts.push(ProviderAttemptRecord {
            provider_id: provider_id.to_string(),
            attempt: 0,
            outcome: "skipped: provider is metadata-only".to_string(),
        });
        provider_searches.push(ProviderSearchRecord {
            provider_id: provider_id.to_string(),
            provider_display_name: descriptor.display_name.clone(),
            provider_trust_rank: descriptor.trust_rank,
            provider_order_index,
            outcome: "metadata_only".to_string(),
            candidate_count: 0,
            error: None,
            retryable: false,
        });
        return Ok(ProviderAttemptOutcome::Tried);
    }

    send_event(
        events,
        &task.task_id,
        DirectorProgress::ProviderAttempt,
        Some(provider_id.to_string()),
        "searching provider",
    );

    let search_candidates = match execute_provider_search(
        provider,
        task,
        plan,
        config,
        search_cache,
        provider_cache_epochs,
        Arc::clone(limit),
        blocking,
        cancel_token,
    )
    .await
    {
        Ok(candidates) => candidates,
        Err(DirectorError::Provider(error)) if error.is_busy() => {
            apply_provider_runtime_error(config, provider_runtime_state, provider_id, &error).await;
            provider_searches.push(ProviderSearchRecord {
                provider_id: provider_id.to_string(),
                provider_display_name: descriptor.display_name.clone(),
                provider_trust_rank: descriptor.trust_rank,
                provider_order_index,
                outcome: "busy".to_string(),
                candidate_count: 0,
                error: Some(error.to_string()),
                retryable: true,
            });
            return Ok(ProviderAttemptOutcome::Busy)
        }
        Err(DirectorError::TaskCancelled) => return Err(DirectorError::TaskCancelled),
        Err(error) => {
            if let DirectorError::Provider(provider_error) = &error {
                apply_provider_runtime_error(config, provider_runtime_state, provider_id, provider_error).await;
            }
            let retryable = matches!(&error, DirectorError::Provider(provider_error) if provider_error.retryable());
            attempts.push(ProviderAttemptRecord {
                provider_id: provider_id.to_string(),
                attempt: 1,
                outcome: error.to_string(),
            });
            provider_searches.push(ProviderSearchRecord {
                provider_id: provider_id.to_string(),
                provider_display_name: descriptor.display_name.clone(),
                provider_trust_rank: descriptor.trust_rank,
                provider_order_index,
                outcome: "search_error".to_string(),
                candidate_count: 0,
                error: Some(error.to_string()),
                retryable,
            });
            return Ok(ProviderAttemptOutcome::Tried);
        }
    };

    provider_searches.push(ProviderSearchRecord {
        provider_id: provider_id.to_string(),
        provider_display_name: descriptor.display_name.clone(),
        provider_trust_rank: descriptor.trust_rank,
        provider_order_index,
        outcome: if search_candidates.is_empty() {
            "no_candidates".to_string()
        } else {
            "candidates_found".to_string()
        },
        candidate_count: search_candidates.len(),
        error: None,
        retryable: false,
    });
    clear_provider_runtime_cooldown(provider_runtime_state, provider_id).await;

    let mut validation_failures = 0usize;
    for (search_rank, candidate) in search_candidates.into_iter().enumerate() {
        match execute_provider_acquire(
            provider,
            task,
            &candidate,
            temp_context,
            plan,
            config,
            Arc::clone(limit),
            blocking,
            cancel_token,
        )
        .await
        {
            Ok(acquisition) => {
                send_event(
                    events,
                    &task.task_id,
                    DirectorProgress::Validating,
                    Some(provider_id.to_string()),
                    "validating candidate",
                );

                match validate_candidate(
                    acquisition.temp_path.clone(),
                    task.target.clone(),
                    config.quality_policy.clone(),
                )
                .await
                {
                    Ok(validation) => {
                        let (score, score_reason) = score_candidate(
                            &task.target,
                            &descriptor,
                            &candidate,
                            &validation,
                            &config.quality_policy,
                        );
                        let disposition = CandidateDisposition {
                            candidate: candidate.clone(),
                            acquisition: acquisition.clone(),
                            validation: validation.clone(),
                            score,
                            score_reason: score_reason.clone(),
                        };
                        attempts.push(ProviderAttemptRecord {
                            provider_id: provider_id.to_string(),
                            attempt: 1,
                            outcome: format!("valid candidate score {}", disposition.score.total),
                        });
                        candidate_records.push(CandidateRecord {
                            provider_id: provider_id.to_string(),
                            provider_display_name: descriptor.display_name.clone(),
                            provider_trust_rank: descriptor.trust_rank,
                            provider_order_index,
                            search_rank,
                            candidate,
                            acquisition_temp_path: Some(acquisition.temp_path),
                            validation: Some(validation),
                            score: Some(disposition.score.clone()),
                            score_reason: Some(score_reason),
                            outcome: if plan.collect_multiple_candidates {
                                "valid_candidate".to_string()
                            } else {
                                "selected_immediate".to_string()
                            },
                            rejection_reason: None,
                        });

                        if !plan.collect_multiple_candidates {
                            let result = finalize_candidate(
                                config,
                                events,
                                task,
                                descriptor,
                                disposition,
                                attempts.len(),
                            )
                            .await?;
                            clear_provider_runtime_cooldown(provider_runtime_state, provider_id).await;
                            return Ok(ProviderAttemptOutcome::Finalized(result));
                        }

                        // Quality-gated compare mode: if the first provider already
                        // delivered a lossless validated candidate, finalize immediately.
                        if plan.compare_after_first_quality_gate
                            && provider_order_index == 0
                            && matches!(disposition.validation.quality, CandidateQuality::Lossless)
                        {
                            let result = finalize_candidate(
                                config,
                                events,
                                task,
                                descriptor,
                                disposition,
                                attempts.len(),
                            )
                            .await?;
                            clear_provider_runtime_cooldown(provider_runtime_state, provider_id).await;
                            return Ok(ProviderAttemptOutcome::Finalized(result));
                        }

                        valid_candidates.push((disposition, descriptor.clone()));
                        clear_provider_runtime_cooldown(provider_runtime_state, provider_id).await;
                    }
                    Err(error) => {
                        validation_failures += 1;
                        if config.temp_recovery.quarantine_failures {
                            let _ = temp_manager
                                .quarantine_file(temp_context, &acquisition.temp_path)
                                .await;
                        } else {
                            let _ = tokio::fs::remove_file(&acquisition.temp_path).await;
                        }
                        attempts.push(ProviderAttemptRecord {
                            provider_id: provider_id.to_string(),
                            attempt: 1,
                            outcome: format!("validation failed: {error}"),
                        });
                        candidate_records.push(CandidateRecord {
                            provider_id: provider_id.to_string(),
                            provider_display_name: descriptor.display_name.clone(),
                            provider_trust_rank: descriptor.trust_rank,
                            provider_order_index,
                            search_rank,
                            candidate,
                            acquisition_temp_path: Some(acquisition.temp_path),
                            validation: None,
                            score: None,
                            score_reason: None,
                            outcome: "validation_failed".to_string(),
                            rejection_reason: Some(error.to_string()),
                        });
                        if validation_failures >= config.validation_failure_bail_threshold {
                            attempts.push(ProviderAttemptRecord {
                                provider_id: provider_id.to_string(),
                                attempt: 1,
                                outcome: format!(
                                    "stopping after {} validation failures",
                                    config.validation_failure_bail_threshold
                                ),
                            });
                            break;
                        }
                    }
                }
            }
            Err(DirectorError::Provider(error)) if error.is_busy() => {
                apply_provider_runtime_error(config, provider_runtime_state, provider_id, &error).await;
                provider_searches.push(ProviderSearchRecord {
                    provider_id: provider_id.to_string(),
                    provider_display_name: descriptor.display_name.clone(),
                    provider_trust_rank: descriptor.trust_rank,
                    provider_order_index,
                    outcome: "busy".to_string(),
                    candidate_count: 0,
                    error: Some(error.to_string()),
                    retryable: true,
                });
                return Ok(ProviderAttemptOutcome::Busy)
            }
            Err(error) => {
                if let DirectorError::Provider(provider_error) = &error {
                    apply_provider_runtime_error(config, provider_runtime_state, provider_id, provider_error).await;
                }
                attempts.push(ProviderAttemptRecord {
                    provider_id: provider_id.to_string(),
                    attempt: 1,
                    outcome: error.to_string(),
                });
                let retryable = matches!(&error, DirectorError::Provider(provider_error) if provider_error.retryable());
                candidate_records.push(CandidateRecord {
                    provider_id: provider_id.to_string(),
                    provider_display_name: descriptor.display_name.clone(),
                    provider_trust_rank: descriptor.trust_rank,
                    provider_order_index,
                    search_rank,
                    candidate,
                    acquisition_temp_path: None,
                    validation: None,
                    score: None,
                    score_reason: None,
                    outcome: "acquire_failed".to_string(),
                    rejection_reason: Some(error.to_string()),
                });
                if retryable {
                    provider_searches.push(ProviderSearchRecord {
                        provider_id: provider_id.to_string(),
                        provider_display_name: descriptor.display_name.clone(),
                        provider_trust_rank: descriptor.trust_rank,
                        provider_order_index,
                        outcome: "acquire_retryable_error".to_string(),
                        candidate_count: 0,
                        error: Some(error.to_string()),
                        retryable,
                    });
                }
            }
        }
    }

    Ok(ProviderAttemptOutcome::Tried)
}

/// Two-pass waterfall with optional parallel search prefetch. When the selection mode
/// is CompareTopN(n), the top N providers are searched concurrently to warm the cache
/// before the sequential waterfall begins. This eliminates sequential search latency
/// for the most common providers while preserving the existing acquire/validate flow.
#[allow(clippy::too_many_arguments)]
async fn execute_waterfall(
    config: &DirectorConfig,
    providers: &[Arc<dyn Provider>],
    provider_limits: &HashMap<String, Arc<Semaphore>>,
    search_cache: &Arc<Cache<String, Arc<Vec<ProviderSearchCandidate>>>>,
    provider_cache_epochs: &Arc<RwLock<HashMap<String, u64>>>,
    provider_health_state: &Arc<RwLock<HashMap<String, ProviderHealthState>>>,
    provider_runtime_state: &Arc<RwLock<HashMap<String, ProviderRuntimeState>>>,
    events: &broadcast::Sender<DirectorEvent>,
    task: &TrackTask,
    plan: &StrategyPlan,
    temp_manager: &TempManager,
    temp_context: &TaskTempContext,
    cancel_token: &CancellationToken,
    attempts: &mut Vec<ProviderAttemptRecord>,
    candidate_records: &mut Vec<CandidateRecord>,
    provider_searches: &mut Vec<ProviderSearchRecord>,
) -> Result<FinalizedTrack, DirectorError> {
    let provider_map: HashMap<String, Arc<dyn Provider>> = providers
        .iter()
        .map(|provider| (provider.descriptor().id.clone(), Arc::clone(provider)))
        .collect();
    let persisted_hints = load_persisted_provider_hints(
        config,
        task,
        search_cache,
        provider_cache_epochs,
    )
    .await;
    let mut valid_candidates = Vec::<(CandidateDisposition, ProviderDescriptor)>::new();
    let mut deferred_providers = Vec::<String>::new();

    // Parallel search prefetch: for CompareTopN, fire searches for top N providers
    // concurrently. Results are cached by moka so the sequential waterfall below
    // will hit cache and skip the network call entirely.
    if let CandidateSelectionMode::CompareTopN(n) = plan.selection_mode {
        if plan.compare_after_first_quality_gate {
            // Quality-gated compare mode intentionally evaluates the first provider
            // before deciding whether to compare, so prefetch is skipped.
        } else {
        let prefetch_count = n.min(plan.provider_order.len());
        let mut prefetch_set = tokio::task::JoinSet::new();

        for provider_id in plan.provider_order.iter().take(prefetch_count) {
            if should_skip_provider(
                config,
                provider_health_state,
                provider_runtime_state,
                &persisted_hints.skip_reasons,
                provider_id,
            )
            .await
            .is_some()
            {
                continue;
            }
            let Some(provider) = provider_map.get(provider_id).cloned() else { continue };
            let Some(limit) = provider_limits.get(provider_id).cloned() else { continue };

            let cache_key = provider_search_cache_key(provider_cache_epochs, &provider_id, task).await;
            if search_cache.get(&cache_key).is_some() {
                continue; // already cached
            }

            let config_clone = config.clone();
            let task_clone = task.clone();
            let plan_clone = plan.clone();
            let cancel_clone = cancel_token.clone();
            let cache_clone = Arc::clone(search_cache);
            let provider_cache_epochs_clone = Arc::clone(provider_cache_epochs);
            let provider_id_clone = provider_id.clone();

            prefetch_set.spawn(async move {
                // Acquire provider permit (blocking — these are prefetch, we want them all)
                let permit = match limit.clone().try_acquire_owned() {
                    Ok(permit) => permit,
                    Err(_) => match limit.acquire_owned().await {
                        Ok(permit) => permit,
                        Err(_) => return,
                    },
                };
                let result = execute_with_retry(
                    &config_clone,
                    provider_id_clone.clone(),
                    &cancel_clone,
                    config_clone.search_timeout(),
                    || provider.search(&task_clone, &plan_clone),
                ).await;
                drop(permit);
                if let Ok(results) = result {
                    let key = provider_search_cache_key(
                        &provider_cache_epochs_clone,
                        &provider_id_clone,
                        &task_clone,
                    )
                    .await;
                    cache_clone.insert(key, Arc::new(results));
                }
            });
        }

        // Wait for all prefetch searches (with cancellation support)
        tokio::select! {
            _ = cancel_token.cancelled() => {
                return Err(DirectorError::TaskCancelled);
            }
            _ = async {
                while prefetch_set.join_next().await.is_some() {}
            } => {}
        }
        }
    }

    // Pass 1: Non-blocking — try each provider, skip if semaphore is full
    for (provider_order_index, provider_id) in plan.provider_order.iter().enumerate() {
        if let Some(reason) = should_skip_provider(
            config,
            provider_health_state,
            provider_runtime_state,
            &persisted_hints.skip_reasons,
            provider_id,
        )
        .await
        {
            record_provider_skip(
                attempts,
                provider_searches,
                provider_id,
                provider_order_index,
                &reason,
            );
            continue;
        }
        let Some(provider) = provider_map.get(provider_id) else {
            continue;
        };
        let Some(limit) = provider_limits.get(provider_id) else {
            continue;
        };

        match try_provider(
            provider.as_ref(),
            provider_id,
            provider_order_index,
            limit,
            false, // non-blocking
            config,
            search_cache,
            provider_cache_epochs,
            provider_runtime_state,
            events,
            task,
            plan,
            temp_manager,
            temp_context,
            cancel_token,
            &mut valid_candidates,
            attempts,
            candidate_records,
            provider_searches,
        )
        .await?
        {
            ProviderAttemptOutcome::Finalized(track) => {
                return Ok(track);
            }
            ProviderAttemptOutcome::Busy => {
                deferred_providers.push(provider_id.clone());
            }
            ProviderAttemptOutcome::Tried => {}
        }
    }

    // CompareTopN early exit: if we already have valid candidates from the top N
    // providers, skip remaining providers and finalize the best one.
    if let CandidateSelectionMode::CompareTopN(_) = plan.selection_mode {
        if !valid_candidates.is_empty() {
            let available_candidates: Vec<_> = valid_candidates
                .into_iter()
                .filter(|(candidate, _)| candidate.acquisition.temp_path.exists())
                .collect();
            if let Some((best, descriptor)) = available_candidates
                .into_iter()
                .max_by_key(|(candidate, _)| candidate.score.total)
            {
                return finalize_candidate(config, events, task, descriptor, best, attempts.len()).await;
            }
            // If all temp files gone, fall through to remaining providers
            valid_candidates = Vec::new();
        }
    }

    // Pass 2: Blocking — try deferred providers (ones that were busy in pass 1)
    for provider_id in &deferred_providers {
        let Some(provider_order_index) = plan.provider_order.iter().position(|id| id == provider_id) else {
            continue;
        };
        if let Some(reason) = should_skip_provider(
            config,
            provider_health_state,
            provider_runtime_state,
            &persisted_hints.skip_reasons,
            provider_id,
        )
        .await
        {
            record_provider_skip(
                attempts,
                provider_searches,
                provider_id,
                provider_order_index,
                &reason,
            );
            continue;
        }
        let Some(provider) = provider_map.get(provider_id) else {
            continue;
        };
        let Some(limit) = provider_limits.get(provider_id) else {
            continue;
        };

        match try_provider(
            provider.as_ref(),
            provider_id,
            provider_order_index,
            limit,
            true, // blocking
            config,
            search_cache,
            provider_cache_epochs,
            provider_runtime_state,
            events,
            task,
            plan,
            temp_manager,
            temp_context,
            cancel_token,
            &mut valid_candidates,
            attempts,
            candidate_records,
            provider_searches,
        )
        .await?
        {
            ProviderAttemptOutcome::Finalized(track) => {
                return Ok(track);
            }
            ProviderAttemptOutcome::Busy | ProviderAttemptOutcome::Tried => {}
        }
    }

    // Filter out candidates whose temp file no longer exists (can happen when multiple
    // candidates from the same provider share a filename — a later failure quarantines
    // the shared path, invalidating an earlier valid candidate's temp_path).
    let available_candidates: Vec<_> = valid_candidates
        .into_iter()
        .filter(|(candidate, _)| candidate.acquisition.temp_path.exists())
        .collect();

    if let Some((best, descriptor)) = available_candidates
        .into_iter()
        .max_by_key(|(candidate, _)| candidate.score.total)
    {
        finalize_candidate(config, events, task, descriptor, best, attempts.len()).await
    } else {
        send_event(
            events,
            &task.task_id,
            DirectorProgress::Exhausted,
            None,
            "all providers exhausted",
        );
        Err(DirectorError::ProviderExhausted {
            task_id: task.task_id.clone(),
        })
    }
}

async fn finalize_candidate(
    config: &DirectorConfig,
    events: &broadcast::Sender<DirectorEvent>,
    task: &TrackTask,
    descriptor: ProviderDescriptor,
    best: CandidateDisposition,
    attempts_len: usize,
) -> Result<FinalizedTrack, DirectorError> {
    let effective_target = merge_normalized_track(&task.target, best.acquisition.resolved_metadata.as_ref());
    let mut effective_task = task.clone();
    effective_task.target = effective_target.clone();

    let selection = CandidateSelection {
        provider_id: best.candidate.provider_id.clone(),
        provider_candidate_id: best.candidate.provider_candidate_id.clone(),
        temp_path: best.acquisition.temp_path.clone(),
        score: best.score,
        reason: best.score_reason,
        validation: best.validation,
        cover_art_url: best.candidate.cover_art_url.clone(),
    };

    send_event(
        events,
        &task.task_id,
        DirectorProgress::Tagging,
        Some(selection.provider_id.clone()),
        "applying metadata",
    );
    if let Err(error) = apply_metadata(effective_task.clone(), selection.clone()).await {
        warn!(task_id = %task.task_id, error = %error, "metadata tagging failed — continuing to finalization without tags");
    }

    send_event(
        events,
        &task.task_id,
        DirectorProgress::Finalizing,
        Some(selection.provider_id.clone()),
        "moving candidate into library",
    );
    let provenance = ProvenanceRecord {
        task_id: task.task_id.clone(),
        source_metadata: effective_target.clone(),
        selected_provider: selection.provider_id.clone(),
        selected_provider_candidate_id: Some(selection.provider_candidate_id.clone()),
        score_reason: selection.reason.clone(),
        validation_summary: selection.validation.clone(),
        final_path: Default::default(),
        acquired_at: Utc::now(),
    };
    let finalized = finalize_selected_candidate(
        config.library_root.clone(),
        selection,
        effective_target,
        config.duplicate_policy,
        provenance,
    )
    .await;

    match finalized {
        Ok(track) => {
            info!(
                task_id = task.task_id,
                provider = track.provenance.selected_provider,
                attempts = attempts_len,
                "director finalized track"
            );
            Ok(track)
        }
        Err(error) => match (&config.duplicate_policy, &error) {
            (DuplicatePolicy::KeepExisting, crate::director::error::FinalizationError::DestinationExists { .. }) => {
                send_event(
                    events,
                    &task.task_id,
                    DirectorProgress::Skipped,
                    Some(descriptor.id),
                    "matching file already present",
                );
                Err(DirectorError::Finalization(error))
            }
            _ => Err(DirectorError::Finalization(error)),
        },
    }
}

/// Acquire a provider semaphore permit. If `blocking` is false, returns
/// `ProviderError::ProviderBusy` immediately when the semaphore is full
/// instead of waiting. This enables the two-pass waterfall to skip busy
/// providers and try the next one.
async fn acquire_provider_permit(
    provider_limit: &Arc<Semaphore>,
    provider_id: &str,
    blocking: bool,
) -> Result<tokio::sync::OwnedSemaphorePermit, ProviderError> {
    if blocking {
        provider_limit
            .clone()
            .acquire_owned()
            .await
            .map_err(|error| ProviderError::Other {
                provider_id: provider_id.to_string(),
                message: error.to_string(),
            })
    } else {
        provider_limit
            .clone()
            .try_acquire_owned()
            .map_err(|_| ProviderError::ProviderBusy {
                provider_id: provider_id.to_string(),
            })
    }
}

async fn execute_provider_search(
    provider: &dyn Provider,
    task: &TrackTask,
    plan: &StrategyPlan,
    config: &DirectorConfig,
    search_cache: &Arc<Cache<String, Arc<Vec<ProviderSearchCandidate>>>>,
    provider_cache_epochs: &Arc<RwLock<HashMap<String, u64>>>,
    provider_limit: Arc<Semaphore>,
    blocking: bool,
    cancel_token: &CancellationToken,
) -> Result<Vec<ProviderSearchCandidate>, DirectorError> {
    let cache_key = provider_search_cache_key(provider_cache_epochs, &provider.descriptor().id, task).await;
    if let Some(cached) = search_cache.get(&cache_key) {
        return Ok(cached.as_ref().clone());
    }

    let _permit = acquire_provider_permit(&provider_limit, &provider.descriptor().id, blocking).await?;
    let results = execute_with_retry(config, provider.descriptor().id.clone(), cancel_token, config.search_timeout(), || {
        provider.search(task, plan)
    })
    .await?;
    search_cache.insert(cache_key, Arc::new(results.clone()));
    Ok(results)
}

async fn execute_provider_acquire(
    provider: &dyn Provider,
    task: &TrackTask,
    candidate: &ProviderSearchCandidate,
    temp_context: &TaskTempContext,
    plan: &StrategyPlan,
    config: &DirectorConfig,
    provider_limit: Arc<Semaphore>,
    blocking: bool,
    cancel_token: &CancellationToken,
) -> Result<crate::director::models::CandidateAcquisition, DirectorError> {
    let _permit = acquire_provider_permit(&provider_limit, &provider.descriptor().id, blocking).await?;
    execute_with_retry(config, provider.descriptor().id.clone(), cancel_token, config.provider_timeout(), || {
        provider.acquire(task, candidate, temp_context, plan)
    })
    .await
}

async fn execute_with_retry<F, Fut, T>(
    config: &DirectorConfig,
    provider_id: String,
    cancel_token: &CancellationToken,
    timeout: std::time::Duration,
    mut operation: F,
) -> Result<T, DirectorError>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, ProviderError>>,
{
    let max_attempts = config.retry_policy.max_attempts_per_provider.max(1);
    let mut last_error = None::<ProviderError>;

    for attempt in 1..=max_attempts {
        let operation_fut = operation();
        tokio::pin!(operation_fut);
        let timeout_fut = sleep(timeout);
        tokio::pin!(timeout_fut);

        let result = tokio::select! {
            _ = cancel_token.cancelled() => {
                return Err(DirectorError::TaskCancelled);
            }
            result = &mut operation_fut => result,
            _ = &mut timeout_fut => {
                Err(ProviderError::TimedOut {
                    provider_id: provider_id.clone(),
                })
            }
        };

        match result {
            Ok(value) => return Ok(value),
            Err(error) => {
                if !error.retryable() || attempt == max_attempts {
                    return Err(DirectorError::Provider(error));
                }
                last_error = Some(error);
            }
        }

        // Exponential backoff: base * 2^(attempt-1)
        let backoff = config.retry_policy.base_backoff_millis * (1u64 << (attempt - 1).min(6));
        tokio::select! {
            _ = cancel_token.cancelled() => {
                return Err(DirectorError::TaskCancelled);
            }
            _ = sleep(std::time::Duration::from_millis(backoff)) => {}
        }
    }

    Err(last_error
        .map(DirectorError::Provider)
        .unwrap_or(DirectorError::Provider(ProviderError::Other {
            provider_id,
            message: "provider operation failed without explicit error".to_string(),
        })))
}

fn unregister_task_token(
    task_tokens: &Arc<std::sync::Mutex<HashMap<String, CancellationToken>>>,
    task_id: &str,
) {
    if let Ok(mut tokens) = task_tokens.lock() {
        tokens.remove(task_id);
    }
}

fn record_provider_skip(
    attempts: &mut Vec<ProviderAttemptRecord>,
    provider_searches: &mut Vec<ProviderSearchRecord>,
    provider_id: &str,
    provider_order_index: usize,
    reason: &ProviderSkipReason,
) {
    attempts.push(ProviderAttemptRecord {
        provider_id: provider_id.to_string(),
        attempt: 0,
        outcome: reason.attempt_outcome(),
    });
    provider_searches.push(ProviderSearchRecord {
        provider_id: provider_id.to_string(),
        provider_display_name: provider_id.to_string(),
        provider_trust_rank: i32::MAX,
        provider_order_index,
        outcome: reason.search_outcome().to_string(),
        candidate_count: 0,
        error: reason.error_message(),
        retryable: reason.retryable(),
    });
}

async fn apply_provider_runtime_error(
    config: &DirectorConfig,
    provider_runtime_state: &Arc<RwLock<HashMap<String, ProviderRuntimeState>>>,
    provider_id: &str,
    error: &ProviderError,
) {
    let mut state_map = provider_runtime_state.write().await;
    let state = state_map
        .entry(provider_id.to_string())
        .or_insert_with(ProviderRuntimeState::default);
    let now = Utc::now();
    match error {
        ProviderError::AuthFailed { .. } => {
            state.disabled_reason = Some(error.to_string());
            state.unavailable_until = None;
            state.unavailable_reason = Some(error.to_string());
            state.busy_streak = 0;
        }
        ProviderError::RateLimited { .. } => {
            state.unavailable_until =
                Some(now + chrono::Duration::seconds(config.provider_rate_limit_cooldown_secs));
            state.unavailable_reason = Some(error.to_string());
            state.busy_streak = 0;
        }
        ProviderError::TemporaryOutage { .. } => {
            state.unavailable_until =
                Some(now + chrono::Duration::seconds(config.provider_temp_outage_cooldown_secs));
            state.unavailable_reason = Some(error.to_string());
            state.busy_streak = 0;
        }
        ProviderError::ProviderBusy { .. } => {
            state.busy_streak = state.busy_streak.saturating_add(1);
            if state.busy_streak >= 2 {
                state.unavailable_until =
                    Some(now + chrono::Duration::seconds(config.provider_busy_cooldown_secs));
                state.unavailable_reason = Some(error.to_string());
            }
        }
        _ => {
            state.busy_streak = 0;
        }
    }
}

async fn clear_provider_runtime_cooldown(
    provider_runtime_state: &Arc<RwLock<HashMap<String, ProviderRuntimeState>>>,
    provider_id: &str,
) {
    let mut state_map = provider_runtime_state.write().await;
    if let Some(state) = state_map.get_mut(provider_id) {
        state.unavailable_until = None;
        state.unavailable_reason = None;
        state.busy_streak = 0;
    }
}

async fn load_persisted_provider_hints(
    config: &DirectorConfig,
    task: &TrackTask,
    search_cache: &Arc<Cache<String, Arc<Vec<ProviderSearchCandidate>>>>,
    provider_cache_epochs: &Arc<RwLock<HashMap<String, u64>>>,
) -> PersistedProviderHints {
    let Some(db_path) = config.runtime_db_path.as_deref() else {
        return PersistedProviderHints::default();
    };

    let db = match Db::open_read_only(db_path) {
        Ok(db) => db,
        Err(error) => {
            warn!(
                path = %db_path.display(),
                error = %error,
                "failed to open runtime db for persisted provider hints"
            );
            return PersistedProviderHints::default();
        }
    };

    let request_signature = director_request_signature(task);
    let now = Utc::now();
    let memory_rows = match db.get_director_provider_memory(&request_signature) {
        Ok(rows) => rows,
        Err(error) => {
            warn!(
                signature = %request_signature,
                error = %error,
                "failed to load persisted provider memory"
            );
            Vec::new()
        }
    };
    let response_rows = match db.get_provider_response_cache(&request_signature) {
        Ok(rows) => rows,
        Err(error) => {
            warn!(
                signature = %request_signature,
                error = %error,
                "failed to load persisted provider response cache"
            );
            Vec::new()
        }
    };

    hydrate_search_cache_from_persisted_rows(
        config,
        task,
        search_cache,
        provider_cache_epochs,
        &response_rows,
        now,
    )
    .await;

    let mut skip_reasons = HashMap::new();
    for row in memory_rows {
        if let Some(reason) = persisted_provider_skip_reason(config, &row, now) {
            skip_reasons.insert(row.provider_id.clone(), reason);
        }
    }

    PersistedProviderHints { skip_reasons }
}

async fn hydrate_search_cache_from_persisted_rows(
    config: &DirectorConfig,
    task: &TrackTask,
    search_cache: &Arc<Cache<String, Arc<Vec<ProviderSearchCandidate>>>>,
    provider_cache_epochs: &Arc<RwLock<HashMap<String, u64>>>,
    rows: &[StoredProviderResponseCache],
    now: chrono::DateTime<Utc>,
) {
    for row in rows {
        let Some(updated_at) = parse_persisted_utc(row.updated_at.as_str()) else {
            continue;
        };
        if now
            .signed_duration_since(updated_at)
            .num_seconds()
            > config.provider_response_cache_max_age_secs.max(1)
        {
            continue;
        }
        if row.candidate_count == 0 {
            continue;
        }
        let Ok(envelope) =
            serde_json::from_str::<PersistedProviderResponseEnvelope>(&row.response_json)
        else {
            continue;
        };

        let mut candidate_rows = envelope
            .candidate_records
            .into_iter()
            .filter(|record| record.provider_id == row.provider_id)
            .collect::<Vec<_>>();
        if candidate_rows.is_empty() {
            continue;
        }
        candidate_rows.sort_by_key(|record| record.search_rank);

        let mut seen = std::collections::BTreeSet::<String>::new();
        let candidates = candidate_rows
            .into_iter()
            .filter_map(|record| {
                seen.insert(record.candidate.provider_candidate_id.clone())
                    .then_some(record.candidate)
            })
            .collect::<Vec<_>>();
        if candidates.is_empty() {
            continue;
        }

        let cache_key = provider_search_cache_key(provider_cache_epochs, &row.provider_id, task).await;
        if search_cache.get(&cache_key).is_none() {
            search_cache.insert(cache_key, Arc::new(candidates));
        }
    }
}

fn persisted_provider_skip_reason(
    config: &DirectorConfig,
    row: &StoredProviderMemory,
    now: chrono::DateTime<Utc>,
) -> Option<ProviderSkipReason> {
    let updated_at = parse_persisted_utc(row.updated_at.as_str())?;
    if now
        .signed_duration_since(updated_at)
        .num_seconds()
        > config.provider_memory_max_age_secs.max(1)
    {
        return None;
    }

    if let Some(until) = row
        .backoff_until
        .as_deref()
        .and_then(parse_persisted_utc)
        .filter(|until| *until > now)
    {
        return Some(ProviderSkipReason::PersistedCoolingDown {
            until,
            reason: format!(
                "{} ({})",
                row.failure_class,
                row.last_outcome
            ),
        });
    }

    match row.failure_class.as_str() {
        "no_result" | "unsupported" | "provider_unhealthy" | "auth_failed" => {
            Some(ProviderSkipReason::PersistedMemory {
                reason: format!("fresh {} from {}", row.failure_class, row.last_outcome),
            })
        }
        _ => None,
    }
}

fn parse_persisted_utc(value: &str) -> Option<chrono::DateTime<Utc>> {
    use chrono::{NaiveDateTime, TimeZone};

    chrono::DateTime::parse_from_rfc3339(value)
        .map(|dt| dt.with_timezone(&Utc))
        .ok()
        .or_else(|| {
            NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S")
                .ok()
                .map(|dt| Utc.from_utc_datetime(&dt))
        })
}

async fn should_skip_provider(
    config: &DirectorConfig,
    provider_health_state: &Arc<RwLock<HashMap<String, ProviderHealthState>>>,
    provider_runtime_state: &Arc<RwLock<HashMap<String, ProviderRuntimeState>>>,
    persisted_skip_reasons: &HashMap<String, ProviderSkipReason>,
    provider_id: &str,
) -> Option<ProviderSkipReason> {
    let runtime_state = provider_runtime_state.read().await.get(provider_id).cloned();
    if let Some(runtime_state) = runtime_state {
        if let Some(reason) = runtime_state.disabled_reason {
            return Some(ProviderSkipReason::Disabled { reason });
        }
        if let Some(until) = runtime_state.unavailable_until {
            if until > Utc::now() {
                return Some(ProviderSkipReason::CoolingDown {
                    until,
                    reason: runtime_state
                        .unavailable_reason
                        .unwrap_or_else(|| "provider cooling down".to_string()),
                });
            }
        }
    }
    if let Some(reason) = persisted_skip_reasons.get(provider_id).cloned() {
        return Some(reason);
    }
    let state = provider_health_state.read().await.get(provider_id).cloned()?;
    let age_secs = chrono::Utc::now()
        .signed_duration_since(state.checked_at)
        .num_seconds();
    if age_secs > config.provider_health_stale_secs {
        return None;
    }
    matches!(state.status, ProviderHealthStatus::Down)
        .then_some(ProviderSkipReason::HealthDown(state))
}

async fn run_provider_health_loop(
    providers: Vec<Arc<dyn Provider>>,
    config: DirectorConfig,
    provider_health_state: Arc<RwLock<HashMap<String, ProviderHealthState>>>,
    provider_cache_epochs: Arc<RwLock<HashMap<String, u64>>>,
    provider_health_events: broadcast::Sender<ProviderHealthState>,
    cancel_token: CancellationToken,
) {
    let mut ordered = providers
        .into_iter()
        .map(|provider| (provider.descriptor().id.clone(), provider))
        .collect::<Vec<_>>();
    ordered.sort_by(|left, right| left.0.cmp(&right.0));

    loop {
        let mut probes = JoinSet::new();
        for (provider_id, provider) in &ordered {
            if cancel_token.is_cancelled() {
                return;
            }
            let provider_id = provider_id.clone();
            let provider = Arc::clone(provider);
            probes.spawn(async move {
                let probe_result = provider.health_check().await;
                let checked_at = Utc::now();
                match probe_result {
                    Ok(mut state) => {
                        state.provider_id = provider_id;
                        state.checked_at = checked_at;
                        state
                    }
                    Err(error) => ProviderHealthState {
                        provider_id,
                        status: ProviderHealthStatus::Down,
                        checked_at,
                        message: Some(error.to_string()),
                    },
                }
            });
        }

        while let Some(joined) = probes.join_next().await {
            if cancel_token.is_cancelled() {
                return;
            }
            let Ok(next_state) = joined else {
                continue;
            };
            let provider_id = next_state.provider_id.clone();
            let changed = {
                let mut states = provider_health_state.write().await;
                let previous = states.insert(provider_id.clone(), next_state.clone());
                previous
                    .map(|state| state.status != next_state.status || state.message != next_state.message)
                    .unwrap_or(true)
            };

            if changed {
                let should_bump_epoch = !matches!(next_state.status, ProviderHealthStatus::Healthy);
                if should_bump_epoch {
                    let mut epochs = provider_cache_epochs.write().await;
                    let next_epoch = epochs.get(&provider_id).copied().unwrap_or(0).saturating_add(1);
                    epochs.insert(provider_id.clone(), next_epoch);
                }
                let _ = provider_health_events.send(next_state);
            }
        }

        tokio::select! {
            _ = cancel_token.cancelled() => return,
            _ = sleep(std::time::Duration::from_secs(config.provider_health_interval_secs.max(1))) => {}
        }
    }
}

fn send_event(
    events: &broadcast::Sender<DirectorEvent>,
    task_id: &str,
    progress: DirectorProgress,
    provider_id: Option<String>,
    message: &str,
) {
    let _ = events.send(DirectorEvent {
        task_id: task_id.to_string(),
        progress,
        provider_id,
        message: message.to_string(),
    });
}

async fn provider_search_cache_key(
    provider_cache_epochs: &Arc<RwLock<HashMap<String, u64>>>,
    provider_id: &str,
    task: &TrackTask,
) -> String {
    let epoch = provider_cache_epochs
        .read()
        .await
        .get(provider_id)
        .copied()
        .unwrap_or(0);
    format!(
        "{}::{}::{}::{}::{}::{}",
        provider_id,
        epoch,
        task.strategy as u8,
        normalize_cache_component(&task.target.artist),
        normalize_cache_component(task.target.album.as_deref().unwrap_or_default()),
        normalize_cache_component(&task.target.title),
    )
}

fn normalize_cache_component(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::director::config::{ProviderPolicy, TempRecoveryPolicy};
    use crate::director::models::{
        AcquisitionStrategy, NormalizedTrack, ProviderCapabilities, ProviderHealthState,
        ProviderHealthStatus, ProviderSearchRecord, TrackTaskSource,
    };
    use crate::db::Db;
    use async_trait::async_trait;
    use tempfile::tempdir;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::sync::Notify;

    #[derive(Clone)]
    struct MockProvider {
        descriptor: ProviderDescriptor,
        search_candidates: Vec<ProviderSearchCandidate>,
        payload: Vec<u8>,
        acquire_gate: Option<Arc<Notify>>,
        health_status: Option<ProviderHealthStatus>,
        search_error: Option<ProviderError>,
        acquire_error: Option<ProviderError>,
        search_calls: Option<Arc<AtomicUsize>>,
    }

    #[async_trait]
    impl Provider for MockProvider {
        fn descriptor(&self) -> ProviderDescriptor {
            self.descriptor.clone()
        }

        async fn health_check(&self) -> Result<ProviderHealthState, ProviderError> {
            match self.health_status.unwrap_or(ProviderHealthStatus::Healthy) {
                ProviderHealthStatus::Down => Err(ProviderError::TemporaryOutage {
                    provider_id: self.descriptor.id.clone(),
                    message: "mock provider marked down".to_string(),
                }),
                status => Ok(ProviderHealthState {
                    provider_id: self.descriptor.id.clone(),
                    status,
                    checked_at: Utc::now(),
                    message: None,
                }),
            }
        }

        async fn search(
            &self,
            _task: &TrackTask,
            _strategy: &StrategyPlan,
        ) -> Result<Vec<ProviderSearchCandidate>, ProviderError> {
            if let Some(counter) = &self.search_calls {
                counter.fetch_add(1, Ordering::SeqCst);
            }
            if let Some(error) = &self.search_error {
                return Err(error.clone());
            }
            Ok(self.search_candidates.clone())
        }

        async fn acquire(
            &self,
            _task: &TrackTask,
            candidate: &ProviderSearchCandidate,
            temp_context: &TaskTempContext,
            _strategy: &StrategyPlan,
        ) -> Result<crate::director::models::CandidateAcquisition, ProviderError> {
            if let Some(gate) = &self.acquire_gate {
                gate.notified().await;
            }
            if let Some(error) = &self.acquire_error {
                return Err(error.clone());
            }
            let extension = candidate
                .extension_hint
                .clone()
                .unwrap_or_else(|| "bin".to_string());
            let path = temp_context
                .active_dir
                .join(format!("{}.{}", candidate.provider_candidate_id, extension));
            tokio::fs::write(&path, &self.payload)
                .await
                .map_err(|error| ProviderError::Other {
                    provider_id: self.descriptor.id.clone(),
                    message: error.to_string(),
                })?;
            Ok(crate::director::models::CandidateAcquisition {
                provider_id: self.descriptor.id.clone(),
                provider_candidate_id: candidate.provider_candidate_id.clone(),
                temp_path: path,
                file_size: self.payload.len() as u64,
                extension_hint: Some(extension),
                resolved_metadata: None,
            })
        }
    }

    fn task(strategy: AcquisitionStrategy) -> TrackTask {
        TrackTask {
            task_id: "task-1".to_string(),
            source: TrackTaskSource::Manual,
            desired_track_id: None,
            source_operation_id: None,
            target: NormalizedTrack {
                spotify_track_id: None,
                source_album_id: None,
                source_artist_id: None,
                source_playlist: None,
                artist: "Artist".to_string(),
                album_artist: None,
                title: "Song".to_string(),
                album: Some("Album".to_string()),
                track_number: Some(1),
                disc_number: Some(1),
                year: Some(2024),
                duration_secs: Some(1.0),
                isrc: None,
                musicbrainz_recording_id: None,
                musicbrainz_release_id: None,
                canonical_artist_id: None,
                canonical_release_id: None,
            },
            strategy,
        }
    }

    fn provider(
        id: &str,
        trust_rank: i32,
        payload: Vec<u8>,
        extension_hint: &str,
    ) -> Arc<dyn Provider> {
        Arc::new(MockProvider {
            descriptor: ProviderDescriptor {
                id: id.to_string(),
                display_name: id.to_string(),
                trust_rank,
                capabilities: ProviderCapabilities {
                    supports_search: true,
                    supports_download: true,
                    supports_lossless: extension_hint == "wav",
                    supports_batch: false,
                },
            },
            search_candidates: vec![ProviderSearchCandidate {
                provider_id: id.to_string(),
                provider_candidate_id: format!("{id}-candidate"),
                artist: "Artist".to_string(),
                title: "Song".to_string(),
                album: Some("Album".to_string()),
                duration_secs: Some(1.0),
                extension_hint: Some(extension_hint.to_string()),
                bitrate_kbps: Some(if extension_hint == "wav" { 1411 } else { 320 }),
                cover_art_url: None,
                metadata_confidence: 0.95,
            }],
            payload,
            acquire_gate: None,
            health_status: None,
            search_error: None,
            acquire_error: None,
            search_calls: None,
        })
    }

    fn gated_provider(
        id: &str,
        trust_rank: i32,
        payload: Vec<u8>,
        extension_hint: &str,
        acquire_gate: Arc<Notify>,
    ) -> Arc<dyn Provider> {
        Arc::new(MockProvider {
            descriptor: ProviderDescriptor {
                id: id.to_string(),
                display_name: id.to_string(),
                trust_rank,
                capabilities: ProviderCapabilities {
                    supports_search: true,
                    supports_download: true,
                    supports_lossless: extension_hint == "wav",
                    supports_batch: false,
                },
            },
            search_candidates: vec![ProviderSearchCandidate {
                provider_id: id.to_string(),
                provider_candidate_id: format!("{id}-candidate"),
                artist: "Artist".to_string(),
                title: "Song".to_string(),
                album: Some("Album".to_string()),
                duration_secs: Some(1.0),
                extension_hint: Some(extension_hint.to_string()),
                bitrate_kbps: Some(if extension_hint == "wav" { 1411 } else { 320 }),
                cover_art_url: None,
                metadata_confidence: 0.95,
            }],
            payload,
            acquire_gate: Some(acquire_gate),
            health_status: None,
            search_error: None,
            acquire_error: None,
            search_calls: None,
        })
    }

    fn unhealthy_provider(id: &str, trust_rank: i32) -> Arc<dyn Provider> {
        Arc::new(MockProvider {
            descriptor: ProviderDescriptor {
                id: id.to_string(),
                display_name: id.to_string(),
                trust_rank,
                capabilities: ProviderCapabilities {
                    supports_search: true,
                    supports_download: true,
                    supports_lossless: true,
                    supports_batch: false,
                },
            },
            search_candidates: vec![ProviderSearchCandidate {
                provider_id: id.to_string(),
                provider_candidate_id: format!("{id}-candidate"),
                artist: "Artist".to_string(),
                title: "Song".to_string(),
                album: Some("Album".to_string()),
                duration_secs: Some(1.0),
                extension_hint: Some("wav".to_string()),
                bitrate_kbps: Some(1411),
                cover_art_url: None,
                metadata_confidence: 0.95,
            }],
            payload: build_wav_bytes(),
            acquire_gate: None,
            health_status: Some(ProviderHealthStatus::Down),
            search_error: None,
            acquire_error: None,
            search_calls: None,
        })
    }

    fn metadata_only_provider(id: &str, trust_rank: i32) -> Arc<dyn Provider> {
        Arc::new(MockProvider {
            descriptor: ProviderDescriptor {
                id: id.to_string(),
                display_name: id.to_string(),
                trust_rank,
                capabilities: ProviderCapabilities {
                    supports_search: true,
                    supports_download: false,
                    supports_lossless: false,
                    supports_batch: false,
                },
            },
            search_candidates: vec![ProviderSearchCandidate {
                provider_id: id.to_string(),
                provider_candidate_id: format!("{id}-candidate"),
                artist: "Artist".to_string(),
                title: "Song".to_string(),
                album: Some("Album".to_string()),
                duration_secs: Some(1.0),
                extension_hint: Some("mp3".to_string()),
                bitrate_kbps: Some(128),
                cover_art_url: None,
                metadata_confidence: 0.95,
            }],
            payload: build_wav_bytes(),
            acquire_gate: None,
            health_status: None,
            search_error: None,
            acquire_error: None,
            search_calls: None,
        })
    }

    fn search_error_provider(id: &str, trust_rank: i32, error: ProviderError) -> Arc<dyn Provider> {
        Arc::new(MockProvider {
            descriptor: ProviderDescriptor {
                id: id.to_string(),
                display_name: id.to_string(),
                trust_rank,
                capabilities: ProviderCapabilities {
                    supports_search: true,
                    supports_download: true,
                    supports_lossless: true,
                    supports_batch: false,
                },
            },
            search_candidates: Vec::new(),
            payload: build_wav_bytes(),
            acquire_gate: None,
            health_status: None,
            search_error: Some(error),
            acquire_error: None,
            search_calls: None,
        })
    }

    fn counted_provider(
        id: &str,
        trust_rank: i32,
        payload: Vec<u8>,
        extension_hint: &str,
        search_calls: Arc<AtomicUsize>,
    ) -> Arc<dyn Provider> {
        Arc::new(MockProvider {
            descriptor: ProviderDescriptor {
                id: id.to_string(),
                display_name: id.to_string(),
                trust_rank,
                capabilities: ProviderCapabilities {
                    supports_search: true,
                    supports_download: true,
                    supports_lossless: extension_hint == "wav",
                    supports_batch: false,
                },
            },
            search_candidates: vec![ProviderSearchCandidate {
                provider_id: id.to_string(),
                provider_candidate_id: format!("{id}-candidate"),
                artist: "Artist".to_string(),
                title: "Song".to_string(),
                album: Some("Album".to_string()),
                duration_secs: Some(1.0),
                extension_hint: Some(extension_hint.to_string()),
                bitrate_kbps: Some(if extension_hint == "wav" { 1411 } else { 320 }),
                cover_art_url: None,
                metadata_confidence: 0.95,
            }],
            payload,
            acquire_gate: None,
            health_status: None,
            search_error: None,
            acquire_error: None,
            search_calls: Some(search_calls),
        })
    }

    fn build_wav_bytes() -> Vec<u8> {
        let sample_rate = 44_100_u32;
        let channels = 1_u16;
        let bits_per_sample = 16_u16;
        let duration_samples = sample_rate;
        let data_len = duration_samples * u32::from(channels) * u32::from(bits_per_sample / 8);
        let byte_rate = sample_rate * u32::from(channels) * u32::from(bits_per_sample / 8);
        let block_align = channels * (bits_per_sample / 8);
        let riff_len = 36 + data_len;

        let mut bytes = Vec::<u8>::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&riff_len.to_le_bytes());
        bytes.extend_from_slice(b"WAVE");
        bytes.extend_from_slice(b"fmt ");
        bytes.extend_from_slice(&16_u32.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&channels.to_le_bytes());
        bytes.extend_from_slice(&sample_rate.to_le_bytes());
        bytes.extend_from_slice(&byte_rate.to_le_bytes());
        bytes.extend_from_slice(&block_align.to_le_bytes());
        bytes.extend_from_slice(&bits_per_sample.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_len.to_le_bytes());
        bytes.resize(bytes.len() + data_len as usize, 0_u8);
        bytes
    }

    #[tokio::test]
    async fn director_waterfall_rejects_bad_payload_and_finalizes_fallback() {
        let root = tempdir().expect("temp dir");
        let config = DirectorConfig {
            library_root: root.path().join("library"),
            temp_root: root.path().join("temp"),
            local_search_roots: vec![root.path().join("staging")],
            worker_concurrency: 1,
            provider_timeout_secs: 2,
            retry_policy: Default::default(),
            quality_policy: crate::director::config::QualityPolicy {
                minimum_duration_secs: 0.5,
                max_duration_delta_secs: Some(2.0),
                preferred_extensions: vec!["wav".to_string()],
            },
            duplicate_policy: DuplicatePolicy::KeepExisting,
            temp_recovery: TempRecoveryPolicy {
                stale_after_hours: 24,
                quarantine_failures: false,
            },
            provider_policies: vec![
                ProviderPolicy {
                    provider_id: "primary".to_string(),
                    max_concurrency: 1,
                },
                ProviderPolicy {
                    provider_id: "fallback".to_string(),
                    max_concurrency: 1,
                },
            ],
            ..DirectorConfig::default()
        };

        let director = Director::new(
            config.clone(),
            vec![
                provider("primary", 1, b"<html>not audio</html>".to_vec(), "mp3"),
                provider("fallback", 5, build_wav_bytes(), "wav"),
            ],
        );
        let handle = director.start();
        let mut events = handle.subscribe();
        handle
            .submitter
            .submit(task(AcquisitionStrategy::Standard))
            .await
            .expect("submit task");

        let final_event = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                let event = events.recv().await.expect("receive event");
                if matches!(event.progress, DirectorProgress::Finalized) {
                    break event;
                }
            }
        })
        .await
        .expect("finalized event");

        handle.shutdown().await.expect("shutdown director");

        assert_eq!(final_event.provider_id.as_deref(), Some("fallback"));
        let expected = config
            .library_root
            .join("Artist")
            .join("Album")
            .join("01 - Song.wav");
        assert!(expected.exists());
    }

    #[tokio::test]
    async fn director_skips_metadata_only_providers_for_acquisition() {
        let root = tempdir().expect("temp dir");
        let config = DirectorConfig {
            library_root: root.path().join("library"),
            temp_root: root.path().join("temp"),
            local_search_roots: vec![root.path().join("staging")],
            worker_concurrency: 1,
            provider_timeout_secs: 2,
            retry_policy: Default::default(),
            quality_policy: crate::director::config::QualityPolicy {
                minimum_duration_secs: 0.5,
                max_duration_delta_secs: Some(2.0),
                preferred_extensions: vec!["wav".to_string()],
            },
            duplicate_policy: DuplicatePolicy::KeepExisting,
            temp_recovery: TempRecoveryPolicy {
                stale_after_hours: 24,
                quarantine_failures: false,
            },
            provider_policies: vec![
                ProviderPolicy {
                    provider_id: "metadata".to_string(),
                    max_concurrency: 1,
                },
                ProviderPolicy {
                    provider_id: "fallback".to_string(),
                    max_concurrency: 1,
                },
            ],
            ..DirectorConfig::default()
        };

        let director = Director::new(
            config.clone(),
            vec![
                metadata_only_provider("metadata", 1),
                provider("fallback", 5, build_wav_bytes(), "wav"),
            ],
        );
        let handle = director.start();
        let mut results = handle.subscribe_results();
        handle
            .submitter
            .submit(task(AcquisitionStrategy::Standard))
            .await
            .expect("submit task");

        let result = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                let result = results.recv().await.expect("receive result");
                if matches!(result.disposition, FinalizedTrackDisposition::Finalized) {
                    break result;
                }
            }
        })
        .await
        .expect("result received");

        handle.shutdown().await.expect("shutdown director");

        assert_eq!(result.finalized.as_ref().map(|track| track.provenance.selected_provider.as_str()), Some("fallback"));
        assert!(result
            .attempts
            .iter()
            .any(|attempt| attempt.provider_id == "metadata"
                && attempt.outcome == "skipped: provider is metadata-only"));
    }

    #[tokio::test]
    async fn director_cancels_specific_queued_task_without_stopping_batch() {
        let root = tempdir().expect("temp dir");
        let gate = Arc::new(Notify::new());
        let config = DirectorConfig {
            library_root: root.path().join("library"),
            temp_root: root.path().join("temp"),
            local_search_roots: vec![root.path().join("staging")],
            worker_concurrency: 1,
            provider_timeout_secs: 2,
            retry_policy: Default::default(),
            quality_policy: crate::director::config::QualityPolicy {
                minimum_duration_secs: 0.5,
                max_duration_delta_secs: Some(2.0),
                preferred_extensions: vec!["wav".to_string()],
            },
            duplicate_policy: DuplicatePolicy::KeepExisting,
            temp_recovery: TempRecoveryPolicy {
                stale_after_hours: 24,
                quarantine_failures: false,
            },
            provider_policies: vec![ProviderPolicy {
                provider_id: "gated".to_string(),
                max_concurrency: 1,
            }],
            ..DirectorConfig::default()
        };

        let director = Director::new(
            config.clone(),
            vec![gated_provider("gated", 5, build_wav_bytes(), "wav", Arc::clone(&gate))],
        );
        let handle = director.start();
        let mut results = handle.subscribe_results();
        let first_task = task(AcquisitionStrategy::Standard);
        let second_task = TrackTask {
            task_id: "task-2".to_string(),
            ..task(AcquisitionStrategy::Standard)
        };

        handle.submitter.submit(first_task).await.expect("submit first");
        handle.submitter.submit(second_task.clone()).await.expect("submit second");
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        assert!(handle.cancel_task(&second_task.task_id));
        gate.notify_waiters();

        let mut saw_first_finalized = false;
        let mut saw_second_cancelled = false;
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            while !(saw_first_finalized && saw_second_cancelled) {
                let result = results.recv().await.expect("receive result");
                if result.task_id == "task-1"
                    && matches!(result.disposition, FinalizedTrackDisposition::Finalized)
                {
                    saw_first_finalized = true;
                }
                if result.task_id == "task-2"
                    && matches!(result.disposition, FinalizedTrackDisposition::Cancelled)
                {
                    saw_second_cancelled = true;
                }
            }
        })
        .await
        .expect("results received");

        handle.shutdown().await.expect("shutdown director");
        assert!(saw_first_finalized);
        assert!(saw_second_cancelled);
    }

    #[tokio::test]
    async fn director_skips_provider_marked_down_by_health_check() {
        let root = tempdir().expect("temp dir");
        let config = DirectorConfig {
            library_root: root.path().join("library"),
            temp_root: root.path().join("temp"),
            local_search_roots: vec![root.path().join("staging")],
            worker_concurrency: 1,
            provider_timeout_secs: 2,
            retry_policy: Default::default(),
            quality_policy: crate::director::config::QualityPolicy {
                minimum_duration_secs: 0.5,
                max_duration_delta_secs: Some(2.0),
                preferred_extensions: vec!["wav".to_string()],
            },
            duplicate_policy: DuplicatePolicy::KeepExisting,
            temp_recovery: TempRecoveryPolicy {
                stale_after_hours: 24,
                quarantine_failures: false,
            },
            provider_policies: vec![
                ProviderPolicy {
                    provider_id: "down".to_string(),
                    max_concurrency: 1,
                },
                ProviderPolicy {
                    provider_id: "fallback".to_string(),
                    max_concurrency: 1,
                },
            ],
            ..DirectorConfig::default()
        };

        let director = Director::new(
            config.clone(),
            vec![
                unhealthy_provider("down", 10),
                provider("fallback", 5, build_wav_bytes(), "wav"),
            ],
        );
        let handle = director.start();
        let mut health = handle.subscribe_health();
        let mut results = handle.subscribe_results();

        tokio::time::timeout(std::time::Duration::from_secs(3), async {
            loop {
                let event = health.recv().await.expect("receive health");
                if event.provider_id == "down" && matches!(event.status, ProviderHealthStatus::Down) {
                    break;
                }
            }
        })
        .await
        .expect("health event");

        handle
            .submitter
            .submit(task(AcquisitionStrategy::Standard))
            .await
            .expect("submit task");

        let result = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                let result = results.recv().await.expect("receive result");
                if matches!(result.disposition, FinalizedTrackDisposition::Finalized) {
                    break result;
                }
            }
        })
        .await
        .expect("result received");

        handle.shutdown().await.expect("shutdown director");

        assert_eq!(
            result.finalized.as_ref().map(|track| track.provenance.selected_provider.as_str()),
            Some("fallback")
        );
        assert!(result
            .attempts
            .iter()
            .any(|attempt| attempt.provider_id == "down"
                && attempt.outcome.contains("skipped: provider health down")));
    }

    #[tokio::test]
    async fn director_auth_failure_disables_provider_for_following_tasks() {
        let root = tempdir().expect("temp dir");
        let config = DirectorConfig {
            library_root: root.path().join("library"),
            temp_root: root.path().join("temp"),
            local_search_roots: vec![root.path().join("staging")],
            worker_concurrency: 1,
            provider_timeout_secs: 2,
            retry_policy: Default::default(),
            quality_policy: crate::director::config::QualityPolicy {
                minimum_duration_secs: 0.5,
                max_duration_delta_secs: Some(2.0),
                preferred_extensions: vec!["wav".to_string()],
            },
            duplicate_policy: DuplicatePolicy::KeepExisting,
            temp_recovery: TempRecoveryPolicy {
                stale_after_hours: 24,
                quarantine_failures: false,
            },
            provider_policies: vec![
                ProviderPolicy {
                    provider_id: "auth".to_string(),
                    max_concurrency: 1,
                },
                ProviderPolicy {
                    provider_id: "fallback".to_string(),
                    max_concurrency: 1,
                },
            ],
            ..DirectorConfig::default()
        };

        let director = Director::new(
            config,
            vec![
                search_error_provider(
                    "auth",
                    1,
                    ProviderError::AuthFailed {
                        provider_id: "auth".to_string(),
                    },
                ),
                provider("fallback", 5, build_wav_bytes(), "wav"),
            ],
        );
        let handle = director.start();
        let mut results = handle.subscribe_results();

        handle
            .submitter
            .submit(task(AcquisitionStrategy::Standard))
            .await
            .expect("submit task one");
        let task_two = TrackTask {
            task_id: "task-2".to_string(),
            ..task(AcquisitionStrategy::Standard)
        };
        handle
            .submitter
            .submit(task_two)
            .await
            .expect("submit task two");

        let mut seen = Vec::new();
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            while seen.len() < 2 {
                let result = results.recv().await.expect("receive result");
                if matches!(
                    result.disposition,
                    FinalizedTrackDisposition::Finalized
                        | FinalizedTrackDisposition::AlreadyPresent
                ) {
                    seen.push(result);
                }
            }
        })
        .await
        .expect("results received");

        handle.shutdown().await.expect("shutdown director");

        let second = seen.iter().find(|result| result.task_id == "task-2").expect("second result");
        assert!(second
            .attempts
            .iter()
            .any(|attempt| attempt.provider_id == "auth"
                && attempt.outcome.contains("skipped: provider unavailable")));
    }

    #[tokio::test]
    async fn director_busy_provider_cools_down_for_following_tasks() {
        let root = tempdir().expect("temp dir");
        let config = DirectorConfig {
            library_root: root.path().join("library"),
            temp_root: root.path().join("temp"),
            local_search_roots: vec![root.path().join("staging")],
            worker_concurrency: 1,
            provider_timeout_secs: 2,
            retry_policy: Default::default(),
            quality_policy: crate::director::config::QualityPolicy {
                minimum_duration_secs: 0.5,
                max_duration_delta_secs: Some(2.0),
                preferred_extensions: vec!["wav".to_string()],
            },
            duplicate_policy: DuplicatePolicy::KeepExisting,
            temp_recovery: TempRecoveryPolicy {
                stale_after_hours: 24,
                quarantine_failures: false,
            },
            provider_policies: vec![
                ProviderPolicy {
                    provider_id: "busy".to_string(),
                    max_concurrency: 1,
                },
                ProviderPolicy {
                    provider_id: "fallback".to_string(),
                    max_concurrency: 1,
                },
            ],
            ..DirectorConfig::default()
        };

        let director = Director::new(
            config,
            vec![
                search_error_provider(
                    "busy",
                    1,
                    ProviderError::ProviderBusy {
                        provider_id: "busy".to_string(),
                    },
                ),
                provider("fallback", 5, build_wav_bytes(), "wav"),
            ],
        );
        let handle = director.start();
        let mut results = handle.subscribe_results();

        handle
            .submitter
            .submit(task(AcquisitionStrategy::Standard))
            .await
            .expect("submit task one");
        let task_two = TrackTask {
            task_id: "task-2".to_string(),
            ..task(AcquisitionStrategy::Standard)
        };
        handle
            .submitter
            .submit(task_two)
            .await
            .expect("submit task two");

        let mut seen = Vec::new();
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            while seen.len() < 2 {
                let result = results.recv().await.expect("receive result");
                if matches!(
                    result.disposition,
                    FinalizedTrackDisposition::Finalized
                        | FinalizedTrackDisposition::AlreadyPresent
                ) {
                    seen.push(result);
                }
            }
        })
        .await
        .expect("results received");

        handle.shutdown().await.expect("shutdown director");

        let second = seen.iter().find(|result| result.task_id == "task-2").expect("second result");
        assert!(
            second
                .attempts
                .iter()
                .any(|attempt| attempt.provider_id == "busy"
                    && (attempt.outcome.contains("cooling down")
                        || attempt.outcome.contains("provider busy")
                        || attempt.outcome.contains("provider unavailable")))
                || second.provider_searches.iter().any(|record| {
                    record.provider_id == "busy"
                        && matches!(
                            record.outcome.as_str(),
                            "busy" | "skipped_runtime_cooldown" | "skipped_runtime_unavailable"
                        )
                }),
            "attempts={:?} provider_searches={:?}",
            second.attempts,
            second.provider_searches
        );
    }

    #[tokio::test]
    async fn director_skips_provider_with_fresh_persisted_negative_memory() {
        let root = tempdir().expect("temp dir");
        let db_path = root.path().join("runtime.db");
        let db = Db::open(&db_path).expect("runtime db");
        let request_task = task(AcquisitionStrategy::Standard);
        db.save_director_task_result(
            &DirectorTaskResult {
                task_id: "persisted-failure".to_string(),
                disposition: FinalizedTrackDisposition::Failed,
                finalized: None,
                attempts: Vec::new(),
                error: Some("no result".to_string()),
                candidate_records: Vec::new(),
                provider_searches: vec![ProviderSearchRecord {
                    provider_id: "dead".to_string(),
                    provider_display_name: "dead".to_string(),
                    provider_trust_rank: 1,
                    provider_order_index: 0,
                    outcome: "no_candidates".to_string(),
                    candidate_count: 0,
                    error: None,
                    retryable: false,
                }],
            },
            Some(&request_task),
        )
        .expect("seed provider memory");

        let dead_search_calls = Arc::new(AtomicUsize::new(0));
        let config = DirectorConfig {
            library_root: root.path().join("library"),
            temp_root: root.path().join("temp"),
            runtime_db_path: Some(db_path.clone()),
            local_search_roots: vec![root.path().join("staging")],
            worker_concurrency: 1,
            provider_timeout_secs: 2,
            retry_policy: Default::default(),
            quality_policy: crate::director::config::QualityPolicy {
                minimum_duration_secs: 0.5,
                max_duration_delta_secs: Some(2.0),
                preferred_extensions: vec!["wav".to_string()],
            },
            duplicate_policy: DuplicatePolicy::KeepExisting,
            temp_recovery: TempRecoveryPolicy {
                stale_after_hours: 24,
                quarantine_failures: false,
            },
            provider_policies: vec![
                ProviderPolicy {
                    provider_id: "dead".to_string(),
                    max_concurrency: 1,
                },
                ProviderPolicy {
                    provider_id: "fallback".to_string(),
                    max_concurrency: 1,
                },
            ],
            ..DirectorConfig::default()
        };

        let director = Director::new(
            config,
            vec![
                counted_provider(
                    "dead",
                    1,
                    build_wav_bytes(),
                    "wav",
                    Arc::clone(&dead_search_calls),
                ),
                provider("fallback", 5, build_wav_bytes(), "wav"),
            ],
        );
        let handle = director.start();
        let mut results = handle.subscribe_results();

        handle
            .submitter
            .submit(task(AcquisitionStrategy::Standard))
            .await
            .expect("submit task");

        let result = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                let result = results.recv().await.expect("receive result");
                if matches!(
                    result.disposition,
                    FinalizedTrackDisposition::Finalized | FinalizedTrackDisposition::AlreadyPresent
                ) {
                    break result;
                }
            }
        })
        .await
        .expect("result received");

        handle.shutdown().await.expect("shutdown director");

        assert_eq!(dead_search_calls.load(Ordering::SeqCst), 0);
        assert!(result
            .attempts
            .iter()
            .any(|attempt| attempt.provider_id == "dead"
                && attempt.outcome.contains("persisted provider memory")));
    }

    #[tokio::test]
    async fn director_hydrates_search_cache_from_persisted_response_cache() {
        let root = tempdir().expect("temp dir");
        let db_path = root.path().join("runtime.db");
        let db = Db::open(&db_path).expect("runtime db");
        let base_config = DirectorConfig {
            library_root: root.path().join("library"),
            temp_root: root.path().join("temp"),
            runtime_db_path: Some(db_path.clone()),
            local_search_roots: vec![root.path().join("staging")],
            worker_concurrency: 1,
            provider_timeout_secs: 2,
            retry_policy: Default::default(),
            quality_policy: crate::director::config::QualityPolicy {
                minimum_duration_secs: 0.5,
                max_duration_delta_secs: Some(2.0),
                preferred_extensions: vec!["wav".to_string()],
            },
            duplicate_policy: DuplicatePolicy::KeepExisting,
            temp_recovery: TempRecoveryPolicy {
                stale_after_hours: 24,
                quarantine_failures: false,
            },
            provider_policies: vec![ProviderPolicy {
                provider_id: "cached".to_string(),
                max_concurrency: 1,
            }],
            ..DirectorConfig::default()
        };

        let first_director = Director::new(
            base_config.clone(),
            vec![provider("cached", 1, build_wav_bytes(), "wav")],
        );
        let first_handle = first_director.start();
        let mut first_results = first_handle.subscribe_results();
        let first_task = task(AcquisitionStrategy::Standard);
        first_handle
            .submitter
            .submit(first_task.clone())
            .await
            .expect("submit first task");
        let first_result = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                let result = first_results.recv().await.expect("receive first result");
                if matches!(
                    result.disposition,
                    FinalizedTrackDisposition::Finalized | FinalizedTrackDisposition::AlreadyPresent
                ) {
                    break result;
                }
            }
        })
        .await
        .expect("first result received");
        db.save_director_task_result(&first_result, Some(&first_task))
            .expect("persist first result");
        first_handle.shutdown().await.expect("shutdown first director");

        let cached_search_calls = Arc::new(AtomicUsize::new(0));
        let second_director = Director::new(
            base_config,
            vec![counted_provider(
                "cached",
                1,
                build_wav_bytes(),
                "wav",
                Arc::clone(&cached_search_calls),
            )],
        );
        let second_handle = second_director.start();
        let mut second_results = second_handle.subscribe_results();
        let second_task = TrackTask {
            task_id: "task-2".to_string(),
            ..task(AcquisitionStrategy::Standard)
        };
        second_handle
            .submitter
            .submit(second_task)
            .await
            .expect("submit second task");
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                let result = second_results.recv().await.expect("receive second result");
                if matches!(
                    result.disposition,
                    FinalizedTrackDisposition::Finalized | FinalizedTrackDisposition::AlreadyPresent
                ) {
                    break;
                }
            }
        })
        .await
        .expect("second result received");

        second_handle.shutdown().await.expect("shutdown second director");

        assert_eq!(cached_search_calls.load(Ordering::SeqCst), 0);
    }
}
