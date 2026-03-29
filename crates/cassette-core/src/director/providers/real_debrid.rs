use crate::director::error::ProviderError;
use crate::director::models::{
    CandidateAcquisition, ProviderCapabilities, ProviderDescriptor, ProviderSearchCandidate,
    TrackTask,
};
use crate::director::provider::Provider;
use crate::director::strategy::StrategyPlan;
use crate::director::temp::TaskTempContext;
use crate::sources::{count_matching_terms, is_audio_path, normalize_text};
use async_trait::async_trait;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use serde_json::Value;
use std::path::PathBuf;
use tokio::time::{sleep, Duration};
use tracing::{info, warn};

const PROVIDER_ID: &str = "real_debrid";

/// Seeding and quality qualifiers for filtering torrent search results.
#[derive(Debug, Clone)]
pub struct SeedingQualifiers {
    pub min_seeders: u32,
    pub max_torrent_size_bytes: u64,
    pub prefer_formats: Vec<String>,
    pub reject_patterns: Vec<String>,
}

impl Default for SeedingQualifiers {
    fn default() -> Self {
        Self {
            min_seeders: 3,
            max_torrent_size_bytes: 10 * 1024 * 1024 * 1024, // 10 GB
            prefer_formats: vec![
                "flac".to_string(),
                "24bit".to_string(),
                "24-bit".to_string(),
                "lossless".to_string(),
            ],
            reject_patterns: vec![
                "mp3 128".to_string(),
                "mp3 192".to_string(),
                "web-dl".to_string(),
                "video".to_string(),
            ],
        }
    }
}

#[derive(Debug, Clone)]
pub struct RealDebridProvider {
    client: reqwest::Client,
    qualifiers: SeedingQualifiers,
}

impl RealDebridProvider {
    pub fn new(api_key: String) -> Self {
        let mut headers = HeaderMap::new();
        if let Ok(value) = HeaderValue::from_str(&format!("Bearer {api_key}")) {
            headers.insert(AUTHORIZATION, value);
        }
        let client = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(Duration::from_secs(60))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        Self {
            client,
            qualifiers: SeedingQualifiers::default(),
        }
    }

    /// Search The Pirate Bay public API for FLAC torrent candidates (no auth required).
    /// Uses category 104 (FLAC) first, then falls back to 101 (Music) for broader coverage.
    async fn search_tpb(
        &self,
        artist: &str,
        album: &str,
    ) -> Result<Vec<TorrentResult>, ProviderError> {
        let query = format!("{artist} {album}");
        let encoded = urlencoding::encode(&query);

        // Try FLAC category (104) first, then general Music (101)
        for cat in ["104", "101"] {
            let url = format!("https://apibay.org/q.php?q={encoded}+FLAC&cat={cat}");

            let response = reqwest::Client::new()
                .get(&url)
                .header("User-Agent", "Mozilla/5.0")
                .send()
                .await
                .map_err(|error| ProviderError::Network {
                    provider_id: PROVIDER_ID.to_string(),
                    message: format!("TPB search failed: {error}"),
                })?;

            if !response.status().is_success() {
                continue;
            }

            let items: Vec<Value> = response.json().await.unwrap_or_default();

            // TPB returns [{"id":"0","name":"No results returned"}] when empty
            if items.len() == 1
                && items[0].get("name").and_then(Value::as_str)
                    == Some("No results returned")
            {
                continue;
            }

            let results: Vec<TorrentResult> = items
                .into_iter()
                .filter_map(|item| {
                    let title = item.get("name")?.as_str()?.to_string();
                    let hash = item.get("info_hash")?.as_str()?;
                    let seeders = item
                        .get("seeders")
                        .and_then(Value::as_str)
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0u32);
                    let size = item
                        .get("size")
                        .and_then(Value::as_str)
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0u64);

                    let encoded_title = urlencoding::encode(&title);
                    let magnet = format!(
                        "magnet:?xt=urn:btih:{hash}&dn={encoded_title}\
                         &tr=udp%3A%2F%2Ftracker.opentrackr.org%3A1337%2Fannounce\
                         &tr=udp%3A%2F%2Fopen.tracker.cl%3A1337%2Fannounce\
                         &tr=udp%3A%2F%2Ftracker.openbittorrent.com%3A6969%2Fannounce"
                    );

                    Some(TorrentResult { title, magnet, seeders, size })
                })
                .collect();

            if !results.is_empty() {
                return Ok(results);
            }
        }

        Ok(Vec::new())
    }

    /// Filter and score torrent results based on seeding qualifiers.
    fn filter_and_score(
        &self,
        results: Vec<TorrentResult>,
        artist: &str,
        title: &str,
    ) -> Vec<(i64, TorrentResult)> {
        let artist_terms = vec![artist.to_ascii_lowercase()];
        let title_terms = vec![title.to_ascii_lowercase()];

        let mut scored: Vec<(i64, TorrentResult)> = results
            .into_iter()
            .filter(|result| {
                // Minimum seeders
                if result.seeders < self.qualifiers.min_seeders {
                    return false;
                }
                // Max size
                if result.size > self.qualifiers.max_torrent_size_bytes {
                    return false;
                }
                // Reject patterns
                let lower = result.title.to_ascii_lowercase();
                for pattern in &self.qualifiers.reject_patterns {
                    if lower.contains(pattern) {
                        return false;
                    }
                }
                true
            })
            .map(|result| {
                let normalized = normalize_text(&result.title);
                let mut score = 0i64;

                // Artist/title matching
                score += (count_matching_terms(&normalized, &artist_terms) as i64) * 20;
                score += (count_matching_terms(&normalized, &title_terms) as i64) * 30;

                // Format preference bonuses
                for format in &self.qualifiers.prefer_formats {
                    if normalized.contains(format) {
                        score += 50;
                    }
                }

                // Seeder bonus (more seeders = faster resolve)
                score += (result.seeders.min(50) as i64) * 2;

                (score, result)
            })
            .collect();

        scored.sort_by(|a, b| b.0.cmp(&a.0));
        scored
    }

    /// Extract the infohash from a magnet URI.
    fn magnet_hash(magnet: &str) -> Option<String> {
        magnet
            .split("xt=urn:btih:")
            .nth(1)
            .and_then(|s| s.split('&').next())
            .map(|h| h.to_ascii_uppercase())
    }

    /// Check Real-Debrid instant availability for a list of infohashes.
    /// Returns the set of hashes that are already cached on RD's servers.
    /// Cached torrents resolve near-instantly — the first poll after addMagnet returns "downloaded".
    async fn check_instant_availability(
        &self,
        hashes: &[String],
    ) -> std::collections::HashSet<String> {
        if hashes.is_empty() {
            return std::collections::HashSet::new();
        }

        // RD accepts up to ~40 hashes at once in the URL path
        let hash_path = hashes.join("/");
        let url = format!(
            "https://api.real-debrid.com/rest/1.0/torrents/instantAvailability/{hash_path}"
        );

        let response = match self.client.get(&url).send().await {
            Ok(r) if r.status().is_success() => r,
            _ => return std::collections::HashSet::new(),
        };

        let body: Value = match response.json().await {
            Ok(v) => v,
            Err(_) => return std::collections::HashSet::new(),
        };

        // Response: { "HASH": { "rd": [ { "file_id": { "filename": "...", "filesize": N } } ] } }
        let mut cached = std::collections::HashSet::new();
        for hash in hashes {
            let hash_upper = hash.to_ascii_uppercase();
            if let Some(entry) = body.get(&hash_upper) {
                if let Some(rd_array) = entry.get("rd").and_then(Value::as_array) {
                    if !rd_array.is_empty() {
                        info!(hash = %hash_upper, "Real-Debrid instant cache hit");
                        cached.insert(hash_upper);
                    }
                }
            }
        }
        cached
    }

    /// Submit a magnet/link to Real-Debrid and wait for it to resolve.
    async fn submit_and_resolve(
        &self,
        magnet_or_link: &str,
    ) -> Result<Vec<String>, ProviderError> {
        // Step 1: Add magnet/link
        let add_response: Value = self
            .client
            .post("https://api.real-debrid.com/rest/1.0/torrents/addMagnet")
            .form(&[("magnet", magnet_or_link)])
            .send()
            .await
            .map_err(|error| self.map_network_error(error))?
            .json()
            .await
            .map_err(|error| self.map_network_error(error))?;

        let torrent_id = add_response
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                let rate_limited = add_response
                    .get("error")
                    .and_then(Value::as_str)
                    .map(|value| value.eq_ignore_ascii_case("too_many_requests"))
                    .unwrap_or(false)
                    || add_response
                        .get("error_code")
                        .and_then(Value::as_i64)
                        == Some(34);
                if rate_limited {
                    ProviderError::RateLimited {
                        provider_id: PROVIDER_ID.to_string(),
                    }
                } else {
                    ProviderError::Other {
                        provider_id: PROVIDER_ID.to_string(),
                        message: format!("No torrent ID in addMagnet response: {add_response}"),
                    }
                }
            })?
            .to_string();

        info!(torrent_id = %torrent_id, "Real-Debrid torrent submitted");

        // Step 2: Select all files
        self.client
            .post(format!(
                "https://api.real-debrid.com/rest/1.0/torrents/selectFiles/{torrent_id}"
            ))
            .form(&[("files", "all")])
            .send()
            .await
            .map_err(|error| self.map_network_error(error))?;

        // Step 3: Poll until downloaded (max 10 minutes)
        let mut links = Vec::new();
        for attempt in 0..120 {
            sleep(Duration::from_secs(5)).await;

            let info: Value = self
                .client
                .get(format!(
                    "https://api.real-debrid.com/rest/1.0/torrents/info/{torrent_id}"
                ))
                .send()
                .await
                .map_err(|error| self.map_network_error(error))?
                .json()
                .await
                .map_err(|error| self.map_network_error(error))?;

            let status = info.get("status").and_then(Value::as_str).unwrap_or("");

            match status {
                "downloaded" => {
                    if let Some(link_array) = info.get("links").and_then(Value::as_array) {
                        for link in link_array {
                            if let Some(url) = link.as_str() {
                                links.push(url.to_string());
                            }
                        }
                    }
                    break;
                }
                "error" | "dead" | "virus" => {
                    return Err(ProviderError::Other {
                        provider_id: PROVIDER_ID.to_string(),
                        message: format!("Torrent failed with status: {status}"),
                    });
                }
                _ => {
                    if attempt % 12 == 0 {
                        let progress = info.get("progress").and_then(Value::as_f64).unwrap_or(0.0);
                        info!(
                            torrent_id = %torrent_id,
                            status = %status,
                            progress = %progress,
                            "Real-Debrid torrent polling..."
                        );
                    }
                }
            }
        }

        if links.is_empty() {
            return Err(ProviderError::TemporaryOutage {
                provider_id: PROVIDER_ID.to_string(),
                message: "Torrent did not resolve within timeout".to_string(),
            });
        }

        Ok(links)
    }

    /// Unrestrict a Real-Debrid link to get a direct download URL.
    async fn unrestrict_link(&self, link: &str) -> Result<UnrestrictedLink, ProviderError> {
        let response: Value = self
            .client
            .post("https://api.real-debrid.com/rest/1.0/unrestrict/link")
            .form(&[("link", link)])
            .send()
            .await
            .map_err(|error| self.map_network_error(error))?
            .json()
            .await
            .map_err(|error| self.map_network_error(error))?;

        let download = response
            .get("download")
            .and_then(Value::as_str)
            .ok_or_else(|| ProviderError::Other {
                provider_id: PROVIDER_ID.to_string(),
                message: "No download URL in unrestrict response".to_string(),
            })?
            .to_string();

        let filename = response
            .get("filename")
            .and_then(Value::as_str)
            .unwrap_or("download")
            .to_string();

        Ok(UnrestrictedLink {
            download,
            filename,
        })
    }

    /// Download a file to the temp directory.
    async fn download_file(
        &self,
        url: &str,
        filename: &str,
        dest_dir: &std::path::Path,
    ) -> Result<PathBuf, ProviderError> {
        let response = reqwest::Client::new()
            .get(url)
            .send()
            .await
            .map_err(|error| self.map_network_error(error))?;

        if !response.status().is_success() {
            return Err(ProviderError::Network {
                provider_id: PROVIDER_ID.to_string(),
                message: format!("Download failed with HTTP {}", response.status()),
            });
        }

        let dest = dest_dir.join(sanitize(filename));
        let bytes = response
            .bytes()
            .await
            .map_err(|error| self.map_network_error(error))?;

        tokio::fs::write(&dest, &bytes)
            .await
            .map_err(|error| ProviderError::Other {
                provider_id: PROVIDER_ID.to_string(),
                message: format!("Failed to write file: {error}"),
            })?;

        Ok(dest)
    }

    /// Attempt to unpack archives (zip, rar, 7z) using 7z command.
    async fn try_unpack(
        &self,
        file_path: &std::path::Path,
        dest_dir: &std::path::Path,
    ) -> Result<bool, ProviderError> {
        let ext = file_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();

        if !matches!(ext.as_str(), "zip" | "rar" | "7z") {
            return Ok(false);
        }

        let output = tokio::process::Command::new("7z")
            .args(["x", "-y", "-o"])
            .arg(dest_dir)
            .arg(file_path)
            .output()
            .await
            .map_err(|error| ProviderError::Other {
                provider_id: PROVIDER_ID.to_string(),
                message: format!("7z extraction failed: {error}"),
            })?;

        if !output.status.success() {
            warn!(
                "7z extraction returned non-zero for {}",
                file_path.display()
            );
        }

        Ok(true)
    }

    /// Scan a directory for audio files and return the best match.
    fn find_best_audio_file(
        &self,
        dir: &std::path::Path,
        artist: &str,
        title: &str,
    ) -> Option<PathBuf> {
        let artist_key = normalize_text(artist);
        let title_key = normalize_text(title);

        let mut candidates: Vec<(i64, PathBuf)> = Vec::new();

        for entry in walkdir::WalkDir::new(dir)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if !entry.file_type().is_file() || !is_audio_path(path) {
                continue;
            }

            let filename = normalize_text(
                path.file_name()
                    .and_then(|v| v.to_str())
                    .unwrap_or_default(),
            );

            let mut score = 10i64; // base score for being an audio file
            if filename.contains(&artist_key) {
                score += 20;
            }
            if filename.contains(&title_key) {
                score += 30;
            }
            // Prefer FLAC over other formats
            if path
                .extension()
                .and_then(|e| e.to_str())
                .map_or(false, |e| e.eq_ignore_ascii_case("flac"))
            {
                score += 50;
            }

            candidates.push((score, path.to_path_buf()));
        }

        candidates.sort_by(|a, b| b.0.cmp(&a.0));
        candidates.into_iter().next().map(|(_, path)| path)
    }

    fn map_network_error(&self, error: reqwest::Error) -> ProviderError {
        if error.status().map_or(false, |s| s == 401 || s == 403) {
            ProviderError::AuthFailed {
                provider_id: PROVIDER_ID.to_string(),
            }
        } else if error.status().map_or(false, |s| s == 429) {
            ProviderError::RateLimited {
                provider_id: PROVIDER_ID.to_string(),
            }
        } else {
            ProviderError::Network {
                provider_id: PROVIDER_ID.to_string(),
                message: error.to_string(),
            }
        }
    }
}

#[async_trait]
impl Provider for RealDebridProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        ProviderDescriptor {
            id: PROVIDER_ID.to_string(),
            display_name: "Real-Debrid".to_string(),
            trust_rank: 80,
            capabilities: ProviderCapabilities {
                supports_search: true,
                supports_download: true,
                supports_lossless: true,
                supports_batch: false,
            },
        }
    }

    async fn search(
        &self,
        task: &TrackTask,
        _strategy: &StrategyPlan,
    ) -> Result<Vec<ProviderSearchCandidate>, ProviderError> {
        let album = task.target.album.as_deref().unwrap_or(&task.target.title);
        let results = self.search_tpb(&task.target.artist, album).await?;

        if results.is_empty() {
            return Ok(Vec::new());
        }

        let scored = self.filter_and_score(results, &task.target.artist, &task.target.title);

        // Batch-check instant availability for top candidates.
        // Sort cached torrents first — they resolve near-instantly vs up to 10 min for uncached.
        let mut top: Vec<(i64, TorrentResult)> = scored.into_iter().take(5).collect();
        let hashes: Vec<String> = top
            .iter()
            .filter_map(|(_, r)| Self::magnet_hash(&r.magnet))
            .collect();
        let cached = self.check_instant_availability(&hashes).await;
        if !cached.is_empty() {
            top.sort_by(|a, b| {
                let a_hit = Self::magnet_hash(&a.1.magnet)
                    .map_or(false, |h| cached.contains(&h));
                let b_hit = Self::magnet_hash(&b.1.magnet)
                    .map_or(false, |h| cached.contains(&h));
                match (b_hit, a_hit) {
                    (true, false) => std::cmp::Ordering::Greater,
                    (false, true) => std::cmp::Ordering::Less,
                    _ => b.0.cmp(&a.0),
                }
            });
        }

        Ok(top
            .into_iter()
            .take(3) // Top 3 candidates
            .map(|(_, result)| ProviderSearchCandidate {
                provider_id: PROVIDER_ID.to_string(),
                provider_candidate_id: result.magnet,
                artist: task.target.artist.clone(),
                title: task.target.title.clone(),
                album: task.target.album.clone(),
                duration_secs: task.target.duration_secs,
                extension_hint: Some("flac".to_string()),
                bitrate_kbps: None,
                cover_art_url: None,
                metadata_confidence: 0.6,
            })
            .collect())
    }

    async fn acquire(
        &self,
        task: &TrackTask,
        candidate: &ProviderSearchCandidate,
        temp_context: &TaskTempContext,
        _strategy: &StrategyPlan,
    ) -> Result<CandidateAcquisition, ProviderError> {
        // Fast path: check if this torrent is already cached on Real-Debrid.
        // Cached torrents resolve instantly — no torrent submission or polling needed.
        let magnet = &candidate.provider_candidate_id;
        let hash_opt = Self::magnet_hash(magnet);
        let is_cached = if let Some(ref hash) = hash_opt {
            !self.check_instant_availability(&[hash.clone()]).await.is_empty()
        } else {
            false
        };

        if is_cached {
            info!(task_id = %task.task_id, "Real-Debrid cache hit — instant resolution");
        } else {
            info!(task_id = %task.task_id, "Real-Debrid cache miss — submitting torrent (may take minutes)");
        }

        let links = self.submit_and_resolve(magnet).await?;

        info!(
            links_count = links.len(),
            task_id = %task.task_id,
            "Real-Debrid resolved torrent"
        );

        // Unrestrict and download each link
        let download_dir = temp_context.active_dir.join("rd-download");
        tokio::fs::create_dir_all(&download_dir)
            .await
            .map_err(|error| ProviderError::Other {
                provider_id: PROVIDER_ID.to_string(),
                message: error.to_string(),
            })?;

        for link in &links {
            match self.unrestrict_link(link).await {
                Ok(unrestricted) => {
                    let downloaded = self
                        .download_file(
                            &unrestricted.download,
                            &unrestricted.filename,
                            &download_dir,
                        )
                        .await?;

                    // Try to unpack if it's an archive
                    let _ = self.try_unpack(&downloaded, &download_dir).await;
                }
                Err(error) => {
                    warn!(link = %link, error = %error, "Failed to unrestrict link, skipping");
                }
            }
        }

        // Find the best audio file from what we downloaded/unpacked
        let best_file = self
            .find_best_audio_file(&download_dir, &task.target.artist, &task.target.title)
            .ok_or_else(|| ProviderError::UnsupportedContent {
                provider_id: PROVIDER_ID.to_string(),
                message: "No audio files found in Real-Debrid download".to_string(),
            })?;

        let extension = best_file
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("bin")
            .to_string();

        // Copy to temp context active_dir for the pipeline
        let destination = temp_context.active_dir.join(format!(
            "rd-{}.{}",
            sanitize(&task.target.title),
            extension
        ));
        tokio::fs::copy(&best_file, &destination)
            .await
            .map_err(|error| ProviderError::Other {
                provider_id: PROVIDER_ID.to_string(),
                message: format!("Failed to copy audio file: {error}"),
            })?;

        let file_size = tokio::fs::metadata(&destination)
            .await
            .map(|m| m.len())
            .unwrap_or_default();

        Ok(CandidateAcquisition {
            provider_id: PROVIDER_ID.to_string(),
            provider_candidate_id: candidate.provider_candidate_id.clone(),
            temp_path: destination,
            file_size,
            extension_hint: Some(extension),
        })
    }
}

#[derive(Debug)]
struct TorrentResult {
    title: String,
    magnet: String,
    seeders: u32,
    size: u64,
}

#[derive(Debug)]
struct UnrestrictedLink {
    download: String,
    filename: String,
}

fn sanitize(value: &str) -> String {
    value
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '\0' => '_',
            other => other,
        })
        .collect()
}
