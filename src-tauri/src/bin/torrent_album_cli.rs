/// torrent_album_cli — album-first torrent downloader via Real-Debrid
///
/// Flow per album:
///   1. Search TPB for "artist album FLAC"
///   2. Add best magnet to Real-Debrid (dedup by hash)
///   3. Wait for RD to finish downloading
///   4. Unrestrict all audio links
///   5. Download audio files into a staging folder
///   6. Match downloaded files to expected tracks by track number / title similarity
///   7. Copy matched files into the library, write tags, upsert to DB
///
/// Usage:
///   torrent_album_cli [--dry-run] [--limit N] [--min-plays N] [--staging PATH]
///   torrent_album_cli --album "Artist" "Album Title"
use cassette_core::db::Db;
use cassette_core::library::read_track_metadata;
use cassette_core::sources::normalize_text;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use serde_json::Value;
use std::path::PathBuf;
use tokio::time::{sleep, Duration};
use tracing::{info, warn};

// ── helpers ─────────────────────────────────────────────────────────────────

fn app_db_path() -> PathBuf {
    let app_data = std::env::var("APPDATA").unwrap_or_default();
    PathBuf::from(app_data).join("dev.cassette.app").join("cassette.db")
}

fn read_setting(db: &Db, key: &str) -> Option<String> {
    db.get_setting(key)
        .ok()
        .flatten()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .or_else(|| {
            std::env::var(key.to_ascii_uppercase())
                .ok()
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
        })
}

fn is_audio(path: &std::path::Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref(),
        Some("flac" | "mp3" | "m4a" | "aac" | "ogg" | "opus" | "wav" | "aiff" | "alac")
    )
}

fn is_archive(path: &std::path::Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref(),
        Some("rar" | "zip" | "7z" | "tar" | "gz")
    )
}

/// Extract an archive to dest_dir using 7-Zip, return list of extracted audio files.
fn extract_archive(archive: &std::path::Path, dest_dir: &std::path::Path) -> Vec<PathBuf> {
    let sevenz = PathBuf::from("C:/Program Files/7-Zip/7z.exe");
    if !sevenz.exists() {
        warn!("7z.exe not found at {:?}", sevenz);
        return Vec::new();
    }
    let status = std::process::Command::new(&sevenz)
        .args(["e", "-y", &archive.to_string_lossy(), &format!("-o{}", dest_dir.to_string_lossy()), "*"])
        .output();
    match status {
        Ok(out) if out.status.success() => {}
        Ok(out) => {
            warn!("7z extraction failed: {}", String::from_utf8_lossy(&out.stderr));
            return Vec::new();
        }
        Err(e) => {
            warn!("7z spawn failed: {e}");
            return Vec::new();
        }
    }
    // Collect audio files from dest_dir
    let mut found = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dest_dir) {
        for entry in entries.flatten() {
            if is_audio(&entry.path()) {
                found.push(entry.path());
            }
        }
    }
    found.sort();
    found
}

/// Simple title similarity: fraction of words in `needle` found in `haystack`.
fn title_similarity(needle: &str, haystack: &str) -> f64 {
    let n = normalize_text(needle);
    let h = normalize_text(haystack);
    let words: Vec<&str> = n.split_whitespace().collect();
    if words.is_empty() {
        return 0.0;
    }
    let matches = words.iter().filter(|w| h.contains(*w)).count();
    matches as f64 / words.len() as f64
}

/// Returns true if the album title looks like a single, not a proper album.
fn is_single_not_album(album: &str) -> bool {
    let t = album.to_ascii_lowercase();
    // Collaboration singles: "Title (feat. X)", "Title (with X)", "Title (ft. X)"
    let feat_patterns = ["(feat.", "(with ", "(ft.", "ft. ", "feat. "];
    if feat_patterns.iter().any(|p| t.contains(p)) {
        return true;
    }
    // Explicit single/remix labels
    let single_patterns = [" - single", "[single]", "(single)", " remix)", " edit)", "- ep]", "[ep]"];
    if single_patterns.iter().any(|p| t.contains(p)) {
        return true;
    }
    false
}

/// Strip feat/with suffixes from artist/album names for cleaner TPB searches.
fn strip_feat(s: &str) -> String {
    let patterns = [" (feat.", " (with ", " (ft.", " feat.", " ft."];
    let mut result = s.to_string();
    for p in &patterns {
        if let Some(pos) = result.to_ascii_lowercase().find(p) {
            // Close the paren if we stripped an open one
            result = result[..pos].trim_end().to_string();
            break;
        }
    }
    // Also strip trailing edition/remaster suffixes for better search hits
    let trailing = [
        " - deluxe", " - remastered", " - limited", " - expanded",
        " (deluxe edition)", " (deluxe)", " (remastered)", " [deluxe]",
        " (20th anniversary", " limited tour edition", " tour edition",
        " special edition", " anniversary edition", " collector's edition",
        " (anniversary", " - super deluxe", " super deluxe",
        // Year-remastered pattern like "- 2014 Remastered" is handled below
    ];
    let lower = result.to_ascii_lowercase();
    for t in &trailing {
        if let Some(pos) = lower.find(t) {
            result = result[..pos].trim_end().to_string();
            break;
        }
    }
    // Strip " - YYYY Remastered" / " - YYYY Edition" patterns
    let lower2 = result.to_ascii_lowercase();
    if let Some(pos) = lower2.find(" - ") {
        let suffix = &lower2[pos + 3..];
        // Check if suffix starts with a 4-digit year followed by space + word
        let is_year_suffix = suffix.len() >= 5
            && suffix.chars().take(4).all(|c| c.is_ascii_digit())
            && suffix.chars().nth(4) == Some(' ');
        if is_year_suffix {
            result = result[..pos].trim_end().to_string();
        }
    }
    result
}

fn sanitize_component(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c => c,
        })
        .collect::<String>()
        .trim()
        .to_string()
}

fn magnet_hash(magnet: &str) -> Option<String> {
    magnet
        .split("xt=urn:btih:")
        .nth(1)
        .and_then(|s| s.split('&').next())
        .map(|h| h.to_ascii_uppercase())
}

// ── Real-Debrid client ───────────────────────────────────────────────────────

struct RdClient {
    client: reqwest::Client,
}

impl RdClient {
    fn new(api_key: &str) -> Self {
        let mut headers = HeaderMap::new();
        if let Ok(v) = HeaderValue::from_str(&format!("Bearer {api_key}")) {
            headers.insert(AUTHORIZATION, v);
        }
        let client = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_default();
        Self { client }
    }

    async fn find_existing(&self, hash: &str) -> Option<String> {
        let hash_upper = hash.to_ascii_uppercase();
        let url = "https://api.real-debrid.com/rest/1.0/torrents?limit=100&page=1";
        let items: Vec<Value> = self.client.get(url).send().await.ok()?.json().await.ok()?;
        items.into_iter().find_map(|item| {
            let h = item.get("hash")?.as_str()?.to_ascii_uppercase();
            if h == hash_upper {
                item.get("id")?.as_str().map(|s| s.to_string())
            } else {
                None
            }
        })
    }

    async fn add_magnet(&self, magnet: &str) -> Result<String, String> {
        // Dedup: reuse existing torrent if already added
        if let Some(hash) = magnet_hash(magnet) {
            if let Some(existing_id) = self.find_existing(&hash).await {
                info!(torrent_id = %existing_id, "RD torrent already exists — reusing");
                return Ok(existing_id);
            }
        }
        let resp: Value = self.client
            .post("https://api.real-debrid.com/rest/1.0/torrents/addMagnet")
            .form(&[("magnet", magnet)])
            .send().await.map_err(|e| e.to_string())?
            .json().await.map_err(|e| e.to_string())?;
        resp.get("id").and_then(Value::as_str)
            .map(|s| s.to_string())
            .ok_or_else(|| format!("addMagnet: no id in response: {resp}"))
    }

    async fn select_all_files(&self, torrent_id: &str) -> Result<(), String> {
        self.client
            .post(format!("https://api.real-debrid.com/rest/1.0/torrents/selectFiles/{torrent_id}"))
            .form(&[("files", "all")])
            .send().await.map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Poll until status=="downloaded", return links. Timeout: 20 min.
    async fn wait_for_links(&self, torrent_id: &str) -> Result<Vec<String>, String> {
        let url = format!("https://api.real-debrid.com/rest/1.0/torrents/info/{torrent_id}");
        for attempt in 0..240 {
            sleep(Duration::from_secs(5)).await;
            // Retry up to 5 times on connection errors before giving up
            let info: Value = {
                let mut last_err = String::new();
                let mut got = None;
                for _retry in 0..5 {
                    match self.client.get(&url).send().await {
                        Ok(resp) => match resp.json().await {
                            Ok(v) => { got = Some(v); break; }
                            Err(e) => { last_err = e.to_string(); sleep(Duration::from_secs(3)).await; }
                        }
                        Err(e) => { last_err = e.to_string(); sleep(Duration::from_secs(3)).await; }
                    }
                }
                match got {
                    Some(v) => v,
                    None => return Err(format!("RD poll failed after retries: {last_err}")),
                }
            };

            let status = info.get("status").and_then(Value::as_str).unwrap_or("");
            match status {
                "downloaded" => {
                    let links = info.get("links").and_then(Value::as_array)
                        .map(|arr| arr.iter().filter_map(|l| l.as_str().map(|s| s.to_string())).collect())
                        .unwrap_or_default();
                    return Ok(links);
                }
                "error" | "dead" | "virus" => {
                    return Err(format!("Torrent failed: status={status}"));
                }
                _ => {
                    let progress = info.get("progress").and_then(Value::as_f64).unwrap_or(0.0);
                    if attempt % 12 == 0 {
                        info!(torrent_id, status, progress, "RD polling...");
                    }
                }
            }
        }
        Err("Torrent did not resolve within 20 minutes".to_string())
    }

    async fn unrestrict(&self, link: &str) -> Result<String, String> {
        let resp: Value = self.client
            .post("https://api.real-debrid.com/rest/1.0/unrestrict/link")
            .form(&[("link", link)])
            .send().await.map_err(|e| e.to_string())?
            .json().await.map_err(|e| e.to_string())?;
        resp.get("download").and_then(Value::as_str)
            .map(|s| s.to_string())
            .ok_or_else(|| format!("unrestrict: no download url: {resp}"))
    }
}

// ── TPB search ───────────────────────────────────────────────────────────────

#[derive(Debug)]
struct TorrentResult {
    title: String,
    magnet: String,
    seeders: u32,
    size: u64,
}

async fn search_tpb(artist: &str, album: &str) -> Vec<TorrentResult> {
    let query = format!("{artist} {album} FLAC");
    let encoded = urlencoding::encode(&query);
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .unwrap_or_default();

    for cat in ["104", "101"] {
        let url = format!("https://apibay.org/q.php?q={encoded}&cat={cat}");
        let Ok(resp) = client.get(&url).header("User-Agent", "Mozilla/5.0").send().await else { continue };
        if !resp.status().is_success() { continue }
        let items: Vec<Value> = resp.json().await.unwrap_or_default();
        if items.len() == 1 && items[0].get("name").and_then(Value::as_str) == Some("No results returned") {
            continue;
        }
        let mut results: Vec<TorrentResult> = items.into_iter().filter_map(|item| {
            let title = item.get("name")?.as_str()?.to_string();
            let hash = item.get("info_hash")?.as_str()?;
            let seeders = item.get("seeders").and_then(Value::as_str).and_then(|s| s.parse().ok()).unwrap_or(0u32);
            let size = item.get("size").and_then(Value::as_str).and_then(|s| s.parse().ok()).unwrap_or(0u64);
            if seeders < 2 { return None; }
            let enc = urlencoding::encode(&title);
            let magnet = format!(
                "magnet:?xt=urn:btih:{hash}&dn={enc}\
                 &tr=udp%3A%2F%2Ftracker.opentrackr.org%3A1337%2Fannounce\
                 &tr=udp%3A%2F%2Fopen.tracker.cl%3A1337%2Fannounce"
            );
            Some(TorrentResult { title, magnet, seeders, size })
        }).collect();

        if results.is_empty() { continue; }

        // Score: artist + album match, FLAC bonus, seeder bonus
        let artist_n = normalize_text(artist);
        let album_n = normalize_text(album);
        // For short album names (≤3 words), require word-boundary match not just substring
        let album_words: Vec<&str> = album_n.split_whitespace().collect();
        let mut scored: Vec<(i64, TorrentResult)> = results.drain(..).filter_map(|r| {
            let t = normalize_text(&r.title);
            let mut score = 0i64;
            let has_artist = t.contains(&artist_n);
            // Album match: all album words must appear in torrent title
            let has_album = album_words.iter().all(|w| t.split_whitespace().any(|tw| tw == *w));
            // Must match the album title to be considered
            if !has_album { return None; }
            if has_artist { score += 40; }
            score += 60; // album matched
            if t.contains("flac") { score += 50; }
            if t.contains("24bit") || t.contains("24-bit") || t.contains("24 bit") { score += 20; }
            score += (r.seeders.min(50) as i64) * 2;
            // Penalise obvious wrong albums
            if r.size > 10 * 1024 * 1024 * 1024 { score -= 100; } // >10GB
            Some((score, r))
        }).collect();
        if scored.is_empty() { continue; }
        scored.sort_by(|a, b| b.0.cmp(&a.0));
        return scored.into_iter().map(|(_, r)| r).collect();
    }
    Vec::new()
}

// ── file download ────────────────────────────────────────────────────────────

async fn download_file(url: &str, dest: &std::path::Path) -> Result<(), String> {
    use tokio::io::AsyncWriteExt;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(600))
        .build()
        .unwrap_or_default();
    let mut resp = client.get(url).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {} downloading {url}", resp.status()));
    }
    let mut file = tokio::fs::File::create(dest).await.map_err(|e| e.to_string())?;
    while let Some(chunk) = resp.chunk().await.map_err(|e| e.to_string())? {
        file.write_all(&chunk).await.map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ── track matching ───────────────────────────────────────────────────────────

#[derive(Debug)]
struct ExpectedTrack {
    disc: u32,
    number: u32,
    title: String,
}

/// Match downloaded audio files to expected tracks.
/// Returns vec of (expected_index, file_path) pairs.
fn match_files_to_tracks(
    files: &[PathBuf],
    tracks: &[ExpectedTrack],
) -> Vec<(usize, PathBuf)> {
    let mut matched: Vec<(usize, PathBuf)> = Vec::new();
    let mut used_files = vec![false; files.len()];

    for (ti, track) in tracks.iter().enumerate() {
        let mut best_score = 0.0f64;
        let mut best_fi: Option<usize> = None;

        for (fi, file) in files.iter().enumerate() {
            if used_files[fi] { continue; }
            let stem = file.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            let normalized_stem = normalize_text(stem);

            // Track number match: look for the number in the filename
            let num_str = format!("{:02}", track.number);
            let has_number = normalized_stem.contains(&num_str)
                || normalized_stem.starts_with(&track.number.to_string());

            let title_sim = title_similarity(&track.title, stem);
            let mut score = title_sim * 0.6;
            if has_number { score += 0.4; }

            if score > best_score {
                best_score = score;
                best_fi = Some(fi);
            }
        }

        // Require at least a reasonable match
        if best_score >= 0.3 {
            if let Some(fi) = best_fi {
                used_files[fi] = true;
                matched.push((ti, files[fi].clone()));
            }
        }
    }
    matched
}

// ── album download pipeline ──────────────────────────────────────────────────

struct AlbumJob {
    artist: String,
    album: String,
}

async fn process_album(
    job: &AlbumJob,
    rd: &RdClient,
    staging_dir: &std::path::Path,
    library_base: &str,
    db: &Db,
    dry_run: bool,
) -> Result<usize, String> {
    let artist = &job.artist;
    let album = &job.album;

    println!("  [{artist} - {album}]");

    // 1. TPB search
    let results = search_tpb(artist, album).await;
    if results.is_empty() {
        return Err(format!("No torrents found on TPB for {artist} - {album}"));
    }
    let best = &results[0];
    println!("    torrent: {} ({} seeders)", best.title, best.seeders);

    if dry_run {
        println!("    [dry-run] would add: {}", best.magnet);
        return Ok(0);
    }

    // 2. Add to RD
    let torrent_id = rd.add_magnet(&best.magnet).await
        .map_err(|e| format!("addMagnet failed: {e}"))?;
    rd.select_all_files(&torrent_id).await
        .map_err(|e| format!("selectFiles failed: {e}"))?;

    // 3. Wait for download
    println!("    waiting for RD to download...");
    let links = rd.wait_for_links(&torrent_id).await
        .map_err(|e| format!("RD wait failed: {e}"))?;
    println!("    RD done, {} links", links.len());

    // 4. Unrestrict + download audio files to staging
    let album_dir = staging_dir.join(format!(
        "{} - {}",
        normalize_text(artist).replace(' ', "_"),
        normalize_text(album).replace(' ', "_")
    ));
    tokio::fs::create_dir_all(&album_dir).await
        .map_err(|e| format!("mkdir staging: {e}"))?;

    let mut downloaded_files: Vec<PathBuf> = Vec::new();
    for link in &links {
        let direct = match rd.unrestrict(link).await {
            Ok(u) => u,
            Err(e) => { warn!("unrestrict failed for {link}: {e}"); continue; }
        };

        // Derive filename from URL (strip query params)
        let raw_filename = direct.split('/').last()
            .and_then(|f| f.split('?').next())
            .unwrap_or("track")
            .to_string();
        // URL-decode and strip non-ASCII/emoji chars (e.g. ⭐️ in filenames breaks Windows paths)
        let decoded = urlencoding::decode(&raw_filename)
            .map(|s| s.into_owned())
            .unwrap_or(raw_filename);
        let filename: String = decoded.chars()
            .filter(|c| c.is_ascii() || c.is_alphanumeric())
            .collect::<String>()
            .trim()
            .to_string();
        let dest = album_dir.join(&filename);

        if is_audio(&dest) {
            if dest.exists() {
                downloaded_files.push(dest);
                continue;
            }
            print!("    downloading {}... ", filename);
            let _ = std::io::Write::flush(&mut std::io::stdout());
            match download_file(&direct, &dest).await {
                Ok(()) => {
                    println!("ok");
                    downloaded_files.push(dest);
                }
                Err(e) => {
                    println!("FAILED: {e}");
                    warn!("download failed: {e}");
                }
            }
        } else if is_archive(&dest) {
            // Download archive then extract
            if !dest.exists() {
                print!("    downloading archive {}... ", filename);
                let _ = std::io::Write::flush(&mut std::io::stdout());
                match download_file(&direct, &dest).await {
                    Ok(()) => println!("ok"),
                    Err(e) => {
                        println!("FAILED: {e}");
                        warn!("archive download failed: {e}");
                        continue;
                    }
                }
            }
            println!("    extracting {}...", filename);
            let extracted = extract_archive(&dest, &album_dir);
            println!("    extracted {} audio files", extracted.len());
            downloaded_files.extend(extracted);
        }
        // else: skip (cover art, nfo, cue, etc.)
    }

    if downloaded_files.is_empty() {
        return Err("No audio files downloaded".to_string());
    }
    println!("    {} audio files downloaded", downloaded_files.len());

    // 5. Build expected tracklist from DB (tracks already in library) or just use files as-is
    // Since this is for missing albums, we don't have DB tracks — use filenames directly.
    // Sort files by filename (track order).
    downloaded_files.sort_by(|a, b| {
        a.file_name().cmp(&b.file_name())
    });

    // 6. Copy into library
    let dest_album_dir = PathBuf::from(library_base)
        .join(sanitize_component(artist))
        .join(sanitize_component(album));

    tokio::fs::create_dir_all(&dest_album_dir).await
        .map_err(|e| format!("mkdir library: {e}"))?;

    let mut installed = 0usize;
    for src in &downloaded_files {
        let filename = src.file_name().unwrap_or_default();
        let dest = dest_album_dir.join(filename);
        if dest.exists() {
            println!("    skip (exists): {}", dest.display());
            continue;
        }
        tokio::fs::copy(src, &dest).await
            .map_err(|e| format!("copy failed: {e}"))?;
        println!("    installed: {}", dest.display());

        // Upsert into DB
        match read_track_metadata(&dest) {
            Ok(track) => { let _ = db.upsert_track(&track); }
            Err(e) => warn!("metadata read failed for {}: {e}", dest.display()),
        }
        installed += 1;
    }

    Ok(installed)
}

// ── main ─────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .init();

    let args: Vec<String> = std::env::args().collect();
    let dry_run = args.iter().any(|a| a == "--dry-run");
    let limit: usize = args.windows(2)
        .find(|w| w[0] == "--limit")
        .and_then(|w| w[1].parse().ok())
        .unwrap_or(50);
    let min_plays: i64 = args.windows(2)
        .find(|w| w[0] == "--min-plays")
        .and_then(|w| w[1].parse().ok())
        .unwrap_or(1);

    // --album "Artist" "Album" for a single targeted run
    let single_album: Option<(String, String)> = args.windows(3)
        .find(|w| w[0] == "--album")
        .map(|w| (w[1].clone(), w[2].clone()));

    let db_path = app_db_path();
    let db = Db::open(&db_path).map_err(|e| format!("DB open: {e}"))?;

    let rd_key = read_setting(&db, "real_debrid_key")
        .ok_or("REAL_DEBRID_KEY not set in DB or environment")?;
    let library_base = read_setting(&db, "library_base").unwrap_or_else(|| "A:\\Music".to_string());
    let staging_folder = read_setting(&db, "staging_folder").unwrap_or_else(|| "A:\\Staging".to_string());
    let staging_dir = PathBuf::from(&staging_folder).join(".torrent-album-staging");

    let rd = RdClient::new(&rd_key);

    let jobs: Vec<AlbumJob> = if let Some((artist, album)) = single_album {
        vec![AlbumJob { artist, album }]
    } else {
        // Pull from Spotify missing albums backlog
        let missing = db.get_missing_spotify_albums_with_min_plays(min_plays)?;
        let completed_keys = db.get_completed_task_keys()?;
        missing.into_iter()
            .filter(|a| !a.artist.trim().is_empty() && !a.album.trim().is_empty())
            // Skip singles masquerading as albums: feat/with/ft in title, or very short titles
            .filter(|a| !is_single_not_album(&a.album))
            // Skip albums where we already have all tracks
            .filter(|a| {
                let artist = a.artist.trim().to_ascii_lowercase();
                let album = a.album.trim().to_ascii_lowercase();
                let prefix = format!("spotify-album-track::{}::{}", artist, album);
                !completed_keys.iter().any(|k| k.starts_with(&prefix))
            })
            .take(limit)
            .map(|a| AlbumJob {
                artist: strip_feat(&a.artist.trim().to_string()),
                album: strip_feat(&a.album.trim().to_string()),
            })
            .collect()
    };

    if jobs.is_empty() {
        println!("No albums to process.");
        return Ok(());
    }

    println!(
        "torrent_album_cli: {} albums to process{}",
        jobs.len(),
        if dry_run { " [DRY RUN]" } else { "" }
    );

    let mut total_installed = 0usize;
    let mut errors: Vec<String> = Vec::new();

    for (i, job) in jobs.iter().enumerate() {
        println!("\n[{}/{}]", i + 1, jobs.len());
        match process_album(&job, &rd, &staging_dir, &library_base, &db, dry_run).await {
            Ok(n) => {
                total_installed += n;
                println!("    -> {n} tracks installed");
            }
            Err(e) => {
                println!("    -> FAILED: {e}");
                errors.push(format!("{} - {}: {e}", job.artist, job.album));
            }
        }
        // Small delay between albums to avoid hammering TPB/RD
        if i + 1 < jobs.len() {
            sleep(Duration::from_millis(500)).await;
        }
    }

    println!("\n=== torrent_album_cli complete ===");
    println!("Installed: {total_installed} tracks");
    println!("Errors:    {}", errors.len());
    for e in &errors {
        println!("  - {e}");
    }

    Ok(())
}
