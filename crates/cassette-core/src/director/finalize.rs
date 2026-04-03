use crate::director::config::DuplicatePolicy;
use crate::director::error::FinalizationError;
use crate::director::models::{CandidateSelection, FinalizedTrack, NormalizedTrack, ProvenanceRecord};
use std::path::{Path, PathBuf};

pub fn merge_normalized_track(
    requested: &NormalizedTrack,
    resolved: Option<&NormalizedTrack>,
) -> NormalizedTrack {
    let Some(resolved) = resolved else {
        return requested.clone();
    };

    NormalizedTrack {
        spotify_track_id: requested
            .spotify_track_id
            .clone()
            .or_else(|| resolved.spotify_track_id.clone()),
        source_album_id: requested
            .source_album_id
            .clone()
            .or_else(|| resolved.source_album_id.clone()),
        source_artist_id: requested
            .source_artist_id
            .clone()
            .or_else(|| resolved.source_artist_id.clone()),
        source_playlist: requested.source_playlist.clone(),
        artist: if requested.artist.trim().is_empty() {
            resolved.artist.clone()
        } else {
            requested.artist.clone()
        },
        album_artist: requested
            .album_artist
            .clone()
            .or_else(|| resolved.album_artist.clone()),
        title: if requested.title.trim().is_empty() {
            resolved.title.clone()
        } else {
            requested.title.clone()
        },
        album: requested.album.clone().or_else(|| resolved.album.clone()),
        track_number: requested.track_number.or(resolved.track_number),
        disc_number: requested.disc_number.or(resolved.disc_number),
        year: requested.year.or(resolved.year),
        duration_secs: requested.duration_secs.or(resolved.duration_secs),
        isrc: requested.isrc.clone().or_else(|| resolved.isrc.clone()),
        musicbrainz_recording_id: requested
            .musicbrainz_recording_id
            .clone()
            .or_else(|| resolved.musicbrainz_recording_id.clone()),
        musicbrainz_release_id: requested
            .musicbrainz_release_id
            .clone()
            .or_else(|| resolved.musicbrainz_release_id.clone()),
        canonical_artist_id: requested.canonical_artist_id.or(resolved.canonical_artist_id),
        canonical_release_id: requested.canonical_release_id.or(resolved.canonical_release_id),
    }
}

fn merge_embedded_metadata(requested: &NormalizedTrack, staged_path: &Path) -> NormalizedTrack {
    match crate::library::read_track_metadata(staged_path) {
        Ok(track) => merge_normalized_track(
            requested,
            Some(&NormalizedTrack {
                spotify_track_id: None,
                source_album_id: None,
                source_artist_id: None,
                source_playlist: None,
                artist: track.artist,
                album_artist: if track.album_artist.trim().is_empty() {
                    None
                } else {
                    Some(track.album_artist)
                },
                title: track.title,
                album: if track.album.trim().is_empty() {
                    None
                } else {
                    Some(track.album)
                },
                track_number: track.track_number.and_then(|value| u32::try_from(value).ok()),
                disc_number: track.disc_number.and_then(|value| u32::try_from(value).ok()),
                year: track.year,
                duration_secs: Some(track.duration_secs),
                isrc: None,
                musicbrainz_recording_id: None,
                musicbrainz_release_id: None,
                canonical_artist_id: None,
                canonical_release_id: None,
            }),
        ),
        Err(_) => requested.clone(),
    }
}

pub fn build_final_path(library_root: &Path, target: &NormalizedTrack, extension: &str) -> PathBuf {
    let artist = sanitize_component(
        target
            .album_artist
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(&target.artist),
    );
    let artist = if artist.is_empty() {
        "Unknown Artist".to_string()
    } else {
        artist
    };

    let album = sanitize_component(target.album.as_deref().unwrap_or("Unknown Album"));
    let album = if album.is_empty() {
        "Unknown Album".to_string()
    } else {
        album
    };

    let title = sanitize_component(&target.title);
    let title = if title.is_empty() {
        "Unknown Title".to_string()
    } else {
        title
    };

    let prefix = match target.track_number {
        Some(track_number) => {
            let disc = target.disc_number.unwrap_or(1);
            if disc > 1 {
                format!("{disc:02}-{track_number:02}")
            } else {
                format!("{track_number:02}")
            }
        }
        None => "00".to_string(),
    };

    library_root
        .join(artist)
        .join(album)
        .join(format!("{prefix} - {title}.{}", extension.to_ascii_lowercase()))
}

pub async fn finalize_selected_candidate(
    library_root: PathBuf,
    selection: CandidateSelection,
    target: NormalizedTrack,
    duplicate_policy: DuplicatePolicy,
    provenance: ProvenanceRecord,
) -> Result<FinalizedTrack, FinalizationError> {
    let temp_path_for_error = selection.temp_path.clone();
    let extension = selection
        .temp_path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("bin")
        .to_string();
    let effective_target = merge_embedded_metadata(&target, &selection.temp_path);
    let destination = build_final_path(&library_root, &effective_target, &extension);
    let source = selection.temp_path.clone();

    tokio::task::spawn_blocking(move || {
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent).map_err(|error| FinalizationError::MoveFailed {
                from: source.clone(),
                to: destination.clone(),
                message: error.to_string(),
            })?;
        }

        let mut replaced_existing = false;
        if destination.exists() {
            match duplicate_policy {
                DuplicatePolicy::KeepExisting => {
                    return Err(FinalizationError::DestinationExists {
                        path: destination.clone(),
                    });
                }
                DuplicatePolicy::ReplaceIfBetter => {
                    if !replacement_should_win(&destination, &selection) {
                        return Err(FinalizationError::ReplacementRejected {
                            path: destination.clone(),
                        });
                    }
                    std::fs::remove_file(&destination).map_err(|error| FinalizationError::MoveFailed {
                        from: source.clone(),
                        to: destination.clone(),
                        message: error.to_string(),
                    })?;
                    replaced_existing = true;
                }
            }
        }

        std::fs::rename(&source, &destination).or_else(|rename_error| {
            std::fs::copy(&source, &destination)
                .map_err(|copy_error| FinalizationError::MoveFailed {
                    from: source.clone(),
                    to: destination.clone(),
                    message: format!("rename={rename_error}; copy={copy_error}"),
                })?;
            std::fs::remove_file(&source).map_err(|error| FinalizationError::MoveFailed {
                from: source.clone(),
                to: destination.clone(),
                message: error.to_string(),
            })?;
            Ok(())
        })?;

        Ok(FinalizedTrack {
            path: destination.clone(),
            replaced_existing,
            provenance: ProvenanceRecord {
                final_path: destination,
                ..provenance
            },
        })
    })
    .await
    .map_err(|error| FinalizationError::MoveFailed {
        from: temp_path_for_error,
        to: library_root,
        message: error.to_string(),
    })?
}

fn replacement_should_win(existing_path: &Path, selection: &CandidateSelection) -> bool {
    let existing_track = crate::library::read_track_metadata(existing_path).ok();
    let incoming_size = std::fs::metadata(&selection.temp_path)
        .map(|metadata| metadata.len())
        .unwrap_or_default();

    match existing_track {
        Some(existing_track) => {
            let existing_quality = match existing_track.format.to_ascii_lowercase().as_str() {
                "flac" | "wav" | "aiff" => 2,
                "mp3" | "m4a" | "aac" | "ogg" | "opus" => 1,
                _ => 0,
            };
            let incoming_quality = match selection.validation.quality {
                crate::director::models::CandidateQuality::Lossless => 2,
                crate::director::models::CandidateQuality::Lossy => 1,
                crate::director::models::CandidateQuality::Unknown => 0,
            };
            if incoming_quality != existing_quality {
                return incoming_quality > existing_quality;
            }

            let existing_bitrate = existing_track.bitrate_kbps.unwrap_or_default();
            let incoming_bitrate = if selection.validation.duration_secs.unwrap_or_default() > 0.0 {
                ((incoming_size as f64 * 8.0)
                    / selection.validation.duration_secs.unwrap_or(1.0)
                    / 1000.0) as u32
            } else {
                0
            };
            if incoming_bitrate != existing_bitrate {
                return incoming_bitrate > existing_bitrate;
            }

            incoming_size > existing_track.file_size
        }
        None => {
            let existing_len = std::fs::metadata(existing_path)
                .map(|metadata| metadata.len())
                .unwrap_or_default();
            incoming_size > existing_len
        }
    }
}

fn sanitize_component(value: &str) -> String {
    value
        .trim()
        .chars()
        .map(|character| match character {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '\0' => '_',
            other => other,
        })
        .collect::<String>()
        .trim()
        .trim_end_matches('.')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::director::models::{
        CandidateQuality, CandidateScore, SelectionReason, ValidationReport,
    };
    use chrono::Utc;
    use std::collections::BTreeMap;
    use tempfile::tempdir;

    fn target() -> NormalizedTrack {
        NormalizedTrack {
            spotify_track_id: None,
            source_album_id: None,
            source_artist_id: None,
            source_playlist: None,
            artist: "AC/DC".to_string(),
            album_artist: None,
            title: "Back:In?Black".to_string(),
            album: Some("Greatest/ Hits".to_string()),
            track_number: Some(1),
            disc_number: Some(1),
            year: None,
            duration_secs: None,
            isrc: None,
            musicbrainz_recording_id: None,
            musicbrainz_release_id: None,
            canonical_artist_id: None,
            canonical_release_id: None,
        }
    }

    fn selection(path: PathBuf) -> CandidateSelection {
        CandidateSelection {
            provider_id: "mock".to_string(),
            provider_candidate_id: "mock-candidate".to_string(),
            temp_path: path,
            score: CandidateScore {
                total: 10,
                metadata_match_points: 1,
                duration_points: 1,
                codec_points: 1,
                provider_points: 1,
                validation_points: 1,
                size_points: 1,
                bitrate_points: 0,
                format_points: 0,
            },
            reason: SelectionReason {
                summary: "test".to_string(),
                details: BTreeMap::new(),
            },
            validation: ValidationReport {
                is_valid: true,
                format_name: Some("flac".to_string()),
                duration_secs: Some(180.0),
                audio_readable: true,
                header_readable: true,
                extension_ok: true,
                file_size: 1024,
                quality: CandidateQuality::Lossless,
                issues: Vec::new(),
            },
            cover_art_url: None,
        }
    }

    fn provenance() -> ProvenanceRecord {
        ProvenanceRecord {
            task_id: "task-1".to_string(),
            source_metadata: target(),
            selected_provider: "mock".to_string(),
            selected_provider_candidate_id: Some("mock-candidate".to_string()),
            score_reason: SelectionReason {
                summary: "test".to_string(),
                details: BTreeMap::new(),
            },
            validation_summary: ValidationReport {
                is_valid: true,
                format_name: Some("flac".to_string()),
                duration_secs: Some(180.0),
                audio_readable: true,
                header_readable: true,
                extension_ok: true,
                file_size: 1024,
                quality: CandidateQuality::Lossless,
                issues: Vec::new(),
            },
            final_path: PathBuf::new(),
            acquired_at: Utc::now(),
        }
    }

    #[test]
    fn path_builder_sanitizes_components() {
        let path = build_final_path(Path::new("Library"), &target(), "FLAC");
        assert!(path.to_string_lossy().contains("AC_DC"));
        assert!(path.to_string_lossy().contains("Greatest_ Hits"));
        assert!(path.to_string_lossy().contains("01 - Back_In_Black.flac"));
    }

    #[test]
    fn merge_normalized_track_uses_resolved_track_number_when_request_is_missing() {
        let mut requested = target();
        requested.track_number = None;
        requested.disc_number = None;

        let resolved = NormalizedTrack {
            track_number: Some(7),
            disc_number: Some(2),
            isrc: Some("ISRC123".to_string()),
            ..target()
        };

        let merged = merge_normalized_track(&requested, Some(&resolved));
        assert_eq!(merged.track_number, Some(7));
        assert_eq!(merged.disc_number, Some(2));
        assert_eq!(merged.isrc.as_deref(), Some("ISRC123"));
        assert_eq!(merged.title, requested.title);
    }

    #[tokio::test]
    async fn finalization_respects_duplicate_policy() {
        let dir = tempdir().expect("temp dir");
        let src = dir.path().join("incoming.flac");
        std::fs::write(&src, vec![1_u8; 1024]).expect("write src");

        let existing = build_final_path(dir.path(), &target(), "flac");
        std::fs::create_dir_all(existing.parent().expect("parent")).expect("mkdirs");
        std::fs::write(&existing, vec![1_u8; 2048]).expect("write existing");

        let result = finalize_selected_candidate(
            dir.path().to_path_buf(),
            selection(src),
            target(),
            DuplicatePolicy::KeepExisting,
            provenance(),
        )
        .await;

        assert!(matches!(result, Err(FinalizationError::DestinationExists { .. })));
    }
}
