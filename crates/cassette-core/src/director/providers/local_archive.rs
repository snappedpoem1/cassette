use crate::director::error::ProviderError;
use crate::director::models::{
    CandidateAcquisition, ProviderCapabilities, ProviderDescriptor, ProviderSearchCandidate,
    TrackTask,
};
use crate::director::provider::Provider;
use crate::director::strategy::StrategyPlan;
use crate::director::temp::TaskTempContext;
use async_trait::async_trait;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct LocalArchiveProvider {
    roots: Vec<PathBuf>,
}

impl LocalArchiveProvider {
    pub fn new(roots: Vec<PathBuf>) -> Self {
        Self { roots }
    }
}

#[async_trait]
impl Provider for LocalArchiveProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        ProviderDescriptor {
            id: "local_archive".to_string(),
            display_name: "Local Archive".to_string(),
            trust_rank: 0,
            capabilities: ProviderCapabilities {
                supports_search: true,
                supports_download: true,
                supports_lossless: true,
                supports_batch: true,
            },
        }
    }

    async fn search(
        &self,
        task: &TrackTask,
        _strategy: &StrategyPlan,
    ) -> Result<Vec<ProviderSearchCandidate>, ProviderError> {
        let roots = self.roots.clone();
        let artist = task.target.artist.clone();
        let title = task.target.title.clone();

        tokio::task::spawn_blocking(move || {
            let mut results = Vec::<ProviderSearchCandidate>::new();
            let artist_key = normalize(&artist);
            let title_key = normalize(&title);

            for root in roots {
                if !root.exists() {
                    continue;
                }

                for entry in walkdir::WalkDir::new(&root)
                    .follow_links(false)
                    .into_iter()
                    .filter_map(|entry| entry.ok())
                {
                    if !entry.file_type().is_file() {
                        continue;
                    }

                    let path = entry.into_path();
                    if !is_audio_path(&path) {
                        continue;
                    }

                    let filename = path
                        .file_stem()
                        .and_then(|value| value.to_str())
                        .unwrap_or_default();
                    let normalized_name = normalize(filename);
                    if normalized_name.contains(&artist_key) && normalized_name.contains(&title_key) {
                        let extension = path
                            .extension()
                            .and_then(|value| value.to_str())
                            .map(ToString::to_string);
                        results.push(ProviderSearchCandidate {
                            provider_id: "local_archive".to_string(),
                            provider_candidate_id: path.to_string_lossy().to_string(),
                            artist: artist.clone(),
                            title: title.clone(),
                            album: None,
                            duration_secs: None,
                            extension_hint: extension,
                            bitrate_kbps: None,
                            cover_art_url: None,
                            metadata_confidence: 0.85,
                        });
                    }
                }
            }

            Ok(results)
        })
        .await
        .map_err(|error| ProviderError::Other {
            provider_id: "local_archive".to_string(),
            message: error.to_string(),
        })?
    }

    async fn acquire(
        &self,
        _task: &TrackTask,
        candidate: &ProviderSearchCandidate,
        temp_context: &TaskTempContext,
        _strategy: &StrategyPlan,
    ) -> Result<CandidateAcquisition, ProviderError> {
        let source = PathBuf::from(&candidate.provider_candidate_id);
        let filename = source
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("candidate.bin");
        let destination = temp_context.active_dir.join(filename);

        tokio::fs::copy(&source, &destination)
            .await
            .map_err(|error| ProviderError::Network {
                provider_id: "local_archive".to_string(),
                message: error.to_string(),
            })?;

        let file_size = tokio::fs::metadata(&destination)
            .await
            .map(|metadata| metadata.len())
            .unwrap_or_default();

        Ok(CandidateAcquisition {
            provider_id: "local_archive".to_string(),
            provider_candidate_id: candidate.provider_candidate_id.clone(),
            temp_path: destination,
            file_size,
            extension_hint: candidate.extension_hint.clone(),
        })
    }
}

fn normalize(value: &str) -> String {
    value
        .to_ascii_lowercase()
        .chars()
        .map(|character| if character.is_alphanumeric() { character } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_audio_path(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str(),
        "flac" | "mp3" | "m4a" | "aac" | "ogg" | "opus" | "wav" | "aiff"
    )
}
