use crate::director::error::ProviderError;
use crate::director::models::{
    CandidateAcquisition, ProviderCapabilities, ProviderDescriptor, ProviderSearchCandidate,
    TrackTask,
};
use crate::director::provider::Provider;
use crate::director::strategy::StrategyPlan;
use crate::director::temp::TaskTempContext;
use async_trait::async_trait;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct YtDlpProvider {
    binary: String,
}

impl YtDlpProvider {
    pub fn new(binary: impl Into<String>) -> Self {
        Self {
            binary: binary.into(),
        }
    }
}

#[async_trait]
impl Provider for YtDlpProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        ProviderDescriptor {
            id: "yt_dlp".to_string(),
            display_name: "yt-dlp".to_string(),
            trust_rank: 50,
            capabilities: ProviderCapabilities {
                supports_search: true,
                supports_download: true,
                supports_lossless: false,
                supports_batch: false,
            },
        }
    }

    async fn search(
        &self,
        task: &TrackTask,
        _strategy: &StrategyPlan,
    ) -> Result<Vec<ProviderSearchCandidate>, ProviderError> {
        let version_check = tokio::process::Command::new(&self.binary)
            .arg("--version")
            .output()
            .await
            .map_err(|_error| ProviderError::NotFound {
                provider_id: "yt_dlp".to_string(),
            })?;
        if !version_check.status.success() {
            return Err(ProviderError::NotFound {
                provider_id: "yt_dlp".to_string(),
            });
        }

        Ok(vec![ProviderSearchCandidate {
            provider_id: "yt_dlp".to_string(),
            provider_candidate_id: format!(
                "ytsearch1:{} audio",
                build_query(&task.target.artist, &task.target.title, task.target.album.as_deref())
            ),
            artist: task.target.artist.clone(),
            title: task.target.title.clone(),
            album: task.target.album.clone(),
            duration_secs: task.target.duration_secs,
            extension_hint: Some("mp3".to_string()),
            bitrate_kbps: Some(320),
            cover_art_url: None,
            metadata_confidence: 0.60,
        }])
    }

    async fn acquire(
        &self,
        task: &TrackTask,
        candidate: &ProviderSearchCandidate,
        temp_context: &TaskTempContext,
        _strategy: &StrategyPlan,
    ) -> Result<CandidateAcquisition, ProviderError> {
        let output_stem = sanitize_component(&format!("{} - {}", task.target.artist, task.target.title));
        let output_template = temp_context
            .active_dir
            .join(format!("{output_stem}.%(ext)s"));
        let output_template_string = output_template.to_string_lossy().to_string();

        #[cfg(windows)]
        let output = tokio::process::Command::new(&self.binary)
            .creation_flags(0x08000000)
            .args([
                "--extract-audio",
                "--audio-format",
                "mp3",
                "--audio-quality",
                "0",
                "--no-playlist",
                "--max-downloads",
                "1",
                "-o",
                &output_template_string,
                &candidate.provider_candidate_id,
            ])
            .output()
            .await
            .map_err(|error| ProviderError::Network {
                provider_id: "yt_dlp".to_string(),
                message: error.to_string(),
            })?;

        #[cfg(not(windows))]
        let output = tokio::process::Command::new(&self.binary)
            .args([
                "--extract-audio",
                "--audio-format",
                "mp3",
                "--audio-quality",
                "0",
                "--no-playlist",
                "--max-downloads",
                "1",
                "-o",
                &output_template_string,
                &candidate.provider_candidate_id,
            ])
            .output()
            .await
            .map_err(|error| ProviderError::Network {
                provider_id: "yt_dlp".to_string(),
                message: error.to_string(),
            })?;

        if !output.status.success() {
            return Err(ProviderError::Other {
                provider_id: "yt_dlp".to_string(),
                message: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            });
        }

        let mut entries = tokio::fs::read_dir(&temp_context.active_dir)
            .await
            .map_err(|error| ProviderError::Other {
                provider_id: "yt_dlp".to_string(),
                message: error.to_string(),
            })?;

        // Select the file with the most recent mtime — avoids depending on OS directory
        // iteration order, which is undefined and silently wrong on some filesystems.
        let mut newest = None::<(PathBuf, std::time::SystemTime)>;
        while let Some(entry) = entries.next_entry().await.map_err(|error| ProviderError::Other {
            provider_id: "yt_dlp".to_string(),
            message: error.to_string(),
        })? {
            let path = entry.path();
            if !path.is_file() { continue; }
            let mtime = tokio::fs::metadata(&path).await
                .ok()
                .and_then(|m| m.modified().ok())
                .unwrap_or(std::time::UNIX_EPOCH);
            match &newest {
                None => { newest = Some((path, mtime)); }
                Some((_, prev_mtime)) if mtime > *prev_mtime => { newest = Some((path, mtime)); }
                _ => {}
            }
        }
        let newest = newest.map(|(path, _)| path);

        let temp_path = newest.ok_or_else(|| ProviderError::Other {
            provider_id: "yt_dlp".to_string(),
            message: "yt-dlp reported success but no output file was found".to_string(),
        })?;
        let file_size = tokio::fs::metadata(&temp_path)
            .await
            .map(|metadata| metadata.len())
            .unwrap_or_default();

        Ok(CandidateAcquisition {
            provider_id: "yt_dlp".to_string(),
            provider_candidate_id: candidate.provider_candidate_id.clone(),
            temp_path,
            file_size,
            extension_hint: Some("mp3".to_string()),
        })
    }
}

fn build_query(artist: &str, title: &str, album: Option<&str>) -> String {
    match album.filter(|value| !value.trim().is_empty()) {
        Some(album) => format!("{artist} {title} {album}"),
        None => format!("{artist} {title}"),
    }
}

fn sanitize_component(value: &str) -> String {
    value
        .trim()
        .chars()
        .map(|character| match character {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '\0' => '_',
            other => other,
        })
        .collect::<String>()
        .trim()
        .trim_end_matches('.')
        .to_string()
}
