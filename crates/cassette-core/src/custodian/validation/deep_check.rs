use crate::custodian::validation::heuristics::plausible_size_for_duration;
use crate::custodian::validation::symphonia_probe::probe_audio;
use chrono::{DateTime, Utc};
use lofty::prelude::{Accessor, AudioFile, TaggedFileExt};
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ValidationStatus {
    Valid,
    SuspiciousSmallForDuration,
    SuspiciousDurationMismatch,
    UnreadableContainer,
    DecodeFailed,
    ZeroByte,
    MissingOnDisk,
    UnsupportedFormat,
    MetadataOnlyNoAudioProof,
    ProbableTruncation,
    HtmlOrTextPayload,
    IncompleteMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationReport {
    pub file_path: PathBuf,
    pub status: ValidationStatus,
    pub codec: Option<String>,
    pub bitrate: Option<u32>,
    pub sample_rate: Option<u32>,
    pub bit_depth: Option<u8>,
    pub channels: Option<u8>,
    pub duration_ms: Option<u64>,
    pub file_size: u64,
    pub content_hash: Option<String>,
    pub reasons: Vec<String>,
    pub is_duplicate_of: Option<PathBuf>,
    pub validation_timestamp: DateTime<Utc>,
}

fn infer_format_from_header(header: &[u8]) -> Option<&'static str> {
    if header.len() >= 4 && &header[..4] == b"fLaC" {
        return Some("flac");
    }

    if header.len() >= 3 && &header[..3] == b"ID3" {
        return Some("mp3");
    }

    if header.len() >= 2 && header[0] == 0xFF && (header[1] & 0xE0) == 0xE0 {
        return Some("mp3");
    }

    if header.len() >= 4 && &header[..4] == b"OggS" {
        return Some("ogg");
    }

    if header.len() >= 12 && &header[4..8] == b"ftyp" {
        return Some("m4a");
    }

    if header.len() >= 12 && &header[..4] == b"RIFF" && &header[8..12] == b"WAVE" {
        return Some("wav");
    }

    None
}

pub fn deep_validate_audio(
    path: &Path,
    allowed_formats: &[String],
    suspicious_size_tolerance: f64,
    with_hash: bool,
) -> ValidationReport {
    let now = Utc::now();
    let mut reasons = Vec::<String>::new();

    if !path.exists() {
        return ValidationReport {
            file_path: path.to_path_buf(),
            status: ValidationStatus::MissingOnDisk,
            codec: None,
            bitrate: None,
            sample_rate: None,
            bit_depth: None,
            channels: None,
            duration_ms: None,
            file_size: 0,
            content_hash: None,
            reasons: vec!["file missing on disk".to_string()],
            is_duplicate_of: None,
            validation_timestamp: now,
        };
    }

    let metadata = match std::fs::metadata(path) {
        Ok(value) => value,
        Err(error) => {
            return ValidationReport {
                file_path: path.to_path_buf(),
                status: ValidationStatus::UnreadableContainer,
                codec: None,
                bitrate: None,
                sample_rate: None,
                bit_depth: None,
                channels: None,
                duration_ms: None,
                file_size: 0,
                content_hash: None,
                reasons: vec![format!("failed to read metadata: {error}")],
                is_duplicate_of: None,
                validation_timestamp: now,
            };
        }
    };

    let file_size = metadata.len();
    if file_size == 0 {
        return ValidationReport {
            file_path: path.to_path_buf(),
            status: ValidationStatus::ZeroByte,
            codec: None,
            bitrate: None,
            sample_rate: None,
            bit_depth: None,
            channels: None,
            duration_ms: None,
            file_size,
            content_hash: None,
            reasons: vec!["file is zero bytes".to_string()],
            is_duplicate_of: None,
            validation_timestamp: now,
        };
    }

    let ext = path
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let mut sniff = [0_u8; 256];
    let mut effective_ext = ext.clone();
    if let Ok(mut file) = std::fs::File::open(path) {
        if let Ok(read) = file.read(&mut sniff) {
            if !allowed_formats.iter().any(|v| v.eq_ignore_ascii_case(&effective_ext)) {
                if let Some(inferred) = infer_format_from_header(&sniff[..read]) {
                    effective_ext = inferred.to_string();
                    reasons.push(format!(
                        "recovered format from file signature despite extension {ext}"
                    ));
                } else {
                    return ValidationReport {
                        file_path: path.to_path_buf(),
                        status: ValidationStatus::UnsupportedFormat,
                        codec: Some(ext),
                        bitrate: None,
                        sample_rate: None,
                        bit_depth: None,
                        channels: None,
                        duration_ms: None,
                        file_size,
                        content_hash: None,
                        reasons: vec!["unsupported extension".to_string()],
                        is_duplicate_of: None,
                        validation_timestamp: now,
                    };
                }
            }

            let head = String::from_utf8_lossy(&sniff[..read]).to_ascii_lowercase();
            if head.contains("<html") || head.contains("<!doctype html") {
                return ValidationReport {
                    file_path: path.to_path_buf(),
                    status: ValidationStatus::HtmlOrTextPayload,
                    codec: Some(effective_ext),
                    bitrate: None,
                    sample_rate: None,
                    bit_depth: None,
                    channels: None,
                    duration_ms: None,
                    file_size,
                    content_hash: None,
                    reasons: vec!["detected html/text payload".to_string()],
                    is_duplicate_of: None,
                    validation_timestamp: now,
                };
            }
        }
    }

    let lofty = lofty::probe::Probe::open(path).and_then(|p| p.read());
    let (bitrate, bit_depth, metadata_missing) = match lofty {
        Ok(tagged) => {
            let tag = tagged.primary_tag().or_else(|| tagged.first_tag());
            let artist_ok = tag.and_then(|t| t.artist()).is_some();
            let album_ok = tag.and_then(|t| t.album()).is_some();
            let title_ok = tag.and_then(|t| t.title()).is_some();
            (
                tagged.properties().overall_bitrate(),
                tagged.properties().bit_depth().map(|v| v as u8),
                !(artist_ok && album_ok && title_ok),
            )
        }
        Err(_) => (None, None, true),
    };

    let probe = match probe_audio(path) {
        Ok(p) => p,
        Err(error) => {
            return ValidationReport {
                file_path: path.to_path_buf(),
                status: ValidationStatus::UnreadableContainer,
                codec: Some(effective_ext),
                bitrate,
                sample_rate: None,
                bit_depth,
                channels: None,
                duration_ms: None,
                file_size,
                content_hash: None,
                reasons: vec![format!("symphonia probe failed: {error}")],
                is_duplicate_of: None,
                validation_timestamp: now,
            };
        }
    };

    if !probe.decode_ok {
        return ValidationReport {
            file_path: path.to_path_buf(),
            status: ValidationStatus::DecodeFailed,
            codec: probe.codec,
            bitrate,
            sample_rate: probe.sample_rate,
            bit_depth,
            channels: probe.channels,
            duration_ms: probe.duration_ms,
            file_size,
            content_hash: None,
            reasons: vec!["first-frame decode failed".to_string()],
            is_duplicate_of: None,
            validation_timestamp: now,
        };
    }

    let size_plausible = plausible_size_for_duration(
        probe.duration_ms,
        bitrate,
        file_size,
        suspicious_size_tolerance,
    );
    if !size_plausible {
        reasons.push("file size appears implausible for duration/bitrate".to_string());
    }

    if let Some(duration_ms) = probe.duration_ms {
        if duration_ms == 0 {
            reasons.push("duration resolved to zero".to_string());
        }
        if duration_ms > 0 && file_size < 1024 {
            reasons.push("probable truncation due to tiny payload".to_string());
        }
    }

    if metadata_missing {
        reasons.push("incomplete metadata fields".to_string());
    }

    let content_hash = if with_hash {
        std::fs::read(path)
            .ok()
            .map(|bytes| blake3::hash(&bytes).to_hex().to_string())
    } else {
        None
    };

    let status = if reasons.iter().any(|r| r.contains("probable truncation")) {
        ValidationStatus::ProbableTruncation
    } else if reasons.iter().any(|r| r.contains("implausible")) {
        ValidationStatus::SuspiciousSmallForDuration
    } else if metadata_missing {
        ValidationStatus::IncompleteMetadata
    } else {
        ValidationStatus::Valid
    };

    ValidationReport {
        file_path: path.to_path_buf(),
        status,
        codec: probe.codec,
        bitrate,
        sample_rate: probe.sample_rate,
        bit_depth,
        channels: probe.channels,
        duration_ms: probe.duration_ms,
        file_size,
        content_hash,
        reasons,
        is_duplicate_of: None,
        validation_timestamp: now,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_is_reported() {
        let report = deep_validate_audio(
            Path::new("Z:/missing/file.flac"),
            &["flac".to_string()],
            1.5,
            false,
        );
        assert_eq!(report.status, ValidationStatus::MissingOnDisk);
    }

    #[test]
    fn zero_byte_is_reported() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("zero.mp3");
        std::fs::write(&file, []).expect("write");
        let report = deep_validate_audio(&file, &["mp3".to_string()], 1.5, false);
        assert_eq!(report.status, ValidationStatus::ZeroByte);
    }
}
