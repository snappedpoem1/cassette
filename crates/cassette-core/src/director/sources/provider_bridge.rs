use crate::director::config::DirectorConfig;
use crate::director::models::{
    AcquisitionStrategy, NormalizedTrack, TrackTask, TrackTaskSource,
};
use crate::director::provider::Provider;
use crate::director::sources::{ResolvedTrack, SourceError, SourceProvider};
use crate::director::strategy::StrategyPlanner;
use crate::director::temp::TempManager;
use crate::librarian::models::DesiredTrack;
use async_trait::async_trait;
use std::sync::Arc;
use uuid::Uuid;

/// Bridges the full `Provider` trait (search + acquire) into the simpler
/// `SourceProvider` trait used by `batch_download`.  When `resolve_download_url`
/// is called the bridge searches for the track, picks the best candidate,
/// acquires the file into a temp directory, and returns a `file://` URL
/// pointing to the downloaded file.
pub struct ProviderBridge {
    provider: Arc<dyn Provider>,
    config: DirectorConfig,
    name: &'static str,
}

impl ProviderBridge {
    pub fn new(provider: Arc<dyn Provider>, config: &DirectorConfig) -> Self {
        let descriptor = provider.descriptor();
        let name: &'static str = Box::leak(descriptor.id.clone().into_boxed_str());
        Self {
            provider,
            config: config.clone(),
            name,
        }
    }
}

#[async_trait]
impl SourceProvider for ProviderBridge {
    fn name(&self) -> &'static str {
        self.name
    }

    fn can_handle(&self, _track: &DesiredTrack) -> bool {
        let desc = self.provider.descriptor();
        desc.capabilities.supports_search && desc.capabilities.supports_download
    }

    async fn resolve_download_url(
        &self,
        track: &DesiredTrack,
    ) -> Result<ResolvedTrack, SourceError> {
        let task = desired_track_to_task(track);
        let planner = StrategyPlanner;
        let strategy = planner.plan(
            &task,
            &[self.provider.descriptor()],
            &self.config,
        );

        let candidates = self
            .provider
            .search(&task, &strategy)
            .await
            .map_err(|e| SourceError::ApiError(format!("{}: {}", self.name, e)))?;

        let best = candidates.first().ok_or_else(|| {
            SourceError::NotAvailable(format!(
                "{}: no candidates for {} - {}",
                self.name, track.artist_name, track.track_title
            ))
        })?;

        let temp_root = self.config.temp_root.join("bridge");
        let temp_manager = TempManager::new(
            temp_root,
            self.config.temp_recovery.clone(),
        );
        let task_id = Uuid::new_v4().to_string();
        let temp_context = temp_manager
            .prepare_task(&task_id)
            .await
            .map_err(|e| SourceError::ApiError(format!("temp setup: {e}")))?;

        let acquisition = self
            .provider
            .acquire(&task, best, &temp_context, &strategy)
            .await
            .map_err(|e| SourceError::ApiError(format!("{}: acquire failed: {}", self.name, e)))?;

        let file_url = format!("file://{}", acquisition.temp_path.to_string_lossy());

        let codec = acquisition
            .extension_hint
            .clone()
            .or_else(|| best.extension_hint.clone());
        let bitrate = best.bitrate_kbps;

        Ok(ResolvedTrack {
            download_url: file_url,
            suggested_filename: acquisition
                .temp_path
                .file_name()
                .and_then(|v| v.to_str())
                .unwrap_or("acquired.bin")
                .to_string(),
            expected_codec: codec,
            expected_bitrate: bitrate,
            expected_duration_ms: track.duration_ms.map(|v| v as u64),
            metadata: serde_json::json!({
                "source": self.name,
                "provider_candidate_id": best.provider_candidate_id,
                "metadata_confidence": best.metadata_confidence,
                "artist": best.artist,
                "title": best.title,
                "album": best.album,
            }),
        })
    }

    async fn check_availability(&self, track: &DesiredTrack) -> Result<bool, SourceError> {
        let task = desired_track_to_task(track);
        let planner = StrategyPlanner;
        let strategy = planner.plan(
            &task,
            &[self.provider.descriptor()],
            &self.config,
        );

        match self.provider.search(&task, &strategy).await {
            Ok(candidates) => Ok(!candidates.is_empty()),
            Err(_) => Ok(false),
        }
    }
}

fn desired_track_to_task(track: &DesiredTrack) -> TrackTask {
    TrackTask {
        task_id: format!("bridge-{}", track.id),
        source: track
            .source_track_id
            .as_ref()
            .filter(|id| id.starts_with("spotify:"))
            .map(|_| TrackTaskSource::SpotifyLibrary)
            .unwrap_or(TrackTaskSource::Manual),
        desired_track_id: Some(track.id),
        source_operation_id: None,
        target: NormalizedTrack {
            spotify_track_id: track.source_track_id.clone(),
            source_playlist: None,
            artist: track.artist_name.clone(),
            album_artist: None,
            title: track.track_title.clone(),
            album: track.album_title.clone(),
            track_number: track.track_number.map(|v| v as u32),
            disc_number: track.disc_number.map(|v| v as u32),
            year: None,
            duration_secs: track.duration_ms.map(|v| v as f64 / 1000.0),
            isrc: track.isrc.clone(),
        },
        strategy: AcquisitionStrategy::Standard,
    }
}
