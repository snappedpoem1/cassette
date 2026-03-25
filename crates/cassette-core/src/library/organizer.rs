use crate::models::Track;
use crate::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Target folder structure: `library_base/Artist/Album (Year)/disc_track - Title.ext`
/// e.g. `A:\music\Coheed and Cambria\Good Apollo I'm Burning Star IV (2005)\01-01 - Keeping the Blade.flac`

#[derive(Debug, Clone)]
pub struct OrganizeResult {
    pub moved: Vec<FileMove>,
    pub skipped: Vec<String>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FileMove {
    pub old_path: String,
    pub new_path: String,
    pub track_id: i64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DuplicateGroup {
    pub key: String,
    pub tracks: Vec<DuplicateTrack>,
    pub recommendation: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DuplicateTrack {
    pub id: i64,
    pub path: String,
    pub format: String,
    pub bit_depth: Option<u32>,
    pub sample_rate: Option<u32>,
    pub bitrate_kbps: Option<u32>,
    pub file_size: u64,
    pub is_best: bool,
}

/// Build the canonical destination path for a track
pub fn canonical_path(library_base: &str, track: &Track) -> PathBuf {
    let artist = sanitize_filename(&track.album_artist);
    let artist = if artist.is_empty() { sanitize_filename(&track.artist) } else { artist };
    let artist = if artist.is_empty() { "Unknown Artist".to_string() } else { artist };

    let album = sanitize_filename(&track.album);
    let album = if album.is_empty() { "Unknown Album".to_string() } else { album };

    let year_suffix = track.year
        .filter(|&y| y > 0)
        .map(|y| format!(" ({y})"))
        .unwrap_or_default();

    let album_folder = format!("{album}{year_suffix}");

    let disc = track.disc_number.unwrap_or(1).max(1);
    let num = track.track_number.unwrap_or(0).max(0);
    let title = sanitize_filename(&track.title);
    let title = if title.is_empty() {
        Path::new(&track.path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Unknown")
            .to_string()
    } else {
        title
    };

    let ext = Path::new(&track.path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("flac")
        .to_lowercase();

    let filename = if disc > 1 {
        format!("{:02}-{:02} - {}.{}", disc, num, title, ext)
    } else {
        format!("{:02} - {}.{}", num, title, ext)
    };

    Path::new(library_base)
        .join(artist)
        .join(album_folder)
        .join(filename)
}

/// Organize a batch of tracks into canonical folder structure.
/// Returns list of moves. Does NOT update the database — caller must do that.
pub fn organize_tracks(library_base: &str, tracks: &[Track], dry_run: bool) -> OrganizeResult {
    let mut result = OrganizeResult {
        moved: Vec::new(),
        skipped: Vec::new(),
        errors: Vec::new(),
    };

    for track in tracks {
        let dest = canonical_path(library_base, track);
        let src = Path::new(&track.path);

        if !src.exists() {
            result.errors.push(format!("Source missing: {}", track.path));
            continue;
        }

        let dest_str = dest.to_string_lossy().to_string();
        if dunce::canonicalize(src).ok() == dunce::canonicalize(&dest).ok() {
            result.skipped.push(format!("Already in place: {}", track.path));
            continue;
        }

        if dest.exists() {
            result.skipped.push(format!("Destination exists: {dest_str}"));
            continue;
        }

        if dry_run {
            result.moved.push(FileMove {
                old_path: track.path.clone(),
                new_path: dest_str,
                track_id: track.id,
            });
            continue;
        }

        // Create parent directories
        if let Some(parent) = dest.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                result.errors.push(format!("Cannot create dir {}: {e}", parent.display()));
                continue;
            }
        }

        match std::fs::rename(src, &dest) {
            Ok(()) => {
                result.moved.push(FileMove {
                    old_path: track.path.clone(),
                    new_path: dest_str,
                    track_id: track.id,
                });
            }
            Err(e) => {
                // Cross-device: try copy + delete
                match std::fs::copy(src, &dest) {
                    Ok(_) => {
                        let _ = std::fs::remove_file(src);
                        result.moved.push(FileMove {
                            old_path: track.path.clone(),
                            new_path: dest_str,
                            track_id: track.id,
                        });
                    }
                    Err(ce) => {
                        result.errors.push(format!(
                            "Move failed for {}: rename={e}, copy={ce}", track.path
                        ));
                    }
                }
            }
        }
    }

    // Clean up empty directories left behind
    if !dry_run {
        let mut dirs_to_check: Vec<PathBuf> = result.moved.iter()
            .filter_map(|m| Path::new(&m.old_path).parent().map(|p| p.to_path_buf()))
            .collect();
        dirs_to_check.sort();
        dirs_to_check.dedup();

        for dir in dirs_to_check.iter().rev() {
            cleanup_empty_dir(dir);
        }
    }

    result
}

/// Detect duplicate tracks in the library.
/// Groups by (normalized artist, normalized album, track_number, disc_number).
pub fn find_duplicates(tracks: &[Track]) -> Vec<DuplicateGroup> {
    let mut groups: HashMap<String, Vec<&Track>> = HashMap::new();

    for track in tracks {
        let key = format!(
            "{}::{}::{}::{}",
            normalize(&track.album_artist),
            normalize(&track.album),
            track.disc_number.unwrap_or(1),
            track.track_number.unwrap_or(0),
        );
        groups.entry(key).or_default().push(track);
    }

    groups.into_iter()
        .filter(|(_, group)| group.len() > 1)
        .map(|(key, group)| {
            let best_idx = pick_best_quality(&group);
            let tracks: Vec<DuplicateTrack> = group.iter().enumerate().map(|(i, t)| {
                DuplicateTrack {
                    id: t.id,
                    path: t.path.clone(),
                    format: t.format.clone(),
                    bit_depth: t.bit_depth,
                    sample_rate: t.sample_rate,
                    bitrate_kbps: t.bitrate_kbps,
                    file_size: t.file_size,
                    is_best: i == best_idx,
                }
            }).collect();

            let best = &tracks[best_idx];
            let recommendation = format!(
                "Keep {} ({}{})",
                Path::new(&best.path).file_name().unwrap_or_default().to_string_lossy(),
                best.format,
                best.bit_depth.map(|b| format!(" {b}bit")).unwrap_or_default(),
            );

            DuplicateGroup { key, tracks, recommendation }
        })
        .collect()
}

/// Pick the best quality track from a group:
/// FLAC > everything, then higher bit depth, then higher sample rate, then larger file
fn pick_best_quality(tracks: &[&Track]) -> usize {
    tracks.iter().enumerate()
        .max_by_key(|(_, t)| {
            let format_score: i64 = match t.format.to_uppercase().as_str() {
                "FLAC" => 1000,
                "WAV" | "AIFF" => 900,
                "OPUS" | "OGG" => 500,
                "M4A" | "AAC" => 400,
                "MP3" => 300,
                _ => 200,
            };
            let depth = t.bit_depth.unwrap_or(16) as i64;
            let rate = t.sample_rate.unwrap_or(44100) as i64 / 1000;
            let size = (t.file_size / 1024) as i64; // KB
            (format_score, depth, rate, size)
        })
        .map(|(i, _)| i)
        .unwrap_or(0)
}

/// Move completed downloads from staging to library
pub fn ingest_staging(staging: &str, library_base: &str) -> Result<Vec<String>> {
    let staging_path = Path::new(staging);
    if !staging_path.exists() {
        return Ok(Vec::new());
    }

    let mut ingested = Vec::new();

    // Walk staging directory for audio files
    for entry in walkdir::WalkDir::new(staging)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() { continue; }
        let ext = entry.path().extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        if !matches!(ext.as_str(), "flac" | "mp3" | "m4a" | "aac" | "ogg" | "opus" | "wav" | "aiff") {
            continue;
        }

        // Read tags to determine destination
        match crate::library::read_track_metadata(entry.path()) {
            Ok(track) => {
                let dest = canonical_path(library_base, &track);
                if dest.exists() {
                    continue; // Already have this file
                }
                if let Some(parent) = dest.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                match std::fs::rename(entry.path(), &dest) {
                    Ok(()) => ingested.push(dest.to_string_lossy().to_string()),
                    Err(_) => {
                        // Cross-device
                        if std::fs::copy(entry.path(), &dest).is_ok() {
                            let _ = std::fs::remove_file(entry.path());
                            ingested.push(dest.to_string_lossy().to_string());
                        }
                    }
                }
            }
            Err(_) => continue,
        }
    }

    // Clean empty dirs in staging
    cleanup_empty_dir(staging_path);

    Ok(ingested)
}

fn sanitize_filename(name: &str) -> String {
    let trimmed = name.trim();
    trimmed.chars().map(|c| match c {
        '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
        '\0' => '_',
        c => c,
    })
    .collect::<String>()
    .trim_end_matches('.')
    .trim()
    .to_string()
}

fn normalize(s: &str) -> String {
    s.trim().to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn cleanup_empty_dir(dir: &Path) {
    // Walk up removing empty directories
    let mut current = dir.to_path_buf();
    for _ in 0..5 {
        if !current.is_dir() { break; }
        let is_empty = std::fs::read_dir(&current)
            .map(|mut entries| entries.next().is_none())
            .unwrap_or(false);
        if !is_empty { break; }
        let _ = std::fs::remove_dir(&current);
        match current.parent() {
            Some(p) => current = p.to_path_buf(),
            None => break,
        }
    }
}
