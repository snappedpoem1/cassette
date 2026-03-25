use crate::director::config::{DirectorConfig, DuplicatePolicy};
use crate::director::error::{DirectorError, ProviderError};
use crate::director::finalize::finalize_selected_candidate;
use crate::director::metadata::apply_metadata;
use crate::director::models::{
    CandidateDisposition, CandidateSelection, DirectorEvent, DirectorProgress, DirectorTaskResult,
    FinalizedTrack, FinalizedTrackDisposition, ProviderAttemptRecord, ProviderDescriptor,
    ProviderSearchCandidate, ProvenanceRecord, TrackTask,
};
use crate::director::provider::Provider;
use crate::director::scoring::score_candidate;
use crate::director::strategy::{StrategyPlan, StrategyPlanner};
use crate::director::temp::{TaskTempContext, TempManager};
use crate::director::validation::validate_candidate;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, Semaphore};
use tokio::task::JoinSet;
use tokio::time::{sleep, timeout};
use tracing::{info, warn};

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
    manager: tokio::task::JoinHandle<Result<(), DirectorError>>,
}

impl DirectorHandle {
    pub fn subscribe(&self) -> broadcast::Receiver<DirectorEvent> {
        self.events.subscribe()
    }

    pub fn subscribe_results(&self) -> broadcast::Receiver<DirectorTaskResult> {
        self.results.subscribe()
    }

    pub async fn shutdown(self) -> Result<(), DirectorError> {
        drop(self.submitter);
        self.manager.await?
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
        let (tx, rx) = mpsc::channel::<TrackTask>(128);
        let (events, _) = broadcast::channel::<DirectorEvent>(256);
        let (results, _) = broadcast::channel::<DirectorTaskResult>(256);
        let manager = tokio::spawn(self.run(rx, events.clone(), results.clone()));

        DirectorHandle {
            submitter: DirectorSubmission { tx },
            events,
            results,
            manager,
        }
    }

    async fn run(
        self,
        mut rx: mpsc::Receiver<TrackTask>,
        events: broadcast::Sender<DirectorEvent>,
        results: broadcast::Sender<DirectorTaskResult>,
    ) -> Result<(), DirectorError> {
        self.recover_temp().await?;

        let semaphore = Arc::new(Semaphore::new(self.config.worker_concurrency.max(1)));
        let provider_limits = self.build_provider_limits();
        let mut join_set = JoinSet::<Result<(), DirectorError>>::new();

        while let Some(task) = rx.recv().await {
            send_event(&events, &task.task_id, DirectorProgress::Queued, None, "task queued");
            let permit = semaphore
                .clone()
                .acquire_owned()
                .await
                .map_err(|error| DirectorError::Queue(error.to_string()))?;

            let config = self.config.clone();
            let providers = self.providers.clone();
            let planner = self.planner.clone();
            let events_clone = events.clone();
            let results_clone = results.clone();
            let provider_limits_clone = provider_limits.clone();

            join_set.spawn(async move {
                let _permit = permit;
                process_task(
                    config,
                    providers,
                    planner,
                    provider_limits_clone,
                    events_clone,
                    results_clone,
                    task,
                )
                .await
            });
        }

        while let Some(result) = join_set.join_next().await {
            result??;
        }

        Ok(())
    }

    fn build_provider_limits(&self) -> HashMap<String, Arc<Semaphore>> {
        self.providers
            .iter()
            .map(|provider| {
                let descriptor = provider.descriptor();
                let limit = self
                    .config
                    .provider_policy(&descriptor.id)
                    .map(|policy| policy.max_concurrency.max(1))
                    .unwrap_or(1);
                (descriptor.id, Arc::new(Semaphore::new(limit)))
            })
            .collect()
    }
}

async fn process_task(
    config: DirectorConfig,
    providers: Vec<Arc<dyn Provider>>,
    planner: StrategyPlanner,
    provider_limits: HashMap<String, Arc<Semaphore>>,
    events: broadcast::Sender<DirectorEvent>,
    results: broadcast::Sender<DirectorTaskResult>,
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
        });
        return Ok(());
    }

    let temp_manager = TempManager::new(config.temp_root.clone(), config.temp_recovery.clone());
    let temp_context = temp_manager.prepare_task(&task.task_id).await?;
    let result = execute_waterfall(
        &config,
        &providers,
        &provider_limits,
        &events,
        &task,
        &plan,
        &temp_manager,
        &temp_context,
    )
    .await;

    match result {
        Ok((finalized, attempts)) => {
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
            });
            temp_manager.cleanup_task(&temp_context).await?;
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
                attempts: Vec::new(),
                error: Some(error.to_string()),
            });
            if !config.temp_recovery.quarantine_failures {
                temp_manager.cleanup_task(&temp_context).await?;
            }
            Err(error)
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn execute_waterfall(
    config: &DirectorConfig,
    providers: &[Arc<dyn Provider>],
    provider_limits: &HashMap<String, Arc<Semaphore>>,
    events: &broadcast::Sender<DirectorEvent>,
    task: &TrackTask,
    plan: &StrategyPlan,
    temp_manager: &TempManager,
    temp_context: &TaskTempContext,
) -> Result<(FinalizedTrack, Vec<ProviderAttemptRecord>), DirectorError> {
    let provider_map = providers
        .iter()
        .map(|provider| (provider.descriptor().id.clone(), Arc::clone(provider)))
        .collect::<HashMap<String, Arc<dyn Provider>>>();
    let mut valid_candidates = Vec::<(CandidateDisposition, ProviderDescriptor)>::new();
    let mut attempts = Vec::<ProviderAttemptRecord>::new();

    for provider_id in &plan.provider_order {
        let Some(provider) = provider_map.get(provider_id) else {
            continue;
        };
        let descriptor = provider.descriptor();
        if !descriptor.capabilities.supports_download {
            attempts.push(ProviderAttemptRecord {
                provider_id: provider_id.clone(),
                attempt: 0,
                outcome: "skipped: provider is metadata-only".to_string(),
            });
            continue;
        }
        let Some(limit) = provider_limits.get(provider_id) else {
            continue;
        };

        send_event(
            events,
            &task.task_id,
            DirectorProgress::ProviderAttempt,
            Some(provider_id.clone()),
            "searching provider",
        );

        let search_candidates = match execute_provider_search(
            provider.as_ref(),
            task,
            plan,
            config,
            Arc::clone(limit),
        )
        .await
        {
            Ok(candidates) => candidates,
            Err(error) => {
                attempts.push(ProviderAttemptRecord {
                    provider_id: provider_id.clone(),
                    attempt: 1,
                    outcome: error.to_string(),
                });
                continue;
            }
        };

        for candidate in search_candidates {
            match execute_provider_acquire(
                provider.as_ref(),
                task,
                &candidate,
                temp_context,
                plan,
                config,
                Arc::clone(limit),
            )
            .await
            {
                Ok(acquisition) => {
                    send_event(
                        events,
                        &task.task_id,
                        DirectorProgress::Validating,
                        Some(provider_id.clone()),
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
                            if plan.require_lossless
                                && !matches!(
                                    validation.quality,
                                    crate::director::models::CandidateQuality::Lossless
                                )
                            {
                                let _ = temp_manager
                                    .quarantine_file(temp_context, &acquisition.temp_path)
                                    .await;
                                attempts.push(ProviderAttemptRecord {
                                    provider_id: provider_id.clone(),
                                    attempt: 1,
                                    outcome: "rejected non-lossless candidate".to_string(),
                                });
                                continue;
                            }

                            let (score, _) = score_candidate(
                                &task.target,
                                &descriptor,
                                &candidate,
                                &validation,
                                &config.quality_policy,
                            );
                            let disposition = CandidateDisposition {
                                candidate,
                                acquisition,
                                validation,
                                score,
                            };
                            attempts.push(ProviderAttemptRecord {
                                provider_id: provider_id.clone(),
                                attempt: 1,
                                outcome: format!("valid candidate score {}", disposition.score.total),
                            });

                            if !plan.collect_multiple_candidates {
                                return finalize_candidate(
                                    config,
                                    events,
                                    task,
                                    descriptor,
                                    disposition,
                                    attempts,
                                )
                                .await;
                            }
                            valid_candidates.push((disposition, descriptor.clone()));
                        }
                        Err(error) => {
                            if config.temp_recovery.quarantine_failures {
                                let _ = temp_manager
                                    .quarantine_file(temp_context, &acquisition.temp_path)
                                    .await;
                            } else {
                                let _ = tokio::fs::remove_file(&acquisition.temp_path).await;
                            }
                            attempts.push(ProviderAttemptRecord {
                                provider_id: provider_id.clone(),
                                attempt: 1,
                                outcome: format!("validation failed: {error}"),
                            });
                        }
                    }
                }
                Err(error) => {
                    attempts.push(ProviderAttemptRecord {
                        provider_id: provider_id.clone(),
                        attempt: 1,
                        outcome: error.to_string(),
                    });
                }
            }
        }
    }

    if let Some((best, descriptor)) = valid_candidates
        .into_iter()
        .max_by_key(|(candidate, _)| candidate.score.total)
    {
        finalize_candidate(config, events, task, descriptor, best, attempts).await
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
    attempts: Vec<ProviderAttemptRecord>,
) -> Result<(FinalizedTrack, Vec<ProviderAttemptRecord>), DirectorError> {
    let (_, reason) = score_candidate(
        &task.target,
        &descriptor,
        &best.candidate,
        &best.validation,
        &config.quality_policy,
    );
    let selection = CandidateSelection {
        provider_id: best.candidate.provider_id.clone(),
        temp_path: best.acquisition.temp_path.clone(),
        score: best.score,
        reason,
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
    apply_metadata(task.clone(), selection.clone()).await?;

    send_event(
        events,
        &task.task_id,
        DirectorProgress::Finalizing,
        Some(selection.provider_id.clone()),
        "moving candidate into library",
    );
    let provenance = ProvenanceRecord {
        task_id: task.task_id.clone(),
        source_metadata: task.target.clone(),
        selected_provider: selection.provider_id.clone(),
        score_reason: selection.reason.clone(),
        validation_summary: selection.validation.clone(),
        final_path: Default::default(),
        acquired_at: Utc::now(),
    };
    let finalized = finalize_selected_candidate(
        config.library_root.clone(),
        selection,
        task.target.clone(),
        config.duplicate_policy,
        provenance,
    )
    .await;

    match finalized {
        Ok(track) => {
            info!(
                task_id = task.task_id,
                provider = track.provenance.selected_provider,
                attempts = attempts.len(),
                "director finalized track"
            );
            Ok((track, attempts))
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

async fn execute_provider_search(
    provider: &dyn Provider,
    task: &TrackTask,
    plan: &StrategyPlan,
    config: &DirectorConfig,
    provider_limit: Arc<Semaphore>,
) -> Result<Vec<ProviderSearchCandidate>, ProviderError> {
    let _permit = provider_limit
        .acquire_owned()
        .await
        .map_err(|error| ProviderError::Other {
            provider_id: provider.descriptor().id,
            message: error.to_string(),
        })?;
    execute_with_retry(config, provider.descriptor().id.clone(), || provider.search(task, plan)).await
}

async fn execute_provider_acquire(
    provider: &dyn Provider,
    task: &TrackTask,
    candidate: &ProviderSearchCandidate,
    temp_context: &TaskTempContext,
    plan: &StrategyPlan,
    config: &DirectorConfig,
    provider_limit: Arc<Semaphore>,
) -> Result<crate::director::models::CandidateAcquisition, ProviderError> {
    let _permit = provider_limit
        .acquire_owned()
        .await
        .map_err(|error| ProviderError::Other {
            provider_id: provider.descriptor().id,
            message: error.to_string(),
        })?;
    execute_with_retry(config, provider.descriptor().id.clone(), || {
        provider.acquire(task, candidate, temp_context, plan)
    })
    .await
}

async fn execute_with_retry<F, Fut, T>(
    config: &DirectorConfig,
    provider_id: String,
    mut operation: F,
) -> Result<T, ProviderError>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, ProviderError>>,
{
    let max_attempts = config.retry_policy.max_attempts_per_provider.max(1);
    let mut last_error = None::<ProviderError>;

    for attempt in 1..=max_attempts {
        match timeout(config.provider_timeout(), operation()).await {
            Ok(Ok(value)) => return Ok(value),
            Ok(Err(error)) => {
                if !error.retryable() || attempt == max_attempts {
                    return Err(error);
                }
                last_error = Some(error);
            }
            Err(_) => {
                let timeout_error = ProviderError::TimedOut {
                    provider_id: provider_id.clone(),
                };
                if attempt == max_attempts {
                    return Err(timeout_error);
                }
                last_error = Some(timeout_error);
            }
        }

        let backoff = config.retry_policy.base_backoff_millis * u64::from(attempt);
        sleep(std::time::Duration::from_millis(backoff)).await;
    }

    Err(last_error.unwrap_or(ProviderError::Other {
        provider_id,
        message: "provider operation failed without explicit error".to_string(),
    }))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::director::config::{ProviderPolicy, TempRecoveryPolicy};
    use crate::director::models::{
        AcquisitionStrategy, NormalizedTrack, ProviderCapabilities, TrackTaskSource,
    };
    use async_trait::async_trait;
    use tempfile::tempdir;

    #[derive(Clone)]
    struct MockProvider {
        descriptor: ProviderDescriptor,
        search_candidates: Vec<ProviderSearchCandidate>,
        payload: Vec<u8>,
    }

    #[async_trait]
    impl Provider for MockProvider {
        fn descriptor(&self) -> ProviderDescriptor {
            self.descriptor.clone()
        }

        async fn search(
            &self,
            _task: &TrackTask,
            _strategy: &StrategyPlan,
        ) -> Result<Vec<ProviderSearchCandidate>, ProviderError> {
            Ok(self.search_candidates.clone())
        }

        async fn acquire(
            &self,
            _task: &TrackTask,
            candidate: &ProviderSearchCandidate,
            temp_context: &TaskTempContext,
            _strategy: &StrategyPlan,
        ) -> Result<crate::director::models::CandidateAcquisition, ProviderError> {
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
            })
        }
    }

    fn task(strategy: AcquisitionStrategy) -> TrackTask {
        TrackTask {
            task_id: "task-1".to_string(),
            source: TrackTaskSource::Manual,
            target: NormalizedTrack {
                spotify_track_id: None,
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
}
