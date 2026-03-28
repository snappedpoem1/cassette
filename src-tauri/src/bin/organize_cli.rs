use cassette_core::db::Db;
use cassette_core::library::organizer;
use cassette_core::models::Track;
use rayon::prelude::*;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

fn app_db_path() -> Result<PathBuf, String> {
    let app_data = std::env::var("APPDATA").map_err(|e| e.to_string())?;
    Ok(PathBuf::from(app_data)
        .join("dev.cassette.app")
        .join("cassette.db"))
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    let live = args.iter().any(|a| a == "--live");

    let db_path = app_db_path()?;
    println!("DB: {}", db_path.display());

    let db = Db::open(&db_path).map_err(|e| e.to_string())?;

    let library_base = db
        .get_setting("library_base")
        .map_err(|e| e.to_string())?
        .unwrap_or_else(|| "A:\\Music".to_string());
    println!("Library base: {library_base}");

    // Step 0: prune stale DB entries (files that no longer exist on disk)
    let pruned = db.prune_missing_tracks().map_err(|e| e.to_string())?;
    if pruned > 0 {
        println!("Pruned {pruned} stale tracks from DB (files no longer on disk)");
    }

    let track_count = db.get_track_count().map_err(|e| e.to_string())?;
    println!("Tracks in DB: {track_count}");

    // Step 1: incremental scan — parallel metadata reads, batched DB writes
    {
        let existing_tracks = db.get_all_tracks_unfiltered().map_err(|e| e.to_string())?;
        let known_paths: HashSet<String> = existing_tracks.iter().map(|t| t.path.clone()).collect();
        let already_indexed = known_paths.len();

        let roots = db.get_library_roots().map_err(|e| e.to_string())?;
        let scan_roots: Vec<String> = if roots.is_empty() {
            vec![library_base.clone()]
        } else {
            roots.iter().map(|r| r.path.clone()).collect()
        };

        // Collect all new file paths first (fast — just a directory walk)
        let start = Instant::now();
        let mut new_paths: Vec<PathBuf> = Vec::new();
        for root in &scan_roots {
            for entry in walkdir::WalkDir::new(root)
                .follow_links(true)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                if !entry.file_type().is_file() {
                    continue;
                }
                if !cassette_core::sources::is_audio_path(entry.path()) {
                    continue;
                }
                let path_str = entry.path().to_string_lossy().to_string();
                if !known_paths.contains(&path_str) {
                    new_paths.push(entry.into_path());
                }
            }
        }
        let walk_time = start.elapsed();
        println!(
            "Found {} new files to index ({} already known) — walk took {:.1}s",
            new_paths.len(),
            already_indexed,
            walk_time.as_secs_f64()
        );

        if !new_paths.is_empty() {
            // Parallel metadata extraction with rayon
            let counter = AtomicU64::new(0);
            let total = new_paths.len() as u64;
            let start = Instant::now();

            let tracks: Vec<Track> = new_paths
                .par_iter()
                .filter_map(|path| {
                    let result = cassette_core::library::read_track_metadata(path).ok()?;
                    let done = counter.fetch_add(1, Ordering::Relaxed) + 1;
                    if done % 500 == 0 || done == total {
                        let elapsed = start.elapsed().as_secs_f64();
                        let rate = done as f64 / elapsed;
                        let eta = (total - done) as f64 / rate;
                        eprint!("\r  reading tags: {done}/{total} ({rate:.0}/s, ~{eta:.0}s left)   ");
                    }
                    Some(result)
                })
                .collect();

            eprintln!();
            let read_time = start.elapsed();
            println!(
                "  Read {} tracks in {:.1}s ({:.0}/s)",
                tracks.len(),
                read_time.as_secs_f64(),
                tracks.len() as f64 / read_time.as_secs_f64()
            );

            // Batch insert into DB (SQLite is single-writer, so this stays sequential but fast)
            let start = Instant::now();
            let mut inserted = 0u64;
            for track in &tracks {
                if let Err(e) = db.upsert_track(track) {
                    eprintln!("  upsert error: {e}");
                } else {
                    inserted += 1;
                }
            }
            let db_time = start.elapsed();
            println!(
                "  Inserted {inserted} tracks in {:.1}s\n",
                db_time.as_secs_f64()
            );
        } else {
            println!("  Nothing new to index.\n");
        }
    }

    // Step 2: get all tracks and run organize
    let tracks = db.get_all_tracks_unfiltered().map_err(|e| e.to_string())?;
    println!("Organizing {} tracks (dry_run={})...\n", tracks.len(), !live);

    let result = organizer::organize_tracks(&library_base, &tracks, !live);

    // Print moves
    if !result.moved.is_empty() {
        println!("=== MOVES ({}) ===", result.moved.len());
        for (i, mv) in result.moved.iter().enumerate() {
            if i < 50 || live {
                println!("  {} \n    -> {}", mv.old_path, mv.new_path);
            }
        }
        if !live && result.moved.len() > 50 {
            println!("  ... and {} more", result.moved.len() - 50);
        }
    }

    // Print skipped count
    if !result.skipped.is_empty() {
        println!("\n=== ALREADY IN PLACE: {} ===", result.skipped.len());
        for s in result.skipped.iter().take(10) {
            println!("  {s}");
        }
        if result.skipped.len() > 10 {
            println!("  ... and {} more", result.skipped.len() - 10);
        }
    }

    // Print errors
    if !result.errors.is_empty() {
        println!("\n=== ERRORS ({}) ===", result.errors.len());
        for e in &result.errors {
            println!("  {e}");
        }
    }

    println!(
        "\nSummary: {} moves, {} skipped, {} errors",
        result.moved.len(),
        result.skipped.len(),
        result.errors.len()
    );

    if !live && !result.moved.is_empty() {
        println!("\nThis was a DRY RUN. To actually move files, run with --live");
    }

    if live && !result.moved.is_empty() {
        println!("\nMoving files and updating DB paths...");
        for mv in &result.moved {
            if let Err(e) = db.update_track_path(mv.track_id, &mv.new_path) {
                eprintln!("  DB update error for track {}: {e}", mv.track_id);
            }
        }
        println!("Done.");
    }

    Ok(())
}
