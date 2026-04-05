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
    pub compare_after_first_quality_gate: bool,
}

#[derive(Debug, Default, Clone)]
pub struct StrategyPlanner;

impl StrategyPlanner {
    pub fn plan(
        &self,
        task: &TrackTask,
        providers: &[ProviderDescriptor],
        config: &DirectorConfig,
    ) -> StrategyPlan {
        let mut ordered = providers.to_vec();

        // Strategy-specific waterfall orders tuned to each provider's strengths
        match task.strategy {
            AcquisitionStrategy::Standard | AcquisitionStrategy::DiscographyBatch => {
                // Mainstream albums: quality-first, lossy last
                // Qobuz → Deezer → Local Archive → Usenet → Jackett → Real Debrid → Slskd → yt-dlp
                ordered.sort_by_key(|d| match d.id.as_str() {
                    "qobuz" => 0,
                    "deezer" => 1,
                    "local_archive" => 2,
                    "usenet" => 3,
                    "jackett" => 4,
                    "real_debrid" => 5,
                    "slskd" => 6,
                    "yt_dlp" => 7,
                    _ => 8,
                });
            }
            AcquisitionStrategy::HighQualityOnly
            | AcquisitionStrategy::RedownloadReplaceIfBetter => {
                // Lossless only: yt-dlp excluded entirely
                // Qobuz → Deezer → Usenet → Local Archive → Jackett → Real Debrid → Slskd
                ordered.sort_by_key(|d| match d.id.as_str() {
                    "qobuz" => 0,
                    "deezer" => 1,
                    "usenet" => 2,
                    "local_archive" => 3,
                    "jackett" => 4,
                    "real_debrid" => 5,
                    "slskd" => 6,
                    "yt_dlp" => 99, // effectively excluded by require_lossless
                    _ => 7,
                });
            }
            AcquisitionStrategy::ObscureFallbackHeavy => {
                // Rare/out-of-print: deep catalog providers promoted
                // Local Archive → Jackett → Real Debrid → Slskd → Usenet → Deezer → Qobuz → yt-dlp
                ordered.sort_by_key(|d| match d.id.as_str() {
                    "local_archive" => 0,
                    "jackett" => 1,
                    "real_debrid" => 2,
                    "slskd" => 3,
                    "usenet" => 4,
                    "deezer" => 5,
                    "qobuz" => 6,
                    "yt_dlp" => 7,
                    _ => 8,
                });
            }
            AcquisitionStrategy::SingleTrackPriority => {
                // Individual tracks/remixes: yt-dlp promoted for community content
                // Deezer → Qobuz → yt-dlp → Slskd → Jackett → Real Debrid → Local Archive
                ordered.sort_by_key(|d| match d.id.as_str() {
                    "deezer" => 0,
                    "qobuz" => 1,
                    "yt_dlp" => 2,
                    "slskd" => 3,
                    "jackett" => 4,
                    "real_debrid" => 5,
                    "local_archive" => 6,
                    _ => 7,
                });
            }
            _ => {
                // Default: trust_rank order
                ordered.sort_by_key(|d| d.trust_rank);
            }
        }

        let provider_order = ordered.into_iter().map(|descriptor| descriptor.id).collect();
        let parallel_n = config.parallel_provider_count.max(1);
        let selection_mode = match task.strategy {
            AcquisitionStrategy::SingleTrackPriority => CandidateSelectionMode::FirstValidWins,
            AcquisitionStrategy::Standard | AcquisitionStrategy::DiscographyBatch => {
                CandidateSelectionMode::CompareTopN(parallel_n)
            }
            _ => CandidateSelectionMode::CompareAllCandidates,
        };

        let collect_multiple_candidates = !matches!(
            selection_mode,
            CandidateSelectionMode::FirstValidWins
        );

        // For compare modes, first evaluate the top-ranked provider and only continue
        // comparing other providers when the first valid candidate does not meet the
        // expected high-quality floor.
        let compare_after_first_quality_gate = matches!(
            task.strategy,
            AcquisitionStrategy::Standard
                | AcquisitionStrategy::DiscographyBatch
                | AcquisitionStrategy::HighQualityOnly
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
            compare_after_first_quality_gate,
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
                album: None,
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
            },
            strategy,
        }
    }

    #[test]
    fn planner_uses_compare_all_for_quality_strategies() {
        let planner = StrategyPlanner;
        let plan = planner.plan(
            &task(AcquisitionStrategy::HighQualityOnly),
            &[provider("deezer", 5, true), provider("qobuz", 1, true)],
            &DirectorConfig::default(),
        );

        assert_eq!(plan.selection_mode, CandidateSelectionMode::CompareAllCandidates);
        // HighQualityOnly: Qobuz before Deezer
        assert_eq!(plan.provider_order, vec!["qobuz".to_string(), "deezer".to_string()]);
        assert!(plan.require_lossless);
    }

    #[test]
    fn obscure_strategy_promotes_real_debrid_and_slskd() {
        let planner = StrategyPlanner;
        let plan = planner.plan(
            &task(AcquisitionStrategy::ObscureFallbackHeavy),
            &[
                provider("qobuz", 10, true),
                provider("real_debrid", 80, true),
                provider("slskd", 90, true),
            ],
            &DirectorConfig::default(),
        );

        // ObscureFallbackHeavy: Real Debrid and Slskd promoted before Qobuz
        assert_eq!(
            plan.provider_order,
            vec!["real_debrid".to_string(), "slskd".to_string(), "qobuz".to_string()]
        );
    }

    #[test]
    fn standard_strategy_prefers_qobuz_then_deezer() {
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

        // Standard: Qobuz → Deezer → Local Archive → Slskd
        assert_eq!(
            plan.provider_order,
            vec![
                "qobuz".to_string(),
                "deezer".to_string(),
                "local_archive".to_string(),
                "slskd".to_string()
            ]
        );
    }

    #[test]
    fn single_track_promotes_yt_dlp() {
        let planner = StrategyPlanner;
        let plan = planner.plan(
            &task(AcquisitionStrategy::SingleTrackPriority),
            &[
                provider("slskd", 0, true),
                provider("deezer", 10, true),
                provider("qobuz", 20, true),
                provider("yt_dlp", 50, true),
            ],
            &DirectorConfig::default(),
        );

        // SingleTrackPriority: Deezer → Qobuz → yt-dlp → Slskd
        assert_eq!(
            plan.provider_order,
            vec![
                "deezer".to_string(),
                "qobuz".to_string(),
                "yt_dlp".to_string(),
                "slskd".to_string(),
            ]
        );
    }
}
