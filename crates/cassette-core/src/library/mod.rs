pub mod organizer;
pub mod config;
pub mod error;
pub mod integration;
pub mod locking;
pub mod manager;
pub mod observability;
pub mod operations;
pub mod recovery;
pub mod schema;
pub mod state;
pub mod transactions;
pub mod types;

pub use config::ManagerConfig;
pub use error::{ManagerError, Result as ManagerResult};
pub use manager::LibraryManager;
pub use state::{LibraryState, Module, OperationStatus};
pub use types::{
    DeadlockReport, FileLineage, InvariantViolation, OperationDetails, OperationEvent,
    OperationRecord, RollbackReport, SchemaVersion,
};

#[cfg(test)]
mod tests;

use crate::db::Db;
use crate::models::{ScanProgress, Track};
use crate::Result;
use lofty::prelude::*;
use lofty::probe::Probe;
use std::path::Path;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use walkdir::WalkDir;

use crate::sources::AUDIO_EXTENSIONS;

const DB_BATCH_SIZE: usize = 128;

const COVER_NAMES: &[&str] = &[
    "cover.jpg", "cover.png", "folder.jpg", "folder.png",
    "artwork.jpg", "front.jpg", "album.jpg",
];

pub struct Scanner {
    db: Arc<Mutex<Db>>,
}

impl Scanner {
    pub fn new(db: Arc<Mutex<Db>>) -> Self {
        Self { db }
    }

    pub async fn scan_roots(
        &self,
        roots: Vec<String>,
        progress_tx: mpsc::Sender<ScanProgress>,
    ) -> Result<u64> {
        let existing_sizes = self
            .db
            .lock()
            .map_err(|_| anyhow::anyhow!("database mutex poisoned"))?
            .get_track_size_index()?;

        // Collect all audio paths first
        let mut paths: Vec<std::path::PathBuf> = Vec::new();
        let mut skipped_unchanged = 0u64;
        for root in &roots {
            for entry in WalkDir::new(root).follow_links(true).into_iter().filter_map(|e| e.ok()) {
                if entry.file_type().is_file() {
                    if let Some(ext) = entry.path().extension() {
                        let ext = ext.to_string_lossy().to_lowercase();
                        if AUDIO_EXTENSIONS.contains(&ext.as_str()) {
                            let path = entry.into_path();
                            let path_key = path.to_string_lossy().to_string();
                            let file_size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);

                            if existing_sizes.get(&path_key).copied() == Some(file_size) {
                                skipped_unchanged += 1;
                            } else {
                                paths.push(path);
                            }
                        }
                    }
                }
            }
        }

        let total = paths.len() as u64;
        if total == 0 {
            let _ = progress_tx.send(ScanProgress {
                scanned: 0,
                total: 0,
                current_file: String::new(),
                done: true,
            }).await;
            return Ok(0);
        }

        let env_workers = std::env::var("CASSETTE_SCAN_WORKERS")
            .ok()
            .and_then(|value| value.parse::<usize>().ok());
        let default_workers = std::thread::available_parallelism()
            .map(|n| n.get().saturating_mul(2))
            .unwrap_or(4);
        let worker_count = env_workers.unwrap_or(default_workers).clamp(1, 32);

        let queue = Arc::new(Mutex::new(VecDeque::from(paths)));
        let scanned = Arc::new(AtomicU64::new(0));
        let mut workers = Vec::with_capacity(worker_count);
        let (track_tx, mut track_rx) = tokio::sync::mpsc::channel::<Track>(1024);

        let db_for_writer = Arc::clone(&self.db);
        let writer = tokio::spawn(async move {
            let mut batch = Vec::with_capacity(DB_BATCH_SIZE);

            while let Some(track) = track_rx.recv().await {
                batch.push(track);
                if batch.len() >= DB_BATCH_SIZE {
                    if let Ok(db) = db_for_writer.lock() {
                        let _ = db.upsert_tracks_batch(&batch);
                    }
                    batch.clear();
                }
            }

            if !batch.is_empty() {
                if let Ok(db) = db_for_writer.lock() {
                    let _ = db.upsert_tracks_batch(&batch);
                }
            }
        });

        for _ in 0..worker_count {
            let queue = Arc::clone(&queue);
            let scanned = Arc::clone(&scanned);
            let tx = progress_tx.clone();
            let track_tx = track_tx.clone();

            workers.push(tokio::spawn(async move {
                loop {
                    let next_path = {
                        match queue.lock() {
                            Ok(mut guard) => guard.pop_front(),
                            Err(_) => None,
                        }
                    };

                    let Some(path) = next_path else {
                        break;
                    };

                    let parse_result = tokio::task::spawn_blocking({
                        let path = path.clone();
                        move || read_track_metadata(&path)
                    })
                    .await;

                    if let Ok(Ok(track)) = parse_result {
                        let _ = track_tx.send(track).await;
                    }

                    let current = scanned.fetch_add(1, Ordering::Relaxed) + 1;
                    let _ = tx
                        .send(ScanProgress {
                            scanned: current,
                            total,
                            current_file: path
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .into(),
                            done: false,
                        })
                        .await;
                }
            }));
        }

        drop(track_tx);

        for handle in workers {
            let _ = handle.await;
        }

        let _ = writer.await;

        let scanned = scanned.load(Ordering::Relaxed);

        let _ = progress_tx.send(ScanProgress {
            scanned: total,
            total,
            current_file: format!("skipped unchanged: {skipped_unchanged}"),
            done: true,
        }).await;

        Ok(scanned)
    }
}

pub fn read_track_metadata(path: &Path) -> Result<Track> {
    let tagged = Probe::open(path)
        .map_err(|e| anyhow::anyhow!("{e}"))?
        .read()
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    let tag = tagged.primary_tag().or_else(|| tagged.first_tag());

    let title = tag.and_then(|t| t.title().map(|s| s.to_string()))
        .unwrap_or_else(|| {
            path.file_stem().unwrap_or_default().to_string_lossy().to_string()
        });

    let artist = tag.and_then(|t| t.artist().map(|s| s.to_string()))
        .unwrap_or_default();

    let album = tag.and_then(|t| t.album().map(|s| s.to_string()))
        .unwrap_or_default();

    let album_artist = tag
        .and_then(|t| {
            t.get_string(&lofty::tag::ItemKey::AlbumArtist)
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| artist.clone());

    let track_number = tag.and_then(|t| t.track()).map(|n| n as i32);
    let disc_number  = tag.and_then(|t| t.disk()).map(|n| n as i32);
    let year         = tag.and_then(|t| t.year()).map(|n| n as i32);

    let props = tagged.properties();
    let duration_secs = props.duration().as_secs_f64();
    let sample_rate   = props.sample_rate();
    let bit_depth     = props.bit_depth().map(|b| b as u32);
    let bitrate_kbps  = props.overall_bitrate().map(|b| b / 1000);

    let format = path.extension()
        .map(|e| e.to_string_lossy().to_uppercase())
        .unwrap_or_default();

    let file_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);

    // Look for cover art in the same directory
    let cover_art_path = find_cover_art(path);

    Ok(Track {
        id: 0,
        path: path.to_string_lossy().to_string(),
        title,
        artist,
        album,
        album_artist,
        track_number,
        disc_number,
        year,
        duration_secs,
        sample_rate,
        bit_depth,
        bitrate_kbps,
        format,
        file_size,
        cover_art_path,
        added_at: String::new(),
    })
}

fn find_cover_art(track_path: &Path) -> Option<String> {
    let dir = track_path.parent()?;
    for name in COVER_NAMES {
        let candidate = dir.join(name);
        if candidate.exists() {
            return Some(candidate.to_string_lossy().to_string());
        }
    }
    None
}
