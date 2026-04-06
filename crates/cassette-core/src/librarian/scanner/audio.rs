use crate::librarian::config::QualityConfig;
use crate::librarian::error::{LibrarianError, Result};
use crate::librarian::models::{IntegrityStatus, NewLocalFile, QualityTier};
use crate::librarian::normalize::{
    album::normalize_album_title, artist::normalize_artist_name, track::normalize_track_title,
};
use crate::librarian::scanner::integrity::assess_integrity;
use lofty::file::AudioFile;
use lofty::prelude::{Accessor, TaggedFileExt};
use lofty::probe::Probe;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct ParsedAudioFacts {
    pub artist: Option<String>,
    pub normalized_artist: Option<String>,
    pub album: Option<String>,
    pub normalized_album: Option<String>,
    pub title: Option<String>,
    pub normalized_title: Option<String>,
    pub track_number: Option<i64>,
    pub disc_number: Option<i64>,
    pub duration_ms: Option<i64>,
    pub isrc: Option<String>,
    pub codec: Option<String>,
    pub bitrate: Option<i64>,
    pub sample_rate: Option<i64>,
    pub bit_depth: Option<i64>,
    pub channels: Option<i64>,
    pub integrity_status: IntegrityStatus,
    pub quality_tier: Option<QualityTier>,
}

pub fn parse_audio_file(path: &Path, quality: &QualityConfig) -> Result<ParsedAudioFacts> {
    let tagged = match Probe::open(path)
        .map_err(|e| LibrarianError::ParseError(e.to_string()))?
        .read()
    {
        Ok(tagged) => tagged,
        Err(_) => {
            return Ok(ParsedAudioFacts {
                artist: None,
                normalized_artist: None,
                album: None,
                normalized_album: None,
                title: None,
                normalized_title: None,
                track_number: None,
                disc_number: None,
                duration_ms: None,
                isrc: None,
                codec: None,
                bitrate: None,
                sample_rate: None,
                bit_depth: None,
                channels: None,
                integrity_status: IntegrityStatus::Unreadable,
                quality_tier: None,
            });
        }
    };

    let props = tagged.properties();
    let duration_ms = i64::try_from(props.duration().as_millis()).ok();
    let bitrate = props.overall_bitrate().map(i64::from);
    let sample_rate = props.sample_rate().map(i64::from);
    let bit_depth = props.bit_depth().map(i64::from);
    let channels = props.channels().map(i64::from);

    let tag = tagged.primary_tag().or_else(|| tagged.first_tag());
    let artist = tag.and_then(|t| t.artist()).map(|v| v.to_string());
    let album = tag.and_then(|t| t.album()).map(|v| v.to_string());
    let title = tag.and_then(|t| t.title()).map(|v| v.to_string());
    let track_number = tag.and_then(|t| t.track()).map(i64::from);
    let disc_number = tag.and_then(|t| t.disk()).map(i64::from);
    let isrc = tag
        .and_then(|t| t.get_string(&lofty::tag::ItemKey::Isrc))
        .map(ToString::to_string)
        .filter(|v| !v.trim().is_empty());

    let extension = path
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let suspicious =
        matches!(extension.as_str(), "mp3") && matches!(bit_depth, Some(depth) if depth > 0);
    let has_required_metadata = artist.as_deref().is_some_and(|v| !v.trim().is_empty())
        && title.as_deref().is_some_and(|v| !v.trim().is_empty());
    let integrity_status =
        assess_integrity(duration_ms.is_some(), has_required_metadata, suspicious);

    let quality_tier = quality_tier_for(&extension, bitrate, quality);

    Ok(ParsedAudioFacts {
        artist: artist.clone(),
        normalized_artist: artist.as_deref().map(normalize_artist_name),
        album: album.clone(),
        normalized_album: album.as_deref().map(normalize_album_title),
        title: title.clone(),
        normalized_title: title.as_deref().map(normalize_track_title),
        track_number,
        disc_number,
        duration_ms,
        isrc,
        codec: Some(extension),
        bitrate,
        sample_rate,
        bit_depth,
        channels,
        integrity_status,
        quality_tier,
    })
}

pub fn quality_tier_for(
    extension: &str,
    bitrate: Option<i64>,
    quality: &QualityConfig,
) -> Option<QualityTier> {
    let ext = extension.to_ascii_lowercase();
    if matches!(
        ext.as_str(),
        "flac" | "wav" | "alac" | "aiff" | "dsf" | "dff"
    ) {
        return Some(QualityTier::LosslessPreferred);
    }

    if matches!(ext.as_str(), "mp3" | "aac" | "m4a" | "opus" | "ogg") {
        let kbps = bitrate.unwrap_or(0);
        if kbps >= i64::from(quality.preferred_lossy_floor_kbps) {
            return Some(QualityTier::LossyAcceptable);
        }
        if kbps < i64::from(quality.absolute_lossy_floor_kbps) {
            return Some(QualityTier::UpgradeCandidate);
        }
        return Some(QualityTier::LossyAcceptable);
    }

    None
}

pub fn to_new_local_file(
    path: &Path,
    file_size: i64,
    file_mtime_ms: Option<i64>,
    facts: ParsedAudioFacts,
    hash: Option<String>,
) -> NewLocalFile {
    let file_name = path
        .file_name()
        .and_then(|v| v.to_str())
        .unwrap_or_default()
        .to_string();
    let extension = path
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    NewLocalFile {
        track_id: None,
        file_path: path.to_string_lossy().to_string(),
        file_name,
        extension,
        codec: facts.codec,
        bitrate: facts.bitrate,
        sample_rate: facts.sample_rate,
        bit_depth: facts.bit_depth,
        channels: facts.channels,
        duration_ms: facts.duration_ms,
        file_size,
        file_mtime_ms,
        content_hash: hash,
        acoustid_fingerprint: None,
        integrity_status: facts.integrity_status,
        quality_tier: facts.quality_tier,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn low_bitrate_is_upgrade_candidate() {
        let tier = quality_tier_for(
            "mp3",
            Some(96),
            &QualityConfig {
                preferred_lossy_floor_kbps: 320,
                absolute_lossy_floor_kbps: 128,
            },
        );
        assert_eq!(tier, Some(QualityTier::UpgradeCandidate));
    }

    #[test]
    fn random_file_is_unreadable() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("bad.mp3");
        std::fs::write(&path, b"not-audio").expect("write");

        let parsed = parse_audio_file(&path, &QualityConfig::default()).expect("parse");
        assert_eq!(parsed.integrity_status, IntegrityStatus::Unreadable);
    }
}
