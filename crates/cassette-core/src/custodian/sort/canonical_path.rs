use crate::custodian::sort::sanitize::sanitize_component;
use crate::custodian::validation::ValidationReport;
use lofty::prelude::{Accessor, TaggedFileExt};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct CanonicalMetadata {
    pub artist: Option<String>,
    pub album: Option<String>,
    pub title: Option<String>,
    pub track_number: Option<u32>,
    pub extension: String,
}

pub fn build_canonical_path(root: &Path, metadata: &CanonicalMetadata) -> PathBuf {
    let artist = sanitize_component(metadata.artist.as_deref().unwrap_or("Unknown Artist"));
    let album = sanitize_component(metadata.album.as_deref().unwrap_or("Unknown Album"));
    let title = sanitize_component(metadata.title.as_deref().unwrap_or("Untitled"));
    let track = metadata
        .track_number
        .map(|n| format!("{n:02}"))
        .unwrap_or_else(|| "00".to_string());
    let ext = sanitize_component(&metadata.extension.to_ascii_lowercase());

    root.join(artist)
        .join(album)
        .join(format!("{track} - {title}.{ext}"))
}

pub fn canonical_metadata_from_report(path: &Path, report: &ValidationReport) -> CanonicalMetadata {
    let extension = path
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or("bin")
        .to_string();

    let lofty = lofty::probe::Probe::open(path).and_then(|p| p.read()).ok();
    let tag = lofty
        .as_ref()
        .and_then(|f| f.primary_tag().or_else(|| f.first_tag()));

    CanonicalMetadata {
        artist: tag.and_then(|t| t.artist()).map(|v| v.to_string()),
        album: tag.and_then(|t| t.album()).map(|v| v.to_string()),
        title: tag.and_then(|t| t.title()).map(|v| v.to_string()),
        track_number: tag.and_then(|t| t.track()),
        extension: report
            .codec
            .clone()
            .unwrap_or(extension)
            .to_ascii_lowercase(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_path_has_fallbacks() {
        let path = build_canonical_path(
            Path::new("A:/music_sorted"),
            &CanonicalMetadata {
                artist: None,
                album: None,
                title: None,
                track_number: None,
                extension: "mp3".to_string(),
            },
        );
        let text = path.to_string_lossy();
        assert!(text.contains("Unknown Artist"));
        assert!(text.contains("Unknown Album"));
        assert!(text.contains("00 - Untitled.mp3"));
    }
}
