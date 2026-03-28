use crate::director::config::QualityPolicy;
use crate::director::error::ValidationError;
use crate::director::models::{CandidateQuality, NormalizedTrack, ValidationIssue, ValidationReport};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::probe::Hint;

pub async fn validate_candidate(
    path: PathBuf,
    target: NormalizedTrack,
    quality_policy: QualityPolicy,
) -> Result<ValidationReport, ValidationError> {
    tokio::task::spawn_blocking(move || validate_candidate_blocking(&path, &target, &quality_policy))
        .await
        .map_err(|error| ValidationError::Rejected {
            message: error.to_string(),
        })?
}

fn validate_candidate_blocking(
    path: &Path,
    target: &NormalizedTrack,
    quality_policy: &QualityPolicy,
) -> Result<ValidationReport, ValidationError> {
    let metadata = std::fs::metadata(path).map_err(|error| ValidationError::Rejected {
        message: error.to_string(),
    })?;
    let file_size = metadata.len();
    if file_size == 0 {
        return Err(ValidationError::EmptyFile);
    }

    let mut sniff = [0_u8; 256];
    let mut file = File::open(path).map_err(|error| ValidationError::Rejected {
        message: error.to_string(),
    })?;
    let read = file.read(&mut sniff).map_err(|error| ValidationError::Rejected {
        message: error.to_string(),
    })?;
    let head = String::from_utf8_lossy(&sniff[..read]).to_ascii_lowercase();
    if head.contains("<html") || head.contains("<!doctype html") {
        return Err(ValidationError::HtmlPayload);
    }
    if file_size < 1024 {
        return Err(ValidationError::Rejected {
            message: format!("candidate file too small: {file_size} bytes"),
        });
    }

    let ext = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    let mut hint = Hint::new();
    if !ext.is_empty() {
        hint.with_extension(&ext);
    }

    let symphonia_file = File::open(path).map_err(|error| ValidationError::Rejected {
        message: error.to_string(),
    })?;
    let source = MediaSourceStream::new(Box::new(symphonia_file), Default::default());
    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            source,
            &Default::default(),
            &Default::default(),
        )
        .map_err(|error| ValidationError::UnreadableContainer {
            message: error.to_string(),
        })?;

    let track = probed
        .format
        .default_track()
        .ok_or_else(|| ValidationError::UnreadableContainer {
            message: "no default audio track".to_string(),
        })?;

    let n_frames = track.codec_params.n_frames.unwrap_or_default();
    let sample_rate = track.codec_params.sample_rate.unwrap_or_default() as f64;
    let duration_secs = if n_frames > 0 && sample_rate > 0.0 {
        Some(n_frames as f64 / sample_rate)
    } else {
        None
    };

    let signature_format = detect_signature_format(&sniff[..read]);
    let extension_ok = signature_format
        .as_ref()
        .map(|format| format == &ext)
        .unwrap_or(true);
    if !extension_ok {
        return Err(ValidationError::ExtensionMismatch {
            expected: ext.clone(),
            actual: signature_format.unwrap_or_else(|| "unknown".to_string()),
        });
    }

    let format_name = if ext.is_empty() { signature_format } else { Some(ext.clone()) };
    let mut issues = Vec::<ValidationIssue>::new();
    if let Some(duration_secs) = duration_secs {
        if duration_secs <= 0.0 || duration_secs < quality_policy.minimum_duration_secs {
            issues.push(ValidationIssue {
                code: "duration_too_short".to_string(),
                message: format!("candidate duration {duration_secs:.2}s is too short"),
            });
        }

        if let (Some(expected), Some(max_delta)) =
            (target.duration_secs, quality_policy.max_duration_delta_secs)
        {
            let delta = (duration_secs - expected).abs();
            if delta > max_delta {
                issues.push(ValidationIssue {
                    code: "duration_mismatch".to_string(),
                    message: format!("candidate duration delta {delta:.2}s exceeds policy"),
                });
            }
        }
    } else {
        issues.push(ValidationIssue {
            code: "duration_unknown".to_string(),
            message: "decoder could not estimate duration".to_string(),
        });
    }

    let quality = match ext.as_str() {
        "flac" | "wav" | "aiff" | "wv" | "ape" => CandidateQuality::Lossless,
        "mp3" | "aac" | "m4a" | "ogg" | "opus" => CandidateQuality::Lossy,
        _ => CandidateQuality::Unknown,
    };

    let report = ValidationReport {
        is_valid: issues.is_empty(),
        format_name,
        duration_secs,
        audio_readable: true,
        header_readable: true,
        extension_ok,
        file_size,
        quality,
        issues,
    };

    if report.is_valid {
        Ok(report)
    } else {
        Err(ValidationError::ImplausibleDuration {
            message: report
                .issues
                .iter()
                .map(|issue| issue.message.clone())
                .collect::<Vec<_>>()
                .join("; "),
        })
    }
}

fn detect_signature_format(bytes: &[u8]) -> Option<String> {
    if bytes.starts_with(b"fLaC") {
        return Some("flac".to_string());
    }
    if bytes.starts_with(b"ID3") {
        return Some("mp3".to_string());
    }
    if bytes.starts_with(b"OggS") {
        // Opus uses the Ogg container but has "OpusHead" at byte 28
        if bytes.len() >= 36 && &bytes[28..36] == b"OpusHead" {
            return Some("opus".to_string());
        }
        return Some("ogg".to_string());
    }
    if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WAVE" {
        return Some("wav".to_string());
    }
    if bytes.len() >= 8 && &bytes[4..8] == b"ftyp" {
        return Some("m4a".to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn target() -> NormalizedTrack {
        NormalizedTrack {
            spotify_track_id: None,
            source_playlist: None,
            artist: "Artist".to_string(),
            album_artist: None,
            title: "Song".to_string(),
            album: None,
            track_number: None,
            disc_number: None,
            year: None,
            duration_secs: Some(180.0),
            isrc: None,
        }
    }

    #[tokio::test]
    async fn validation_rejects_html_payload() {
        let dir = tempdir().expect("temp dir");
        let path = dir.path().join("fake.mp3");
        std::fs::write(&path, "<html>bad gateway</html>").expect("write html");

        let result = validate_candidate(path, target(), QualityPolicy::default()).await;
        assert!(matches!(result, Err(ValidationError::HtmlPayload)));
    }
}
