pub mod config;
pub mod error;
pub mod integration;
pub mod locking;
pub mod manager;
pub mod observability;
pub mod operations;
pub mod organizer;
pub mod recovery;
pub mod schema;
pub mod state;
pub mod track_number_repair;
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
use crate::director::models::{CandidateQuality, DirectorTaskResult, NormalizedTrack};
use crate::identity::{normalize_artist_identity, normalize_identity_text};
use crate::library::track_number_repair::parse_filename_numbers;
use crate::models::{ScanProgress, Track};
use crate::Result;
use lofty::picture::PictureType;
use lofty::prelude::*;
use lofty::probe::Probe;
use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use walkdir::WalkDir;

use crate::sources::AUDIO_EXTENSIONS;

const DB_BATCH_SIZE: usize = 128;

const COVER_NAMES: &[&str] = &[
    "cover.jpg",
    "cover.jpeg",
    "cover.png",
    "cover.webp",
    "folder.jpg",
    "folder.jpeg",
    "folder.png",
    "folder.webp",
    "artwork.jpg",
    "artwork.jpeg",
    "artwork.png",
    "front.jpg",
    "front.jpeg",
    "front.png",
    "album.jpg",
    "album.jpeg",
    "album.png",
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
            for entry in WalkDir::new(root)
                .follow_links(true)
                .into_iter()
                .filter_entry(|e| {
                    // Skip quarantine directories
                    if e.file_type().is_dir() {
                        let name = e.file_name().to_string_lossy();
                        return name != "_Cassette_Quarantine";
                    }
                    true
                })
                .filter_map(|e| e.ok())
            {
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
            let _ = progress_tx
                .send(ScanProgress {
                    scanned: 0,
                    total: 0,
                    current_file: String::new(),
                    done: true,
                })
                .await;
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

        let _ = progress_tx
            .send(ScanProgress {
                scanned: total,
                total,
                current_file: format!("skipped unchanged: {skipped_unchanged}"),
                done: true,
            })
            .await;

        Ok(scanned)
    }
}

pub fn read_track_metadata(path: &Path) -> Result<Track> {
    let tagged = Probe::open(path)
        .map_err(|e| anyhow::anyhow!("{e}"))?
        .read()
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    let tag = tagged.primary_tag().or_else(|| tagged.first_tag());

    let filename_title = cleaned_filename_title(path);
    let title = tag
        .and_then(|t| t.title().map(|s| s.to_string()))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| filename_title.clone());

    let artist = tag
        .and_then(|t| t.artist().map(|s| s.to_string()))
        .map(|value| value.trim().to_string())
        .unwrap_or_default();

    let album = tag
        .and_then(|t| t.album().map(|s| s.to_string()))
        .map(|value| value.trim().to_string())
        .unwrap_or_default();

    let mut album_artist = tag
        .and_then(|t| {
            t.get_string(&lofty::tag::ItemKey::AlbumArtist)
                .map(|s| s.to_string())
        })
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| artist.clone());
    if album_artist.trim().is_empty() {
        album_artist = artist.clone();
    }

    let filename_numbers = parse_filename_numbers(&path.to_string_lossy());
    let track_number = tag
        .and_then(|t| t.track())
        .map(|n| n as i32)
        .filter(|value| *value > 0)
        .or_else(|| filename_numbers.as_ref().map(|value| value.track_number));
    let disc_number = tag
        .and_then(|t| t.disk())
        .map(|n| n as i32)
        .filter(|value| *value > 0)
        .or_else(|| {
            filename_numbers
                .as_ref()
                .and_then(|value| value.disc_number)
        });
    let year = tag
        .and_then(|t| t.year())
        .map(|n| n as i32)
        .filter(|value| *value > 0);
    let isrc = tag
        .and_then(|t| t.get_string(&lofty::tag::ItemKey::Isrc))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    let props = tagged.properties();
    let duration_secs = props.duration().as_secs_f64();
    let sample_rate = props.sample_rate();
    let bit_depth = props.bit_depth().map(|b| b as u32);
    let bitrate_kbps = props.overall_bitrate().map(|b| b / 1000);

    let format = path
        .extension()
        .map(|e| e.to_string_lossy().to_uppercase())
        .unwrap_or_default();

    let file_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);

    // Look for cover art in the same directory
    let cover_art_path = find_cover_art(path).or_else(|| extract_embedded_cover_art(&tagged, path));

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
        isrc,
        musicbrainz_recording_id: None,
        musicbrainz_release_id: None,
        canonical_artist_id: None,
        canonical_release_id: None,
        quality_tier: None,
        content_hash: None,
        added_at: String::new(),
    })
}

pub fn enrich_track_with_normalized_metadata(track: &mut Track, metadata: &NormalizedTrack) {
    if track.artist.trim().is_empty() && !metadata.artist.trim().is_empty() {
        track.artist = metadata.artist.clone();
    }
    if track.album_artist.trim().is_empty()
        || normalize_artist_identity(&track.album_artist)
            != normalize_artist_identity(&metadata.artist)
    {
        if let Some(album_artist) = metadata
            .album_artist
            .as_ref()
            .filter(|value| !value.trim().is_empty())
        {
            track.album_artist = album_artist.clone();
        }
    }
    if track.album.trim().is_empty() {
        if let Some(album) = metadata
            .album
            .as_ref()
            .filter(|value| !value.trim().is_empty())
        {
            track.album = album.clone();
        }
    }
    if track.track_number.filter(|value| *value > 0).is_none() {
        track.track_number = metadata.track_number.map(|value| value as i32);
    }
    if track.disc_number.filter(|value| *value > 0).is_none() {
        track.disc_number = metadata.disc_number.map(|value| value as i32);
    }
    if track.year.filter(|value| *value > 0).is_none() {
        track.year = metadata.year;
    }
    if track.isrc.is_none() {
        track.isrc = metadata.isrc.clone();
    }
    if track.musicbrainz_recording_id.is_none() {
        track.musicbrainz_recording_id = metadata.musicbrainz_recording_id.clone();
    }
    if track.musicbrainz_release_id.is_none() {
        track.musicbrainz_release_id = metadata.musicbrainz_release_id.clone();
    }
    if track.canonical_artist_id.is_none() {
        track.canonical_artist_id = metadata.canonical_artist_id;
    }
    if track.canonical_release_id.is_none() {
        track.canonical_release_id = metadata.canonical_release_id;
    }
}

pub fn enrich_track_with_director_result(track: &mut Track, result: &DirectorTaskResult) {
    let Some(finalized) = result.finalized.as_ref() else {
        return;
    };

    enrich_track_with_normalized_metadata(track, &finalized.provenance.source_metadata);
    if track.quality_tier.is_none() {
        track.quality_tier = match finalized.provenance.validation_summary.quality {
            CandidateQuality::Lossless => Some("lossless_preferred".to_string()),
            CandidateQuality::Lossy => Some("lossy_acceptable".to_string()),
            CandidateQuality::Unknown => None,
        };
    }
}

fn find_cover_art(track_path: &Path) -> Option<String> {
    let dir = track_path.parent()?;
    let valid_names = COVER_NAMES.iter().copied().collect::<HashSet<_>>();
    let mut matches = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let file_type = entry.file_type().ok()?;
            if !file_type.is_file() {
                return None;
            }
            let file_name = entry.file_name();
            let lowered = file_name.to_string_lossy().to_ascii_lowercase();
            valid_names
                .contains(lowered.as_str())
                .then(|| entry.path().to_string_lossy().to_string())
        })
        .collect::<Vec<_>>();
    matches.sort_by(|left, right| {
        normalize_identity_text(left)
            .cmp(&normalize_identity_text(right))
            .then_with(|| left.cmp(right))
    });
    matches.into_iter().next()
}

fn cleaned_filename_title(path: &Path) -> String {
    let stem = path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .trim()
        .to_string();
    let without_prefix = if let Some(numbers) = parse_filename_numbers(&path.to_string_lossy()) {
        let track_prefix = if let Some(disc_number) = numbers.disc_number {
            format!("{disc_number:02}-{track:02}", track = numbers.track_number)
        } else {
            format!("{:02}", numbers.track_number)
        };
        stem.strip_prefix(&track_prefix)
            .map(str::trim_start)
            .map(|value| value.trim_start_matches(['-', '.', '_', ' ']).trim())
            .filter(|value| !value.is_empty())
            .unwrap_or(&stem)
            .to_string()
    } else {
        stem
    };
    without_prefix
}

fn extract_embedded_cover_art(
    tagged: &lofty::file::TaggedFile,
    track_path: &Path,
) -> Option<String> {
    let tag = tagged.primary_tag().or_else(|| tagged.first_tag())?;
    let picture = tag
        .pictures()
        .iter()
        .find(|picture| picture.pic_type() == PictureType::CoverFront)
        .or_else(|| tag.pictures().first())?;
    let bytes = picture.data();
    if bytes.is_empty() {
        return None;
    }

    let extension = if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        "png"
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        "jpg"
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        "webp"
    } else {
        return None;
    };

    let cache_dir = cover_cache_dir();
    std::fs::create_dir_all(&cache_dir).ok()?;

    let metadata = std::fs::metadata(track_path).ok();
    let mtime_ms = metadata
        .as_ref()
        .and_then(|value| value.modified().ok())
        .and_then(|value| value.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|value| value.as_millis())
        .unwrap_or_default();
    let file_size = metadata
        .as_ref()
        .map(|value| value.len())
        .unwrap_or_default();
    let cache_key = format!("{}::{file_size}::{mtime_ms}", track_path.to_string_lossy());
    let hash = blake3::hash(cache_key.as_bytes()).to_hex().to_string();
    let cache_path = cache_dir.join(format!("{hash}.{extension}"));

    if !cache_path.exists() {
        std::fs::write(&cache_path, bytes).ok()?;
    }
    Some(cache_path.to_string_lossy().to_string())
}

fn cover_cache_dir() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("Cassette")
        .join("cover-cache")
}

#[cfg(test)]
mod helper_tests {
    use super::{cleaned_filename_title, find_cover_art};

    #[test]
    fn cleaned_filename_title_strips_numeric_prefixes() {
        let path = std::path::Path::new(r"C:\Music\Artist\Album\02 - Track Name.flac");
        assert_eq!(cleaned_filename_title(path), "Track Name");
    }

    #[test]
    fn find_cover_art_matches_case_insensitive_extended_names() {
        let dir = tempfile::tempdir().expect("tempdir");
        let album_dir = dir.path().join("Album");
        std::fs::create_dir_all(&album_dir).expect("mkdir");
        let track_path = album_dir.join("01 - Song.flac");
        let cover_path = album_dir.join("FOLDER.JPEG");
        std::fs::write(&track_path, b"audio").expect("write track");
        std::fs::write(&cover_path, b"image").expect("write cover");

        let detected = find_cover_art(&track_path).expect("cover should be detected");
        assert_eq!(std::path::Path::new(&detected), cover_path.as_path());
    }
}
