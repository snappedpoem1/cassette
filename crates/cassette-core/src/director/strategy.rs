use crate::director::config::DirectorConfig;
use crate::director::models::{
    AcquisitionStrategy, CandidateSelectionMode, ProviderDescriptor, TrackTask,
};

#[derive(Debug, Clone)]
pub struct StrategyPlan {
    pub strategy: AcquisitionStrategy,
    pub provider_order: Vec<String>,
    pub collect_multiple_candidates: bool,
    pub selection_mode: CandidateSelectionMode,
    pub require_lossless: bool,
}

#[derive(Debug, Default, Clone)]
pub struct StrategyPlanner;

impl StrategyPlanner {
    pub fn plan(
        &self,
        task: &TrackTask,
        providers: &[ProviderDescriptor],
        _config: &DirectorConfig,
    ) -> StrategyPlan {
        let mut ordered = providers.to_vec();
        ordered.sort_by_key(|descriptor| descriptor.trust_rank);

        if matches!(task.strategy, AcquisitionStrategy::ObscureFallbackHeavy) {
            ordered.sort_by_key(|descriptor| {
                (
                    !descriptor.capabilities.supports_search,
                    descriptor.trust_rank,
                )
            });
        }

        if matches!(task.strategy, AcquisitionStrategy::SingleTrackPriority) {
            ordered.sort_by_key(|descriptor| {
                (
                    !descriptor.capabilities.supports_download,
                    descriptor.trust_rank,
                )
            });
        }

        if matches!(
            task.strategy,
            AcquisitionStrategy::Standard | AcquisitionStrategy::SingleTrackPriority
        ) {
            ordered.sort_by_key(|descriptor| {
                let provider_priority = match descriptor.id.as_str() {
                    "deezer" => 0,
                    "qobuz" => 1,
                    _ => 2,
                };
                (provider_priority, descriptor.trust_rank)
            });
        }

        // Always keep slskd as final fallback. It is resilient but slower and more volatile
        // than primary providers, so only use it after higher-priority sources are exhausted.
        ordered.sort_by_key(|descriptor| descriptor.id == "slskd");

        let provider_order = ordered.into_iter().map(|descriptor| descriptor.id).collect();
        let selection_mode = match task.strategy {
            AcquisitionStrategy::Standard | AcquisitionStrategy::SingleTrackPriority => {
                CandidateSelectionMode::FirstValidWins
            }
            _ => CandidateSelectionMode::CompareAllCandidates,
        };

        let collect_multiple_candidates = matches!(
            task.strategy,
            AcquisitionStrategy::HighQualityOnly
                | AcquisitionStrategy::DiscographyBatch
                | AcquisitionStrategy::RedownloadReplaceIfBetter
                | AcquisitionStrategy::ObscureFallbackHeavy
        );

        let require_lossless = matches!(
            task.strategy,
            AcquisitionStrategy::HighQualityOnly | AcquisitionStrategy::RedownloadReplaceIfBetter
        );

        StrategyPlan {
            strategy: task.strategy,
            provider_order,
            collect_multiple_candidates,
            selection_mode,
            require_lossless,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::director::models::{NormalizedTrack, ProviderCapabilities, TrackTaskSource};

    fn provider(id: &str, trust_rank: i32, supports_search: bool) -> ProviderDescriptor {
        ProviderDescriptor {
            id: id.to_string(),
            display_name: id.to_string(),
            trust_rank,
            capabilities: ProviderCapabilities {
                supports_search,
                supports_download: true,
                supports_lossless: true,
                supports_batch: false,
            },
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
                album: None,
                track_number: None,
                disc_number: None,
                year: None,
                duration_secs: None,
                isrc: None,
            },
            strategy,
        }
    }

    #[test]
    fn planner_uses_compare_all_for_quality_strategies() {
        let planner = StrategyPlanner;
        let plan = planner.plan(
            &task(AcquisitionStrategy::HighQualityOnly),
            &[provider("fallback", 5, true), provider("primary", 1, true)],
            &DirectorConfig::default(),
        );

        assert_eq!(plan.selection_mode, CandidateSelectionMode::CompareAllCandidates);
        assert_eq!(plan.provider_order, vec!["primary".to_string(), "fallback".to_string()]);
        assert!(plan.require_lossless);
    }

    #[test]
    fn obscure_strategy_prefers_search_capable_providers() {
        let planner = StrategyPlanner;
        let plan = planner.plan(
            &task(AcquisitionStrategy::ObscureFallbackHeavy),
            &[provider("no-search", 1, false), provider("search", 5, true)],
            &DirectorConfig::default(),
        );

        assert_eq!(plan.provider_order.first().map(String::as_str), Some("search"));
    }

    #[test]
    fn standard_strategy_prefers_deezer_then_qobuz() {
        let planner = StrategyPlanner;
        let plan = planner.plan(
            &task(AcquisitionStrategy::Standard),
            &[
                provider("local_archive", 0, true),
                provider("qobuz", 5, true),
                provider("deezer", 20, true),
                provider("slskd", 1, true),
            ],
            &DirectorConfig::default(),
        );

        assert_eq!(
            plan.provider_order,
            vec![
                "deezer".to_string(),
                "qobuz".to_string(),
                "local_archive".to_string(),
                "slskd".to_string()
            ]
        );
    }

    #[test]
    fn slskd_is_last_even_when_trust_rank_is_better() {
        let planner = StrategyPlanner;
        let plan = planner.plan(
            &task(AcquisitionStrategy::SingleTrackPriority),
            &[
                provider("slskd", 0, true),
                provider("deezer", 10, true),
                provider("qobuz", 20, true),
            ],
            &DirectorConfig::default(),
        );

        assert_eq!(
            plan.provider_order,
            vec![
                "deezer".to_string(),
                "qobuz".to_string(),
                "slskd".to_string(),
            ]
        );
    }
}
