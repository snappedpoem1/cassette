use crate::models::Track;
use crate::library::track_number_repair::parse_filename_numbers;
use crate::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub const ZERO_TRACK_PREFIX: &str = "00 - ";

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
    let existing_number = existing_track_number_prefix(&track.path);
    let effective_num = if num > 0 {
        num
    } else {
        existing_number.unwrap_or(0)
    };
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

    let filename = if should_preserve_existing_basename(track, effective_num) {
        existing_filename_or_title(&track.path, &title, &ext)
    } else if disc > 1 {
        format!("{:02}-{:02} - {}.{}", disc, effective_num, title, ext)
    } else {
        format!("{:02} - {}.{}", effective_num, title, ext)
    };

    Path::new(library_base)
        .join(artist)
        .join(album_folder)
        .join(filename)
}

fn existing_filename_or_title(path: &str, title: &str, ext: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(|value| value.to_str())
        .map(|value| value.to_string())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| format!("{title}.{ext}"))
}

fn should_preserve_existing_basename(track: &Track, effective_num: i32) -> bool {
    if effective_num <= 0 {
        return true;
    }

    track.album.eq_ignore_ascii_case("Singles") && existing_track_number_prefix(&track.path).is_none()
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
        // Skip files with no usable tags — moving them to "Unknown Artist" is destructive
        let has_artist = !track.album_artist.trim().is_empty() || !track.artist.trim().is_empty();
        let has_album = !track.album.trim().is_empty();
        if !has_artist || !has_album {
            result.skipped.push(format!("No tags (artist/album empty): {}", track.path));
            continue;
        }

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
    let sanitized = trimmed.chars().map(|c| match c {
        '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
        '\0' => '_',
        c => c,
    })
    .collect::<String>()
    .trim_end_matches('.')
    .trim()
    .to_string();

    if sanitized.is_empty() && !trimmed.is_empty() {
        if trimmed.chars().all(|c| c == '.') {
            return "dot".to_string();
        }
        return "Unknown".to_string();
    }

    sanitized
}

pub fn existing_track_number_prefix(path: &str) -> Option<i32> {
    parse_filename_numbers(path).map(|value| value.track_number)
}

pub fn is_zero_track_rename(old_path: &str, new_path: &str) -> bool {
    let old_prefix = existing_track_number_prefix(old_path);
    let new_filename = Path::new(new_path)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    old_prefix.is_some() && new_filename.starts_with(ZERO_TRACK_PREFIX)
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

#[cfg(test)]
mod tests {
    use super::{canonical_path, sanitize_filename};
    use crate::models::Track;

    fn sample_track() -> Track {
        Track {
            id: 1,
            path: r"A:\music\girl in red\if i could make it go quiet (2021)\09 - 09 - Unknown.flac"
                .to_string(),
            title: ".".to_string(),
            artist: "girl in red".to_string(),
            album: "if i could make it go quiet".to_string(),
            album_artist: "girl in red".to_string(),
            track_number: Some(9),
            disc_number: Some(1),
            year: Some(2021),
            duration_secs: 0.0,
            sample_rate: None,
            bit_depth: None,
            bitrate_kbps: None,
            format: "flac".to_string(),
            file_size: 0,
            cover_art_path: None,
            isrc: None,
            musicbrainz_recording_id: None,
            musicbrainz_release_id: None,
            canonical_artist_id: None,
            canonical_release_id: None,
            quality_tier: None,
            content_hash: None,
            added_at: String::new(),
        }
    }

    #[test]
    fn sanitize_filename_handles_punctuation_only_titles_stably() {
        assert_eq!(sanitize_filename("."), "dot");
        assert_eq!(sanitize_filename("..."), "dot");
        assert_eq!(sanitize_filename("///"), "___");
    }

    #[test]
    fn canonical_path_does_not_reuse_existing_garbage_basename() {
        let path = canonical_path(r"A:\music", &sample_track());
        assert_eq!(
            path.to_string_lossy(),
            r"A:\music\girl in red\if i could make it go quiet (2021)\09 - dot.flac"
        );
    }

    #[test]
    fn canonical_path_preserves_existing_basename_when_track_number_is_untrusted() {
        let mut track = sample_track();
        track.path = r"A:\music\Artist\Singles\In America.flac".to_string();
        track.title = "In America".to_string();
        track.artist = "Artist".to_string();
        track.album = "Singles".to_string();
        track.album_artist = "Artist".to_string();
        track.track_number = Some(0);
        track.disc_number = Some(0);
        track.year = None;

        let path = canonical_path(r"A:\music", &track);
        assert_eq!(
            path.to_string_lossy(),
            r"A:\music\Artist\Singles\In America.flac"
        );
    }

    #[test]
    fn canonical_path_preserves_existing_basename_for_catch_all_singles_without_prefix() {
        let mut track = sample_track();
        track.path = r"A:\music\Artist\Singles\2 X 4.flac".to_string();
        track.title = "2 X 4".to_string();
        track.artist = "Artist".to_string();
        track.album = "Singles".to_string();
        track.album_artist = "Artist".to_string();
        track.track_number = Some(2);
        track.disc_number = Some(1);
        track.year = None;

        let path = canonical_path(r"A:\music", &track);
        assert_eq!(
            path.to_string_lossy(),
            r"A:\music\Artist\Singles\2 X 4.flac"
        );
    }
}
