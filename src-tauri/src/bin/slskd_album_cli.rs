/// slskd_album_cli — album-first Soulseek downloader via slskd REST API
///
/// Flow per album:
///   1. Search slskd for "artist album FLAC"
///   2. Pick best result directory (most FLAC files, good name match)
///   3. Queue entire folder for download
///   4. Wait for all files to complete
///   5. Copy audio files to library_base/Artist/Album/
///   6. Upsert tracks to DB, mark in_library
///
/// Usage:
///   slskd_album_cli [--dry-run] [--limit N] [--min-plays N]
///   slskd_album_cli --album "Artist" "Album Title"
use cassette_core::db::Db;
use cassette_core::library::read_track_metadata;
use cassette_core::sources::normalize_text;
use serde_json::Value;
use std::path::PathBuf;
use tokio::time::{sleep, Duration};
use tracing::warn;

// ── helpers ──────────────────────────────────────────────────────────────────

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

fn is_audio_ext(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    matches!(
        lower.rsplit('.').next().unwrap_or(""),
        "flac" | "mp3" | "m4a" | "aac" | "ogg" | "opus" | "wav" | "aiff" | "alac"
    )
}

fn is_flac_ext(name: &str) -> bool {
    name.to_ascii_lowercase().ends_with(".flac")
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

fn strip_feat(s: &str) -> String {
    let patterns = [" (feat.", " (with ", " (ft.", " feat.", " ft."];
    let mut result = s.to_string();
    for p in &patterns {
        if let Some(pos) = result.to_ascii_lowercase().find(p) {
            result = result[..pos].trim_end().to_string();
            break;
        }
    }
    let trailing = [
        " - deluxe", " - remastered", " - limited", " - expanded",
        " (deluxe edition)", " (deluxe)", " (remastered)", " [deluxe]",
        " (20th anniversary", " limited tour edition", " tour edition",
        " special edition", " anniversary edition", " - super deluxe", " super deluxe",
    ];
    let lower = result.to_ascii_lowercase();
    for t in &trailing {
        if let Some(pos) = lower.find(t) {
            result = result[..pos].trim_end().to_string();
            break;
        }
    }
    // Strip " - YYYY ..." year suffix
    let lower2 = result.to_ascii_lowercase();
    if let Some(pos) = lower2.find(" - ") {
        let suffix = &lower2[pos + 3..];
        let is_year = suffix.len() >= 5
            && suffix.chars().take(4).all(|c| c.is_ascii_digit())
            && suffix.chars().nth(4) == Some(' ');
        if is_year {
            result = result[..pos].trim_end().to_string();
        }
    }
    result
}

fn is_single_not_album(album: &str) -> bool {
    let t = album.to_ascii_lowercase();
    let feat_patterns = ["(feat.", "(with ", "(ft.", "ft. ", "feat. "];
    if feat_patterns.iter().any(|p| t.contains(p)) { return true; }
    let single_patterns = [" - single", "[single]", "(single)", " remix)", " edit)", "- ep]", "[ep]"];
    single_patterns.iter().any(|p| t.contains(p))
}

fn is_live_or_bootleg(album: &str) -> bool {
    let t = album.to_ascii_lowercase();
    let chars: Vec<char> = t.chars().collect();
    for i in 0..chars.len().saturating_sub(7) {
        if chars[i..i+4].iter().all(|c| c.is_ascii_digit()) {
            let sep = chars[i+4];
            if (sep == '-' || sep == '\u{2010}' || sep == '/')
                && i + 7 < chars.len()
                && chars[i+5..i+7].iter().all(|c| c.is_ascii_digit())
            {
                return true;
            }
        }
    }
    let live_patterns = [
        "live at ", "live from ", "live in ", "live on ",
        ": live", "(live)", "[live]", " - live",
        "concert at ", "concert in ", "bootleg", "unofficial",
        "black session", "session #", "fm broadcast", "radio session",
        "bbc session", "kcrw", "kexp", "morning becomes eclectic",
        ": venue", "unknown venue", "pre‐fm", "pre-fm",
    ];
    live_patterns.iter().any(|p| t.contains(p))
}

// ── slskd client ──────────────────────────────────────────────────────────────

struct SlskdClient {
    client: reqwest::Client,
    base: String,
    token: String,
}

impl SlskdClient {
    async fn new(base: &str, user: &str, pass: &str) -> Result<Self, String> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| e.to_string())?;

        let resp: Value = client
            .post(format!("{base}/api/v0/session"))
            .json(&serde_json::json!({"username": user, "password": pass}))
            .send().await.map_err(|e| format!("slskd login: {e}"))?
            .json().await.map_err(|e| format!("slskd login parse: {e}"))?;

        let token = resp.get("token")
            .and_then(Value::as_str)
            .ok_or("slskd: no token in login response")?
            .to_string();

        Ok(Self { client, base: base.to_string(), token })
    }

    fn auth(&self) -> String {
        format!("Bearer {}", self.token)
    }

    /// Search slskd, wait for results, return raw response.
    async fn search(&self, query: &str) -> Result<Value, String> {
        // Start search
        let resp: Value = self.client
            .post(format!("{}/api/v0/searches", self.base))
            .header("Authorization", self.auth())
            .json(&serde_json::json!({
                "searchText": query,
                "fileLimit": 10000,
                "resultLimit": 100,
            }))
            .send().await.map_err(|e| e.to_string())?
            .json().await.map_err(|e| e.to_string())?;

        let id = resp.get("id").and_then(Value::as_str)
            .ok_or("no search id")?
            .to_string();

        // Poll until complete (max 60s)
        for _ in 0..60 {
            sleep(Duration::from_secs(1)).await;
            let status: Value = self.client
                .get(format!("{}/api/v0/searches/{id}", self.base))
                .header("Authorization", self.auth())
                .send().await.map_err(|e| e.to_string())?
                .json().await.map_err(|e| e.to_string())?;

            let state = status.get("state").and_then(Value::as_str).unwrap_or("");
            let is_complete = status.get("isComplete").and_then(Value::as_bool).unwrap_or(false);
            if is_complete || state.contains("Completed") || state == "Stopped" {
                // Fetch full results
                let results: Value = self.client
                    .get(format!("{}/api/v0/searches/{id}/responses", self.base))
                    .header("Authorization", self.auth())
                    .send().await.map_err(|e| e.to_string())?
                    .json().await.map_err(|e| e.to_string())?;
                // Clean up search
                let _ = self.client
                    .delete(format!("{}/api/v0/searches/{id}", self.base))
                    .header("Authorization", self.auth())
                    .send().await;
                return Ok(results);
            }
        }
        Err("search timed out after 60s".to_string())
    }

    /// Queue a folder download from a specific user.
    async fn download_folder(&self, username: &str, folder: &str, files: &[String]) -> Result<(), String> {
        for file in files {
            // slskd expects an array of QueueDownloadRequest
            let body = serde_json::json!([{
                "filename": file,
                "size": 0,
            }]);
            let resp = self.client
                .post(format!("{}/api/v0/transfers/downloads/{username}", self.base))
                .header("Authorization", self.auth())
                .json(&body)
                .send().await.map_err(|e| e.to_string())?;
            if !resp.status().is_success() {
                let status = resp.status();
                let text = resp.text().await.unwrap_or_default();
                warn!("enqueue failed for {file}: {status} {text}");
            }
        }
        let _ = folder; // folder path is embedded in file paths
        Ok(())
    }

    /// Cancel all queued (not yet transferring) downloads for a username.
    async fn cancel_queued_downloads(&self, username: &str) -> Result<(), String> {
        let transfers: Value = self.client
            .get(format!("{}/api/v0/transfers/downloads/{username}", self.base))
            .header("Authorization", self.auth())
            .send().await.map_err(|e| e.to_string())?
            .json().await.map_err(|e| e.to_string())?;

        let dirs = transfers.get("directories").and_then(Value::as_array).cloned().unwrap_or_default();
        for dir in &dirs {
            for f in dir.get("files").and_then(Value::as_array).cloned().unwrap_or_default() {
                let state = f.get("state").and_then(Value::as_str).unwrap_or("");
                if state.contains("Queued") {
                    if let Some(id) = f.get("id").and_then(Value::as_str) {
                        let _ = self.client
                            .delete(format!("{}/api/v0/transfers/downloads/{username}/{id}", self.base))
                            .header("Authorization", self.auth())
                            .send().await;
                    }
                }
            }
        }
        Ok(())
    }

    /// Poll downloads for a username until all given files complete or fail.
    /// Returns list of completed remote filenames (to be resolved to local paths by caller).
    /// Abandons early if nothing starts transferring within 60s (peer is slow/offline).
    async fn wait_for_downloads(&self, username: &str, filenames: &[String]) -> Result<Vec<String>, String> {
        let timeout = 48; // 4 min max (48 * 5s)
        let stall_limit = 12; // abandon after 60s with no bytes transferred
        let mut any_started = false;

        for attempt in 0..timeout {
            sleep(Duration::from_secs(5)).await;

            let transfers: Value = self.client
                .get(format!("{}/api/v0/transfers/downloads/{username}", self.base))
                .header("Authorization", self.auth())
                .send().await.map_err(|e| e.to_string())?
                .json().await.map_err(|e| e.to_string())?;

            // Response: { username, directories: [{ directory, files: [...] }] }
            let dirs = transfers.get("directories").and_then(Value::as_array).cloned().unwrap_or_default();
            let mut completed = Vec::new();
            let mut pending = 0usize;
            let mut failed = 0usize;
            let mut transferring = 0usize;

            for dir in &dirs {
                let dir_files = dir.get("files").and_then(Value::as_array).cloned().unwrap_or_default();
                for f in &dir_files {
                    let fname = f.get("filename").and_then(Value::as_str).unwrap_or("");
                    // Only track our files
                    if !filenames.iter().any(|n| fname.ends_with(n.as_str()) || n.ends_with(fname)) {
                        continue;
                    }
                    let state = f.get("state").and_then(Value::as_str).unwrap_or("");
                    let pct = f.get("percentComplete").and_then(Value::as_f64).unwrap_or(0.0);
                    match state {
                        "Completed, Succeeded" => {
                            if let Some(remote) = f.get("filename").and_then(Value::as_str) {
                                completed.push(remote.to_string());
                            }
                        }
                        "Completed, Errored" | "Completed, Cancelled" | "Completed, TimedOut" | "Completed, Aborted" => {
                            failed += 1;
                        }
                        s if s.contains("Transferring") || pct > 0.0 => {
                            transferring += 1;
                            pending += 1;
                            any_started = true;
                        }
                        _ => { pending += 1; }
                    }
                }
            }

            if attempt % 4 == 0 {
                println!("    waiting... completed={} transferring={} pending={} failed={}", completed.len(), transferring, pending, failed);
            }

            if pending == 0 && (completed.len() + failed) >= filenames.len() {
                return Ok(completed);
            }
            if pending == 0 && attempt > 2 {
                return Ok(completed);
            }
            // Abandon if nothing has started after stall_limit polls
            if !any_started && attempt >= stall_limit {
                return Err(format!("peer {username} not responding after {}s, skipping", stall_limit * 5));
            }
        }
        Err("download timed out after 4 minutes".to_string())
    }
}

// ── search result scoring ─────────────────────────────────────────────────────

#[derive(Debug)]
struct AlbumCandidate {
    username: String,
    folder: String,       // directory path on remote
    files: Vec<String>,   // full file paths to download
    flac_count: u32,
    score: i64,
}

fn score_candidates(results: &Value, artist: &str, album: &str) -> Vec<AlbumCandidate> {
    let artist_n = normalize_text(artist);
    let album_n = normalize_text(album);
    let album_words: Vec<String> = album_n.split_whitespace().map(|s| s.to_string()).collect();

    let responses = match results.as_array() {
        Some(a) => a,
        None => return Vec::new(),
    };

    let mut candidates: Vec<AlbumCandidate> = Vec::new();

    for response in responses {
        let username = response.get("username").and_then(Value::as_str).unwrap_or("").to_string();
        let upload_speed = response.get("uploadSpeed").and_then(Value::as_u64).unwrap_or(0);
        let free_upload_slots = response.get("freeUploadSlots").and_then(Value::as_u64).unwrap_or(0);

        let files = match response.get("files").and_then(Value::as_array) {
            Some(f) => f,
            None => continue,
        };

        // Group files by directory
        let mut dirs: std::collections::HashMap<String, Vec<(String, bool)>> = std::collections::HashMap::new();
        for file in files {
            let fname = match file.get("filename").and_then(Value::as_str) {
                Some(f) => f,
                None => continue,
            };
            // Directory is everything up to last backslash
            let dir = fname.rsplitn(2, '\\').nth(1).unwrap_or("").to_string();
            let is_flac = is_flac_ext(fname);
            let is_audio = is_audio_ext(fname);
            if is_audio {
                dirs.entry(dir).or_default().push((fname.to_string(), is_flac));
            }
        }

        for (dir, dir_files) in dirs {
            let flac_count = dir_files.iter().filter(|(_, is_flac)| *is_flac).count() as u32;
            if flac_count < 3 { continue; } // need at least 3 FLAC files to be an album

            let dir_n = normalize_text(&dir);
            let has_artist = dir_n.contains(&artist_n);
            let has_album = album_words.iter().all(|w| dir_n.split_whitespace().any(|dw| dw == w.as_str()));

            if !has_album { continue; }

            let mut score = 60i64; // album name matched
            if has_artist { score += 40; }
            if flac_count >= 8 { score += 30; }
            if flac_count >= 12 { score += 20; }
            score += (free_upload_slots.min(5) as i64) * 10;
            score += (upload_speed / 1024).min(100) as i64; // MB/s bonus

            candidates.push(AlbumCandidate {
                username: username.clone(),
                folder: dir,
                files: dir_files.into_iter().map(|(f, _)| f).collect(),
                flac_count,
                score,
            });
        }
    }

    candidates.sort_by(|a, b| b.score.cmp(&a.score));
    candidates
}

// ── album pipeline ────────────────────────────────────────────────────────────

struct AlbumJob {
    artist: String,
    album: String,
}

async fn process_album(
    job: &AlbumJob,
    slskd: &SlskdClient,
    library_base: &str,
    slskd_downloads_dir: &str,
    db: &Db,
    dry_run: bool,
) -> Result<usize, String> {
    let artist = &job.artist;
    let album = &job.album;

    println!("  [{artist} - {album}]");

    // 1. Search
    let query = format!("{artist} {album} FLAC");
    let results = slskd.search(&query).await
        .map_err(|e| format!("search failed: {e}"))?;

    let candidates = score_candidates(&results, artist, album);
    if candidates.is_empty() {
        return Err(format!("No Soulseek results for {artist} - {album}"));
    }

    if dry_run {
        let best = &candidates[0];
        println!("    slskd: {} files from {} (score {})", best.flac_count, best.username, best.score);
        println!("    folder: {}", best.folder);
        println!("    [dry-run] would download {} files from {}", best.files.len(), best.username);
        return Ok(0);
    }

    // Try candidates in order until one succeeds
    let mut last_err = String::from("no candidates");
    for (ci, best) in candidates.iter().enumerate().take(5) {
        println!("    slskd: {} files from {} (score {}) [candidate {}/{}]",
            best.flac_count, best.username, best.score, ci + 1, candidates.len().min(5));
        println!("    folder: {}", best.folder);

        // Cancel any stale queued transfers from previous attempts
        let _ = slskd.cancel_queued_downloads(&best.username).await;

        // Queue download
        slskd.download_folder(&best.username, &best.folder, &best.files).await
            .map_err(|e| format!("enqueue failed: {e}"))?;

        println!("    queued {} files, waiting...", best.files.len());

        match slskd.wait_for_downloads(&best.username, &best.files).await {
            Ok(completed_remote) if !completed_remote.is_empty() => {
                println!("    {} files downloaded from {}", completed_remote.len(), best.username);

                // 4. Copy to library
                // slskd saves to: downloads_dir/{remote_album_dir}/{filename}
                // remote_album_dir is the last path segment of the remote folder
                let remote_dir_name = best.folder.rsplitn(2, '\\').next().unwrap_or(&best.folder);
                let src_dir = PathBuf::from(slskd_downloads_dir).join(remote_dir_name);

                let dest_album_dir = PathBuf::from(library_base)
                    .join(sanitize_component(artist))
                    .join(sanitize_component(album));

                tokio::fs::create_dir_all(&dest_album_dir).await
                    .map_err(|e| format!("mkdir library: {e}"))?;

                let mut installed = 0usize;
                // completed_remote contains full remote paths; extract just basename
                let mut filenames: Vec<String> = completed_remote.iter()
                    .map(|r| r.rsplitn(2, '\\').next().unwrap_or(r).to_string())
                    .collect();
                filenames.sort();

                for fname in &filenames {
                    let src = src_dir.join(fname);
                    if !is_audio_ext(fname) { continue; }
                    let dest = dest_album_dir.join(fname);
                    if dest.exists() {
                        println!("    skip (exists): {fname}");
                        continue;
                    }
                    match tokio::fs::copy(&src, &dest).await {
                        Ok(_) => {
                            println!("    installed: {fname}");
                            match read_track_metadata(&dest) {
                                Ok(track) => { let _ = db.upsert_track(&track); }
                                Err(e) => warn!("metadata read failed for {fname}: {e}"),
                            }
                            installed += 1;
                        }
                        Err(e) => {
                            warn!("copy failed for {}: {e}", src.display());
                        }
                    }
                }

                if installed > 0 {
                    let _ = db.mark_spotify_album_in_library(artist, album);
                }
                return Ok(installed);
            }
            Ok(_) => {
                last_err = format!("peer {} delivered 0 files", best.username);
                println!("    -> peer {} delivered nothing, trying next candidate...", best.username);
            }
            Err(e) => {
                last_err = e.clone();
                println!("    -> peer {} failed: {e}, trying next candidate...", best.username);
            }
        }
    }

    Err(last_err)
}

// ── main ──────────────────────────────────────────────────────────────────────

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
        .unwrap_or(3);
    let single_album: Option<(String, String)> = args.windows(3)
        .find(|w| w[0] == "--album")
        .map(|w| (w[1].clone(), w[2].clone()));

    let db_path = app_db_path();
    let db = Db::open(&db_path).map_err(|e| format!("DB open: {e}"))?;

    let slskd_url = read_setting(&db, "slskd_url").unwrap_or_else(|| "http://localhost:5030".to_string());
    let slskd_user = read_setting(&db, "slskd_user").unwrap_or_else(|| "slskd".to_string());
    let slskd_pass = read_setting(&db, "slskd_pass").unwrap_or_else(|| "slskd".to_string());
    let slskd_downloads_dir = read_setting(&db, "slskd_downloads_dir").unwrap_or_else(|| "A:\\Staging\\slskd".to_string());
    let library_base = read_setting(&db, "library_base").unwrap_or_else(|| "A:\\Music".to_string());

    println!("Connecting to slskd at {slskd_url}...");
    let slskd = SlskdClient::new(&slskd_url, &slskd_user, &slskd_pass).await
        .map_err(|e| format!("slskd connect: {e}"))?;
    println!("Connected.");

    let jobs: Vec<AlbumJob> = if let Some((artist, album)) = single_album {
        vec![AlbumJob { artist, album }]
    } else {
        let missing = db.get_missing_spotify_albums_with_min_plays(min_plays)?;
        let completed_keys = db.get_completed_task_keys()?;
        let lib = PathBuf::from(&library_base);

        missing.into_iter()
            .filter(|a| !a.artist.trim().is_empty() && !a.album.trim().is_empty())
            .filter(|a| !is_single_not_album(&a.album))
            .filter(|a| !is_live_or_bootleg(&a.album))
            .filter(|a| {
                let artist = a.artist.trim().to_ascii_lowercase();
                let album = a.album.trim().to_ascii_lowercase();
                let prefix = format!("spotify-album-track::{}::{}", artist, album);
                !completed_keys.iter().any(|k| k.starts_with(&prefix))
            })
            .filter(|a| {
                let artist_s = strip_feat(&a.artist.trim().to_string());
                let album_s = strip_feat(&a.album.trim().to_string());
                !lib.join(sanitize_component(&artist_s)).join(sanitize_component(&album_s)).exists()
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
        "slskd_album_cli: {} albums to process{}",
        jobs.len(),
        if dry_run { " [DRY RUN]" } else { "" }
    );

    let mut total_installed = 0usize;
    let mut errors: Vec<String> = Vec::new();

    for (i, job) in jobs.iter().enumerate() {
        println!("\n[{}/{}]", i + 1, jobs.len());
        match process_album(&job, &slskd, &library_base, &slskd_downloads_dir, &db, dry_run).await {
            Ok(n) => {
                total_installed += n;
                println!("    -> {n} tracks installed");
            }
            Err(e) => {
                println!("    -> FAILED: {e}");
                errors.push(format!("{} - {}: {e}", job.artist, job.album));
            }
        }
        sleep(Duration::from_millis(500)).await;
    }

    println!("\n=== slskd_album_cli complete ===");
    println!("Installed: {total_installed} tracks");
    println!("Errors:    {}", errors.len());
    for e in &errors {
        println!("  - {e}");
    }

    Ok(())
}
