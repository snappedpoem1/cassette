use crate::director::config::QualityPolicy;
use crate::director::models::{
    CandidateQuality, CandidateScore, NormalizedTrack, ProviderDescriptor, ProviderSearchCandidate,
    SelectionReason, ValidationReport,
};
use std::collections::BTreeMap;

pub fn score_candidate(
    target: &NormalizedTrack,
    provider: &ProviderDescriptor,
    candidate: &ProviderSearchCandidate,
    validation: &ValidationReport,
    quality_policy: &QualityPolicy,
) -> (CandidateScore, SelectionReason) {
    let metadata_match_points = (candidate.metadata_confidence.clamp(0.0, 1.0) * 40.0) as i32;

    let duration_points = match (target.duration_secs, validation.duration_secs) {
        (Some(expected), Some(actual)) => {
            let delta = (expected - actual).abs();
            if delta <= 1.5 {
                25
            } else if quality_policy
                .max_duration_delta_secs
                .is_some_and(|max_delta| delta <= max_delta)
            {
                10
            } else {
                -25
            }
        }
        _ => 0,
    };

    let codec_points = match validation.quality {
        CandidateQuality::Lossless => 20,
        CandidateQuality::Lossy => 5,
        CandidateQuality::Unknown => 0,
    };

    let provider_points = (20 - provider.trust_rank).max(0);
    let validation_points = if validation.is_valid { 20 } else { -50 };
    let size_points = if validation.file_size > 5 * 1024 * 1024 {
        5
    } else {
        0
    };

    // Bitrate bonus: reward higher bitrate candidates (0-10 points)
    let bitrate_points = match candidate.bitrate_kbps {
        Some(kbps) if kbps >= 900 => 10, // lossless-tier (FLAC ~900+)
        Some(kbps) if kbps >= 320 => 7,  // high lossy (320 MP3/AAC)
        Some(kbps) if kbps >= 256 => 4,  // medium lossy
        Some(kbps) if kbps >= 128 => 2,  // acceptable lossy
        _ => 0,
    };

    // Format preference bonus: reward preferred extensions (0-5 points)
    let format_points = candidate.extension_hint.as_deref().map_or(0, |ext| {
        let ext_lower = ext.to_ascii_lowercase();
        if quality_policy
            .preferred_extensions
            .iter()
            .any(|pref| pref == &ext_lower)
        {
            5
        } else {
            0
        }
    });

    let score = CandidateScore {
        total: metadata_match_points
            + duration_points
            + codec_points
            + provider_points
            + validation_points
            + size_points
            + bitrate_points
            + format_points,
        metadata_match_points,
        duration_points,
        codec_points,
        provider_points,
        validation_points,
        size_points,
        bitrate_points,
        format_points,
    };

    let mut details = BTreeMap::<String, String>::new();
    details.insert(
        "metadata_confidence".to_string(),
        format!("{:.2}", candidate.metadata_confidence),
    );
    details.insert("provider".to_string(), provider.id.clone());
    details.insert("quality".to_string(), format!("{:?}", validation.quality));
    details.insert("file_size".to_string(), validation.file_size.to_string());
    details.insert("score_total".to_string(), score.total.to_string());

    let reason = SelectionReason {
        summary: format!(
            "Selected {} via {} with score {}",
            candidate.title, provider.display_name, score.total
        ),
        details,
    };

    (score, reason)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::director::models::{ProviderCapabilities, ValidationIssue};

    #[test]
    fn scoring_prefers_lossless_high_confidence_candidates() {
        let target = NormalizedTrack {
            spotify_track_id: None,
            source_album_id: None,
            source_artist_id: None,
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
            musicbrainz_recording_id: None,
            musicbrainz_release_group_id: None,
            musicbrainz_release_id: None,
            canonical_artist_id: None,
            canonical_release_id: None,
        };
        let provider = ProviderDescriptor {
            id: "provider-a".to_string(),
            display_name: "Provider A".to_string(),
            trust_rank: 1,
            capabilities: ProviderCapabilities::default(),
        };
        let candidate = ProviderSearchCandidate {
            provider_id: provider.id.clone(),
            provider_candidate_id: "cand-1".to_string(),
            artist: "Artist".to_string(),
            title: "Song".to_string(),
            album: None,
            duration_secs: Some(180.0),
            extension_hint: Some("flac".to_string()),
            bitrate_kbps: Some(1000),
            cover_art_url: None,
            metadata_confidence: 0.98,
        };
        let validation = ValidationReport {
            is_valid: true,
            format_name: Some("flac".to_string()),
            duration_secs: Some(180.1),
            audio_readable: true,
            header_readable: true,
            extension_ok: true,
            file_size: 30 * 1024 * 1024,
            quality: CandidateQuality::Lossless,
            issues: Vec::<ValidationIssue>::new(),
        };

        let (score, _) = score_candidate(
            &target,
            &provider,
            &candidate,
            &validation,
            &QualityPolicy::default(),
        );
        assert!(score.total >= 100);
    }
}
