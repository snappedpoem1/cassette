use super::read_track_metadata;
use crate::models::Track;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RepairSource {
    EmbeddedTag,
    FilenamePrefix,
    AlbumPattern,
}

impl RepairSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::EmbeddedTag => "embedded_tag",
            Self::FilenamePrefix => "filename_prefix",
            Self::AlbumPattern => "album_pattern",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FilenameNumbers {
    pub track_number: i32,
    pub disc_number: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepairRow {
    pub track_id: i64,
    pub path: String,
    pub old_track_number: Option<i32>,
    pub new_track_number: Option<i32>,
    pub old_disc_number: Option<i32>,
    pub new_disc_number: Option<i32>,
    pub repair_source: RepairSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnresolvedRow {
    pub track_id: i64,
    pub path: String,
    pub current_track_number: Option<i32>,
    pub current_disc_number: Option<i32>,
    pub reason: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackRepairPlan {
    pub repaired: Vec<RepairRow>,
    pub unresolved: Vec<UnresolvedRow>,
}

#[derive(Debug, Clone)]
struct CandidateTrack {
    track: Track,
    filename_numbers: Option<FilenameNumbers>,
    embedded_track_number: Option<i32>,
    embedded_disc_number: Option<i32>,
    embedded_error: Option<String>,
}

fn multi_disc_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"^(?P<disc>\d{1,2})\s*[-_.]\s*(?P<track>\d{1,2})(?:\s*[-_.]\s*|\s+)")
            .expect("multi-disc filename regex")
    })
}

fn single_disc_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"^(?P<track>\d{1,2})\s*[-_.]\s*").expect("single-disc filename regex")
    })
}

fn is_catch_all_singles_folder(path: &str) -> bool {
    Path::new(path)
        .parent()
        .and_then(|value| value.file_name())
        .and_then(|value| value.to_str())
        .map(|value| value.eq_ignore_ascii_case("singles"))
        .unwrap_or(false)
}

pub fn parse_filename_numbers(path: &str) -> Option<FilenameNumbers> {
    let filename = Path::new(path).file_stem()?.to_str()?.trim();
    if let Some(captures) = multi_disc_regex().captures(filename) {
        let disc_number = captures.name("disc")?.as_str().parse::<i32>().ok()?;
        let track_number = captures.name("track")?.as_str().parse::<i32>().ok()?;
        return (disc_number > 0 && track_number > 0).then_some(FilenameNumbers {
            track_number,
            disc_number: Some(disc_number),
        });
    }

    let captures = single_disc_regex().captures(filename)?;
    let track_number = captures.name("track")?.as_str().parse::<i32>().ok()?;
    (track_number > 0).then_some(FilenameNumbers {
        track_number,
        disc_number: None,
    })
}

pub fn build_track_repair_plan(tracks: &[Track]) -> TrackRepairPlan {
    let mut candidates = Vec::with_capacity(tracks.len());
    let mut unresolved = Vec::new();

    for track in tracks {
        let current_track_number = track.track_number.filter(|value| *value > 0);
        let current_disc_number = track.disc_number.filter(|value| *value > 0);

        if current_track_number.is_some() {
            candidates.push(CandidateTrack {
                track: track.clone(),
                filename_numbers: None,
                embedded_track_number: None,
                embedded_disc_number: current_disc_number,
                embedded_error: None,
            });
            continue;
        }

        let path = Path::new(&track.path);
        if !path.exists() {
            unresolved.push(UnresolvedRow {
                track_id: track.id,
                path: track.path.clone(),
                current_track_number: track.track_number,
                current_disc_number: track.disc_number,
                reason: "missing_file".to_string(),
            });
            continue;
        }

        let (embedded_track_number, embedded_disc_number, embedded_error) = match read_track_metadata(path) {
            Ok(value) => (
                value.track_number.filter(|candidate| *candidate > 0),
                value.disc_number.filter(|candidate| *candidate > 0),
                None,
            ),
            Err(error) => (None, None, Some(error.to_string())),
        };

        candidates.push(CandidateTrack {
            track: track.clone(),
            filename_numbers: parse_filename_numbers(&track.path),
            embedded_track_number,
            embedded_disc_number,
            embedded_error,
        });
    }

    let mut repaired = Vec::new();
    let mut candidate_rows = Vec::new();
    for candidate in candidates {
        let current_track_number = candidate.track.track_number.filter(|value| *value > 0);
        let current_disc_number = candidate.track.disc_number.filter(|value| *value > 0);

        if current_track_number.is_some() {
            candidate_rows.push(candidate);
            continue;
        }

        if let Some(next_track_number) = candidate.embedded_track_number {
            repaired.push(RepairRow {
                track_id: candidate.track.id,
                path: candidate.track.path.clone(),
                old_track_number: candidate.track.track_number,
                new_track_number: Some(next_track_number),
                old_disc_number: candidate.track.disc_number,
                new_disc_number: current_disc_number.or(candidate.embedded_disc_number),
                repair_source: RepairSource::EmbeddedTag,
            });
            candidate_rows.push(candidate);
            continue;
        }

        if let Some(filename_numbers) = &candidate.filename_numbers {
            repaired.push(RepairRow {
                track_id: candidate.track.id,
                path: candidate.track.path.clone(),
                old_track_number: candidate.track.track_number,
                new_track_number: Some(filename_numbers.track_number),
                old_disc_number: candidate.track.disc_number,
                new_disc_number: current_disc_number.or(filename_numbers.disc_number),
                repair_source: RepairSource::FilenamePrefix,
            });
        }

        candidate_rows.push(candidate);
    }

    let mut repaired_by_track = repaired
        .iter()
        .map(|row| (row.track_id, row.clone()))
        .collect::<HashMap<_, _>>();

    let candidate_reason_map = candidate_rows
        .iter()
        .map(|candidate| {
            let reason = candidate
                .embedded_error
                .as_ref()
                .map(|error| format!("embedded_tag_read_failed:{error}"))
                .unwrap_or_else(|| "no_recoverable_track_number".to_string());
            (candidate.track.id, reason)
        })
        .collect::<HashMap<_, _>>();

    let mut by_folder = HashMap::<String, Vec<CandidateTrack>>::new();
    for candidate in candidate_rows {
        let folder = Path::new(&candidate.track.path)
            .parent()
            .map(|value| value.to_string_lossy().to_string())
            .unwrap_or_default();
        by_folder.entry(folder).or_default().push(candidate);
    }

    for mut group in by_folder.into_values() {
        if group
            .first()
            .map(|candidate| is_catch_all_singles_folder(&candidate.track.path))
            .unwrap_or(false)
        {
            continue;
        }

        group.sort_by(|left, right| {
            let left_name = Path::new(&left.track.path)
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or_default()
                .to_ascii_lowercase();
            let right_name = Path::new(&right.track.path)
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or_default()
                .to_ascii_lowercase();
            left_name.cmp(&right_name).then_with(|| left.track.path.cmp(&right.track.path))
        });

        let effective = group
            .iter()
            .map(|candidate| {
                if let Some(existing) = repaired_by_track.get(&candidate.track.id) {
                    (
                        existing.new_track_number,
                        existing.new_disc_number.or(Some(1)),
                    )
                } else {
                    (
                        candidate.track.track_number.filter(|value| *value > 0),
                        candidate.track.disc_number.filter(|value| *value > 0).or(Some(1)),
                    )
                }
            })
            .collect::<Vec<_>>();

        let mut index = 0usize;
        while index < group.len() {
            if effective[index].0.is_some() {
                index += 1;
                continue;
            }

            let start = index;
            while index < group.len() && effective[index].0.is_none() {
                index += 1;
            }
            let end = index;

            let previous = (0..start).rev().find_map(|candidate_index| {
                effective[candidate_index]
                    .0
                    .zip(effective[candidate_index].1)
                    .map(|(track_number, disc_number)| (candidate_index, track_number, disc_number))
            });
            let next = (end..group.len()).find_map(|candidate_index| {
                effective[candidate_index]
                    .0
                    .zip(effective[candidate_index].1)
                    .map(|(track_number, disc_number)| (candidate_index, track_number, disc_number))
            });

            let gap_len = i32::try_from(end - start).unwrap_or(i32::MAX);
            let inferred = match (previous, next) {
                (Some((_, previous_track, previous_disc)), Some((_, next_track, next_disc)))
                    if previous_disc == next_disc && next_track - previous_track == gap_len + 1 =>
                {
                    Some((previous_track + 1, previous_disc))
                }
                (None, Some((_, next_track, next_disc))) if next_track == gap_len + 1 => {
                    Some((1, next_disc))
                }
                (Some((previous_index, previous_track, previous_disc)), None)
                    if folder_prefix_is_contiguous(&effective[..=previous_index], previous_disc) =>
                {
                    Some((previous_track + 1, previous_disc))
                }
                _ => None,
            };

            if let Some((starting_track, disc_number)) = inferred {
                for offset in 0..(end - start) {
                    let candidate = &group[start + offset];
                    let new_track_number = starting_track + i32::try_from(offset).unwrap_or(0);
                    repaired_by_track.entry(candidate.track.id).or_insert_with(|| RepairRow {
                        track_id: candidate.track.id,
                        path: candidate.track.path.clone(),
                        old_track_number: candidate.track.track_number,
                        new_track_number: Some(new_track_number),
                        old_disc_number: candidate.track.disc_number,
                        new_disc_number: Some(disc_number),
                        repair_source: RepairSource::AlbumPattern,
                    });
                }
            }
        }
    }

    for row in repaired_by_track.into_values() {
        if row.old_track_number != row.new_track_number || row.old_disc_number != row.new_disc_number {
            repaired.push(row);
        }
    }

    repaired.sort_by(|left, right| left.path.cmp(&right.path).then_with(|| left.track_id.cmp(&right.track_id)));
    repaired.dedup_by(|left, right| left.track_id == right.track_id);

    let repaired_track_ids = repaired.iter().map(|row| row.track_id).collect::<std::collections::HashSet<_>>();
    for track in tracks {
        let current_track_number = track.track_number.filter(|value| *value > 0);
        if current_track_number.is_some() || repaired_track_ids.contains(&track.id) {
            continue;
        }
        unresolved.push(UnresolvedRow {
            track_id: track.id,
            path: track.path.clone(),
            current_track_number: track.track_number,
            current_disc_number: track.disc_number,
            reason: candidate_reason_map
                .get(&track.id)
                .cloned()
                .unwrap_or_else(|| "no_recoverable_track_number".to_string()),
        });
    }

    unresolved.sort_by(|left, right| left.path.cmp(&right.path).then_with(|| left.track_id.cmp(&right.track_id)));
    unresolved.dedup_by(|left, right| left.track_id == right.track_id);

    TrackRepairPlan { repaired, unresolved }
}

fn folder_prefix_is_contiguous(entries: &[(Option<i32>, Option<i32>)], disc_number: i32) -> bool {
    let mut values = entries
        .iter()
        .filter_map(|(track_number, candidate_disc)| match (track_number, candidate_disc) {
            (Some(track_number), Some(candidate_disc)) if *candidate_disc == disc_number => Some(*track_number),
            _ => None,
        })
        .collect::<Vec<_>>();
    values.sort_unstable();
    values
        .iter()
        .enumerate()
        .all(|(index, value)| *value == i32::try_from(index + 1).unwrap_or(i32::MAX))
}

#[cfg(test)]
mod tests {
    use super::{build_track_repair_plan, parse_filename_numbers, RepairSource};
    use crate::models::Track;

    fn sample_track(id: i64, path: &str, track_number: Option<i32>, disc_number: Option<i32>) -> Track {
        Track {
            id,
            path: path.to_string(),
            title: String::new(),
            artist: "Artist".to_string(),
            album: "Album".to_string(),
            album_artist: "Artist".to_string(),
            track_number,
            disc_number,
            year: None,
            duration_secs: 0.0,
            sample_rate: None,
            bit_depth: None,
            bitrate_kbps: None,
            format: "FLAC".to_string(),
            file_size: 0,
            cover_art_path: None,
            added_at: String::new(),
        }
    }

    #[test]
    fn parses_filename_track_patterns() {
        assert_eq!(
            parse_filename_numbers(r"A:\Music\Artist\Album\02 - Track.flac")
                .expect("single-disc"),
            super::FilenameNumbers {
                track_number: 2,
                disc_number: None,
            }
        );
        assert_eq!(
            parse_filename_numbers(r"A:\Music\Artist\Album\2-03 - Track.flac")
                .expect("disc-track"),
            super::FilenameNumbers {
                track_number: 3,
                disc_number: Some(2),
            }
        );
        assert_eq!(
            parse_filename_numbers(r"A:\Music\Artist\Album\02-03 - Track.flac")
                .expect("zero-padded disc-track"),
            super::FilenameNumbers {
                track_number: 3,
                disc_number: Some(2),
            }
        );
        assert_eq!(
            parse_filename_numbers(r"A:\Music\Artist\Album\07 Hysteria – Live.flac")
                .is_none(),
            true
        );
        assert!(parse_filename_numbers(r"A:\Music\Artist\Album\Track Name.flac").is_none());
    }

    #[test]
    fn album_pattern_fills_deterministic_gap() {
        let dir = tempfile::tempdir().expect("tempdir");
        let first = dir.path().join("A Intro.flac");
        let second = dir.path().join("B Middle.flac");
        let third = dir.path().join("Finale.flac");
        std::fs::write(&first, b"not-audio").expect("write first");
        std::fs::write(&second, b"not-audio").expect("write second");
        std::fs::write(&third, b"not-audio").expect("write third");

        let plan = build_track_repair_plan(&[
            sample_track(1, &first.to_string_lossy(), Some(1), Some(1)),
            sample_track(2, &second.to_string_lossy(), Some(2), Some(1)),
            sample_track(3, &third.to_string_lossy(), Some(0), Some(0)),
        ]);

        let repaired = plan
            .repaired
            .iter()
            .find(|row| row.track_id == 3)
            .expect("gap-filled repair");
        assert_eq!(repaired.new_track_number, Some(3));
        assert_eq!(repaired.new_disc_number, Some(1));
        assert_eq!(repaired.repair_source, RepairSource::AlbumPattern);
    }

    #[test]
    fn unresolved_files_without_numbering_are_reported() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("No Number.flac");
        std::fs::write(&path, b"not-audio").expect("write");

        let plan = build_track_repair_plan(&[sample_track(
            1,
            &path.to_string_lossy(),
            Some(0),
            Some(0),
        )]);

        assert!(plan.repaired.is_empty());
        assert_eq!(plan.unresolved.len(), 1);
        assert!(!plan.unresolved[0].reason.is_empty());
    }

    #[test]
    fn valid_track_numbers_do_not_force_file_reads() {
        let missing_path = r"A:\Music\Artist\Album\01 - Existing.flac";
        let plan = build_track_repair_plan(&[sample_track(
            1,
            missing_path,
            Some(1),
            Some(1),
        )]);

        assert!(plan.repaired.is_empty());
        assert!(plan.unresolved.is_empty());
    }
}
