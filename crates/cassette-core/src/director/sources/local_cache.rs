use crate::director::sources::{ResolvedTrack, SourceError, SourceProvider};
use crate::librarian::models::DesiredTrack;
use async_trait::async_trait;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct LocalCacheSource {
    roots: Vec<PathBuf>,
}

impl LocalCacheSource {
    pub fn new(roots: Vec<PathBuf>) -> Self {
        Self { roots }
    }
}

#[async_trait]
impl SourceProvider for LocalCacheSource {
    fn name(&self) -> &'static str {
        "local_cache"
    }

    fn can_handle(&self, _track: &DesiredTrack) -> bool {
        !self.roots.is_empty()
    }

    async fn resolve_download_url(&self, track: &DesiredTrack) -> Result<ResolvedTrack, SourceError> {
        let roots = self.roots.clone();
        let artist = track.artist_name.clone();
        let title = track.track_title.clone();

        let found = tokio::task::spawn_blocking(move || {
            for root in roots {
                if !root.exists() {
                    continue;
                }
                for entry in walkdir::WalkDir::new(root)
                    .follow_links(false)
                    .into_iter()
                    .filter_map(|e| e.ok())
                {
                    if !entry.file_type().is_file() {
                        continue;
                    }
                    let path = entry.path();
                    let stem = path
                        .file_stem()
                        .and_then(|v| v.to_str())
                        .unwrap_or_default()
                        .to_ascii_lowercase();
                    if stem.contains(&artist.to_ascii_lowercase())
                        && stem.contains(&title.to_ascii_lowercase())
                    {
                        return Some(path.to_path_buf());
                    }
                }
            }
            None
        })
        .await
        .map_err(|error| SourceError::ApiError(error.to_string()))?;

        let path = found.ok_or_else(|| {
            SourceError::NotAvailable(format!(
                "No local cache match for {} - {}",
                track.artist_name, track.track_title
            ))
        })?;

        let extension = path
            .extension()
            .and_then(|v| v.to_str())
            .unwrap_or("bin");

        Ok(ResolvedTrack {
            download_url: format!("file://{}", path.to_string_lossy()),
            suggested_filename: path
                .file_name()
                .and_then(|v| v.to_str())
                .unwrap_or("cached.bin")
                .to_string(),
            expected_codec: Some(extension.to_string()),
            expected_bitrate: None,
            expected_duration_ms: track.duration_ms.map(|v| v as u64),
            metadata: serde_json::json!({"source": "local_cache", "path": path.to_string_lossy()}),
        })
    }

    async fn check_availability(&self, track: &DesiredTrack) -> Result<bool, SourceError> {
        self.resolve_download_url(track).await.map(|_| true)
    }
}
