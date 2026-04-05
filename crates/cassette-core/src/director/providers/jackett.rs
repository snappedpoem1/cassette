use crate::director::error::ProviderError;
use crate::director::models::{
    CandidateAcquisition, ProviderCapabilities, ProviderDescriptor, ProviderHealthState,
    ProviderHealthStatus, ProviderSearchCandidate, TrackTask,
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

const PROVIDER_ID: &str = "jackett";

/// Jackett-backed torrent search provider.
///
/// Searches across all Jackett-configured indexers via the Torznab API,
/// then resolves magnets through Real-Debrid for acquisition.
/// This replaces the TPB-only search in RealDebridProvider with multi-indexer coverage.
#[derive(Debug, Clone)]
pub struct JackettProvider {
    jackett_url: String,
    jackett_api_key: String,
    rd_client: reqwest::Client,
    archive_binary: String,
}

impl JackettProvider {
    pub fn new(
        jackett_url: String,
        jackett_api_key: String,
        rd_api_key: String,
        archive_binary: Option<String>,
    ) -> Self {
        let mut headers = HeaderMap::new();
        if let Ok(value) = HeaderValue::from_str(&format!("Bearer {rd_api_key}")) {
            headers.insert(AUTHORIZATION, value);
        }
        let rd_client = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(Duration::from_secs(60))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        Self {
            jackett_url,
            jackett_api_key,
            rd_client,
            archive_binary: archive_binary
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "7z".to_string()),
        }
    }

    /// Search Jackett Torznab API across all configured indexers.
    async fn search_torznab(
        &self,
        artist: &str,
        album: &str,
    ) -> Result<Vec<TorrentResult>, ProviderError> {
        let query = format!("{artist} {album} FLAC");
        let encoded = urlencoding::encode(&query);
        // cat=3000 = Audio in Torznab
        let url = format!(
            "{}/api/v2.0/indexers/all/results/torznab/?apikey={}&t=search&q={}&cat=3000",
            self.jackett_url, self.jackett_api_key, encoded
        );

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(25))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        let response = client.get(&url).send().await.map_err(|error| {
            ProviderError::Network {
                provider_id: PROVIDER_ID.to_string(),
                message: format!("Jackett search failed: {error}"),
            }
        })?;

        if !response.status().is_success() {
            let status = response.status();
            if status.as_u16() == 401 || status.as_u16() == 403 {
                return Err(ProviderError::AuthFailed {
                    provider_id: PROVIDER_ID.to_string(),
                });
            }
            return Err(ProviderError::Network {
                provider_id: PROVIDER_ID.to_string(),
                message: format!("Jackett returned HTTP {status}"),
            });
        }

        let text = response.text().await.map_err(|error| ProviderError::Network {
            provider_id: PROVIDER_ID.to_string(),
            message: format!("Jackett body read failed: {error}"),
        })?;

        Ok(parse_torznab_results(&text, artist, album))
    }

    /// Filter and score torrent results for quality and relevance.
    fn filter_and_score(
        results: Vec<TorrentResult>,
        artist: &str,
        album: &str,
    ) -> Vec<(i64, TorrentResult)> {
        let artist_terms = vec![artist.to_ascii_lowercase()];
        let album_terms = vec![album.to_ascii_lowercase()];

        let mut scored: Vec<(i64, TorrentResult)> = results
            .into_iter()
            .filter(|r| {
                r.seeders >= 2 && r.size <= 10 * 1024 * 1024 * 1024 // min 2 seeders, max 10 GB
            })
            .map(|r| {
                let normalized = normalize_text(&r.title);
                let mut score = 0i64;

                score += (count_matching_terms(&normalized, &artist_terms) as i64) * 20;
                score += (count_matching_terms(&normalized, &album_terms) as i64) * 30;

                // Format bonuses
                if normalized.contains("flac") {
                    score += 50;
                }
                if normalized.contains("24bit")
                    || normalized.contains("24-bit")
                    || normalized.contains("24 bit")
                {
                    score += 20;
                }
                if normalized.contains("lossless") {
                    score += 30;
                }

                // Seeder bonus (more = faster RD resolve)
                score += (r.seeders.min(50) as i64) * 2;

                // Penalise oversized torrents
                if r.size > 5 * 1024 * 1024 * 1024 {
                    score -= 30;
                }

                // Reject patterns
                let lower = r.title.to_ascii_lowercase();
                for pattern in &["mp3 128", "mp3 192", "web-dl", "video"] {
                    if lower.contains(pattern) {
                        score -= 200;
                    }
                }

                (score, r)
            })
            .filter(|(score, _)| *score > 0)
            .collect();

        scored.sort_by(|a, b| b.0.cmp(&a.0));
        scored
    }

    // ── Real-Debrid resolution (shared with RealDebridProvider) ──

    fn magnet_hash(magnet: &str) -> Option<String> {
        magnet
            .split("xt=urn:btih:")
            .nth(1)
            .and_then(|s| s.split('&').next())
            .map(|h| h.to_ascii_uppercase())
    }

    async fn check_instant_availability(
        &self,
        hashes: &[String],
    ) -> std::collections::HashSet<String> {
        if hashes.is_empty() {
            return std::collections::HashSet::new();
        }
        let hash_path = hashes.join("/");
        let url = format!(
            "https://api.real-debrid.com/rest/1.0/torrents/instantAvailability/{hash_path}"
        );

        let response = match self.rd_client.get(&url).send().await {
            Ok(r) if r.status().is_success() => r,
            _ => return std::collections::HashSet::new(),
        };

        let body: Value = match response.json().await {
            Ok(v) => v,
            Err(_) => return std::collections::HashSet::new(),
        };

        let mut cached = std::collections::HashSet::new();
        for hash in hashes {
            let hash_upper = hash.to_ascii_uppercase();
            if let Some(entry) = body.get(&hash_upper) {
                if let Some(rd_array) = entry.get("rd").and_then(Value::as_array) {
                    if !rd_array.is_empty() {
                        info!(hash = %hash_upper, "Jackett+RD instant cache hit");
                        cached.insert(hash_upper);
                    }
                }
            }
        }
        cached
    }

    async fn find_existing_torrent(&self, hash: &str) -> Option<String> {
        let hash_upper = hash.to_ascii_uppercase();
        let url = "https://api.real-debrid.com/rest/1.0/torrents?limit=100&page=1";
        let response = self.rd_client.get(url).send().await.ok()?;
        if !response.status().is_success() {
            return None;
        }
        let items: Vec<Value> = response.json().await.ok()?;
        items.into_iter().find_map(|item| {
            let item_hash = item.get("hash")?.as_str()?.to_ascii_uppercase();
            if item_hash == hash_upper {
                item.get("id")?.as_str().map(|s| s.to_string())
            } else {
                None
            }
        })
    }

    async fn submit_and_resolve(&self, magnet: &str) -> Result<Vec<String>, ProviderError> {
        let existing_id = if let Some(hash) = Self::magnet_hash(magnet) {
            self.find_existing_torrent(&hash).await
        } else {
            None
        };

        let torrent_id = if let Some(id) = existing_id {
            info!(torrent_id = %id, "RD torrent already exists — reusing");
            id
        } else {
            let add_response: Value = self
                .rd_client
                .post("https://api.real-debrid.com/rest/1.0/torrents/addMagnet")
                .form(&[("magnet", magnet)])
                .send()
                .await
                .map_err(|e| self.map_rd_error(e))?
                .json()
                .await
                .map_err(|e| self.map_rd_error(e))?;

            let rate_limited = add_response
                .get("error")
                .and_then(Value::as_str)
                .map(|v| v.eq_ignore_ascii_case("too_many_requests"))
                .unwrap_or(false)
                || add_response.get("error_code").and_then(Value::as_i64) == Some(34);

            add_response
                .get("id")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    if rate_limited {
                        ProviderError::RateLimited {
                            provider_id: PROVIDER_ID.to_string(),
                        }
                    } else {
                        ProviderError::Other {
                            provider_id: PROVIDER_ID.to_string(),
                            message: format!(
                                "No torrent ID in addMagnet response: {add_response}"
                            ),
                        }
                    }
                })?
                .to_string()
        };

        // Select all files
        self.rd_client
            .post(format!(
                "https://api.real-debrid.com/rest/1.0/torrents/selectFiles/{torrent_id}"
            ))
            .form(&[("files", "all")])
            .send()
            .await
            .map_err(|e| self.map_rd_error(e))?;

        // Poll until downloaded (max 10 min)
        let mut links = Vec::new();
        for attempt in 0..120 {
            sleep(Duration::from_secs(5)).await;

            let info: Value = self
                .rd_client
                .get(format!(
                    "https://api.real-debrid.com/rest/1.0/torrents/info/{torrent_id}"
                ))
                .send()
                .await
                .map_err(|e| self.map_rd_error(e))?
                .json()
                .await
                .map_err(|e| self.map_rd_error(e))?;

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
                        let progress =
                            info.get("progress").and_then(Value::as_f64).unwrap_or(0.0);
                        info!(
                            torrent_id = %torrent_id,
                            status = %status,
                            progress = %progress,
                            "Jackett+RD torrent polling..."
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

    async fn unrestrict_link(&self, link: &str) -> Result<(String, String), ProviderError> {
        let response: Value = self
            .rd_client
            .post("https://api.real-debrid.com/rest/1.0/unrestrict/link")
            .form(&[("link", link)])
            .send()
            .await
            .map_err(|e| self.map_rd_error(e))?
            .json()
            .await
            .map_err(|e| self.map_rd_error(e))?;

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

        Ok((download, filename))
    }

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
            .map_err(|e| self.map_rd_error(e))?;

        if !response.status().is_success() {
            return Err(ProviderError::Network {
                provider_id: PROVIDER_ID.to_string(),
                message: format!("Download failed with HTTP {}", response.status()),
            });
        }

        let dest = dest_dir.join(sanitize(filename));
        let bytes = response.bytes().await.map_err(|e| self.map_rd_error(e))?;
        tokio::fs::write(&dest, &bytes)
            .await
            .map_err(|error| ProviderError::Other {
                provider_id: PROVIDER_ID.to_string(),
                message: format!("Failed to write file: {error}"),
            })?;

        Ok(dest)
    }

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

        let output = tokio::process::Command::new(&self.archive_binary)
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

    fn find_best_audio_file(dir: &std::path::Path, artist: &str, title: &str) -> Option<PathBuf> {
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

            let mut score = 10i64;
            if filename.contains(&artist_key) {
                score += 20;
            }
            if filename.contains(&title_key) {
                score += 30;
            }
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

    fn map_rd_error(&self, error: reqwest::Error) -> ProviderError {
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
impl Provider for JackettProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        ProviderDescriptor {
            id: PROVIDER_ID.to_string(),
            display_name: "Jackett (multi-indexer)".to_string(),
            // Between Usenet (30) and yt-dlp (50): better than yt-dlp, less trusted than direct APIs
            trust_rank: 40,
            capabilities: ProviderCapabilities {
                supports_search: true,
                supports_download: true,
                supports_lossless: true,
                supports_batch: false,
            },
        }
    }

    async fn health_check(&self) -> Result<ProviderHealthState, ProviderError> {
        // Ping Jackett server info endpoint
        let url = format!(
            "{}/api/v2.0/server/config?apikey={}",
            self.jackett_url, self.jackett_api_key
        );
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        match client.get(&url).send().await {
            Ok(r) if r.status().is_success() => Ok(ProviderHealthState {
                provider_id: PROVIDER_ID.to_string(),
                status: ProviderHealthStatus::Healthy,
                checked_at: chrono::Utc::now(),
                message: None,
            }),
            Ok(r) => Ok(ProviderHealthState {
                provider_id: PROVIDER_ID.to_string(),
                status: ProviderHealthStatus::Down,
                checked_at: chrono::Utc::now(),
                message: Some(format!("Jackett returned HTTP {}", r.status())),
            }),
            Err(e) => Ok(ProviderHealthState {
                provider_id: PROVIDER_ID.to_string(),
                status: ProviderHealthStatus::Down,
                checked_at: chrono::Utc::now(),
                message: Some(format!("Jackett unreachable: {e}")),
            }),
        }
    }

    async fn search(
        &self,
        task: &TrackTask,
        _strategy: &StrategyPlan,
    ) -> Result<Vec<ProviderSearchCandidate>, ProviderError> {
        // Torrent indexes overwhelmingly organize by release. If we do not have
        // a concrete album, skip this provider rather than doing weak song-only searches.
        let Some(album) = task
            .target
            .album
            .as_deref()
            .map(str::trim)
            .filter(|album| !album.is_empty())
        else {
            info!(task_id = %task.task_id, "jackett search skipped: missing album metadata");
            return Ok(Vec::new());
        };
        let results = self.search_torznab(&task.target.artist, album).await?;

        if results.is_empty() {
            return Ok(Vec::new());
        }

        let scored = Self::filter_and_score(results, &task.target.artist, album);

        // Batch-check instant availability — sort cached first for faster resolve
        let mut top: Vec<(i64, TorrentResult)> = scored.into_iter().take(8).collect();
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
            .take(3)
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
                metadata_confidence: 0.65,
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
        let magnet = &candidate.provider_candidate_id;

        let hash_opt = Self::magnet_hash(magnet);
        let is_cached = if let Some(ref hash) = hash_opt {
            !self.check_instant_availability(&[hash.clone()]).await.is_empty()
        } else {
            false
        };

        if is_cached {
            info!(task_id = %task.task_id, "Jackett+RD cache hit — instant resolution");
        } else {
            info!(task_id = %task.task_id, "Jackett+RD cache miss — submitting torrent");
        }

        let links = self.submit_and_resolve(magnet).await?;

        info!(
            links_count = links.len(),
            task_id = %task.task_id,
            "Jackett+RD resolved torrent"
        );

        let download_dir = temp_context.active_dir.join("jackett-download");
        tokio::fs::create_dir_all(&download_dir)
            .await
            .map_err(|error| ProviderError::Other {
                provider_id: PROVIDER_ID.to_string(),
                message: error.to_string(),
            })?;

        for link in &links {
            match self.unrestrict_link(link).await {
                Ok((download_url, filename)) => {
                    let downloaded = self
                        .download_file(&download_url, &filename, &download_dir)
                        .await?;
                    let _ = self.try_unpack(&downloaded, &download_dir).await;
                }
                Err(error) => {
                    warn!(link = %link, error = %error, "Failed to unrestrict link, skipping");
                }
            }
        }

        let best_file =
            Self::find_best_audio_file(&download_dir, &task.target.artist, &task.target.title)
                .ok_or_else(|| ProviderError::UnsupportedContent {
                    provider_id: PROVIDER_ID.to_string(),
                    message: "No audio files found in Jackett+RD download".to_string(),
                })?;

        let extension = best_file
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("bin")
            .to_string();

        let destination = temp_context.active_dir.join(format!(
            "jackett-{}.{}",
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
            resolved_metadata: None,
        })
    }
}

// ── Torznab XML parsing ─────────────────────────────────────────────────────

#[derive(Debug)]
struct TorrentResult {
    title: String,
    magnet: String,
    seeders: u32,
    size: u64,
}

/// Parse Torznab RSS XML into scored torrent results.
/// Requires album title words to match in the torrent title.
fn parse_torznab_results(xml: &str, _artist: &str, album: &str) -> Vec<TorrentResult> {
    let album_n = normalize_text(album);
    let album_words: Vec<String> = album_n.split_whitespace().map(|s| s.to_string()).collect();

    let mut results = Vec::new();

    for item_block in xml.split("<item>").skip(1) {
        let end = item_block.find("</item>").unwrap_or(item_block.len());
        let item = &item_block[..end];

        let title = match extract_xml_text(item, "title") {
            Some(t) => t
                .replace("<![CDATA[", "")
                .replace("]]>", "")
                .trim()
                .to_string(),
            None => continue,
        };

        let seeders = extract_torznab_attr(item, "seeders")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0u32);

        let size: u64 = extract_xml_text(item, "size")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        // Extract magnet: torznab:attr magneturl → guid → infohash → skip
        let magnet = extract_torznab_attr(item, "magneturl")
            .or_else(|| extract_xml_text(item, "guid").filter(|s| s.starts_with("magnet:")))
            .or_else(|| {
                extract_torznab_attr(item, "infohash").map(|h| {
                    let enc = urlencoding::encode(&title);
                    format!(
                        "magnet:?xt=urn:btih:{h}&dn={enc}\
                         &tr=udp%3A%2F%2Ftracker.opentrackr.org%3A1337%2Fannounce"
                    )
                })
            });

        let Some(magnet) = magnet else { continue };

        // Require album words to match
        let t = normalize_text(&title);
        let has_album = album_words
            .iter()
            .all(|w| t.split_whitespace().any(|tw| tw == w.as_str()));
        if !has_album {
            continue;
        }

        results.push(TorrentResult {
            title,
            magnet,
            seeders,
            size,
        });
    }

    results
}

/// Extract text content from a simple XML tag.
fn extract_xml_text(block: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let start = block.find(&open)?;
    let inner_start = block[start..].find('>')? + start + 1;
    let end = block[inner_start..].find(&close)? + inner_start;
    Some(block[inner_start..end].trim().to_string())
}

/// Extract a torznab:attr value by name.
fn extract_torznab_attr(block: &str, name: &str) -> Option<String> {
    let needle = format!("\"{name}\"");
    for chunk in block.split("<torznab:attr") {
        if chunk.contains(&needle) {
            let tag_str = format!("<torznab:attr{chunk}");
            let attr_pat = "value=\"";
            let a_start = tag_str.find(attr_pat)? + attr_pat.len();
            let a_end = tag_str[a_start..].find('"')? + a_start;
            return Some(tag_str[a_start..a_end].to_string());
        }
    }
    None
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_torznab_extracts_results() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss><channel>
<item>
<title>Radiohead - OK Computer FLAC</title>
<size>350000000</size>
<guid>magnet:?xt=urn:btih:ABCDEF123456&amp;dn=test</guid>
<torznab:attr name="seeders" value="15"/>
</item>
<item>
<title>Radiohead - Kid A MP3</title>
<size>100000000</size>
<guid>magnet:?xt=urn:btih:DEADBEEF&amp;dn=test2</guid>
<torznab:attr name="seeders" value="5"/>
</item>
</channel></rss>"#;

        let results = parse_torznab_results(xml, "Radiohead", "OK Computer");
        assert_eq!(results.len(), 1); // Only "OK Computer" matches
        assert!(results[0].title.contains("OK Computer"));
        assert_eq!(results[0].seeders, 15);
    }

    #[test]
    fn magnet_hash_extraction() {
        let magnet = "magnet:?xt=urn:btih:ABCDEF123456&dn=test";
        assert_eq!(
            JackettProvider::magnet_hash(magnet),
            Some("ABCDEF123456".to_string())
        );
    }

    #[test]
    fn filter_and_score_rejects_low_seeders() {
        let results = vec![TorrentResult {
            title: "Artist - Album FLAC".to_string(),
            magnet: "magnet:?xt=urn:btih:ABC".to_string(),
            seeders: 1,
            size: 500_000_000,
        }];
        let scored = JackettProvider::filter_and_score(results, "Artist", "Album");
        assert!(scored.is_empty());
    }
}
