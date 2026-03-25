use cassette_core::db::Db;
use cassette_core::library::Scanner;
use cassette_core::models::ScanProgress;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

fn resolve_db_path(arg: Option<String>) -> Result<PathBuf, String> {
    if let Some(path) = arg {
        return Ok(PathBuf::from(path));
    }

    let app_data = std::env::var("APPDATA").map_err(|e| format!("APPDATA is not set: {e}"))?;
    Ok(PathBuf::from(app_data)
        .join("dev.cassette.app")
        .join("cassette.db"))
}

#[tokio::main]
async fn main() -> Result<(), String> {
    let db_path = resolve_db_path(std::env::args().nth(1))?;
    println!("Using DB: {}", db_path.display());

    let db = Db::open(&db_path).map_err(|e| format!("Failed opening DB: {e}"))?;
    let roots = db
        .get_library_roots()
        .map_err(|e| format!("Failed loading library roots: {e}"))?;

    let enabled_roots: Vec<String> = roots
        .iter()
        .filter(|r| r.enabled)
        .map(|r| r.path.clone())
        .collect();

    if enabled_roots.is_empty() {
        return Err("No enabled library roots found in DB.".to_string());
    }

    println!("Enabled roots:");
    for root in &enabled_roots {
        println!("  - {root}");
    }

    let db = Arc::new(Mutex::new(db));
    let scanner = Scanner::new(Arc::clone(&db));
    let (tx, mut rx) = tokio::sync::mpsc::channel::<ScanProgress>(256);

    let progress_task = tokio::spawn(async move {
        let mut last_printed = 0u64;
        let mut sample_started = Instant::now();
        let mut sample_scanned = 0u64;

        while let Some(progress) = rx.recv().await {
            if progress.done {
                println!("Scan complete: {}/{} files", progress.scanned, progress.total);
                if !progress.current_file.is_empty() {
                    println!("{}", progress.current_file);
                }
                break;
            }

            let now = Instant::now();
            if now.duration_since(sample_started) >= Duration::from_secs(5) {
                let elapsed = now.duration_since(sample_started).as_secs_f64();
                let delta = progress.scanned.saturating_sub(sample_scanned);
                let rate = if elapsed > 0.0 {
                    delta as f64 / elapsed
                } else {
                    0.0
                };
                println!("Throughput: {rate:.1} files/sec");
                sample_started = now;
                sample_scanned = progress.scanned;
            }

            // Reduce log noise on very large libraries.
            if progress.scanned == 0
                || progress.scanned >= last_printed.saturating_add(500)
                || progress.scanned == progress.total
            {
                println!(
                    "Scanned {}/{} ({})",
                    progress.scanned, progress.total, progress.current_file
                );
                last_printed = progress.scanned;
            }
        }
    });

    let scanned = scanner
        .scan_roots(enabled_roots, tx)
        .await
        .map_err(|e| format!("Scanner failed: {e}"))?;

    let _ = progress_task.await;

    let track_count = db
        .lock()
        .map_err(|e| format!("DB mutex poisoned: {e}"))?
        .get_track_count()
        .map_err(|e| format!("Failed getting track count: {e}"))?;

    println!("Scanned files: {scanned}");
    println!("Tracks currently in DB: {track_count}");

    Ok(())
}