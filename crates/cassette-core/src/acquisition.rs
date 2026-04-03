use crate::director::models::{
    AcquisitionStrategy, NormalizedTrack, TrackTask, TrackTaskSource,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AcquisitionScope {
    Track,
    Album,
    Artist,
    Discography,
    SelectedAlbums,
}

impl AcquisitionScope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Track => "track",
            Self::Album => "album",
            Self::Artist => "artist",
            Self::Discography => "discography",
            Self::SelectedAlbums => "selected_albums",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConfirmationPolicy {
    Automatic,
    Advisory,
    ManualReview,
}

impl ConfirmationPolicy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Automatic => "automatic",
            Self::Advisory => "advisory",
            Self::ManualReview => "manual_review",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AcquisitionRequestStatus {
    Pending,
    Queued,
    Submitted,
    InProgress,
    Reviewing,
    Finalized,
    AlreadyPresent,
    Failed,
    Cancelled,
}

impl AcquisitionRequestStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Queued => "queued",
            Self::Submitted => "submitted",
            Self::InProgress => "in_progress",
            Self::Reviewing => "reviewing",
            Self::Finalized => "finalized",
            Self::AlreadyPresent => "already_present",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcquisitionRequest {
    pub id: Option<i64>,
    pub scope: AcquisitionScope,
    pub source: TrackTaskSource,
    pub source_name: String,
    pub source_track_id: Option<String>,
    pub source_album_id: Option<String>,
    pub source_artist_id: Option<String>,
    pub artist: String,
    pub album: Option<String>,
    pub title: String,
    pub track_number: Option<u32>,
    pub disc_number: Option<u32>,
    pub year: Option<i32>,
    pub duration_secs: Option<f64>,
    pub isrc: Option<String>,
    pub musicbrainz_recording_id: Option<String>,
    pub musicbrainz_release_id: Option<String>,
    pub canonical_artist_id: Option<i64>,
    pub canonical_release_id: Option<i64>,
    pub strategy: AcquisitionStrategy,
    pub quality_policy: Option<String>,
    pub excluded_providers: Vec<String>,
    pub edition_policy: Option<String>,
    pub confirmation_policy: ConfirmationPolicy,
    pub desired_track_id: Option<i64>,
    pub source_operation_id: Option<String>,
    pub task_id: Option<String>,
    pub request_signature: Option<String>,
    pub status: AcquisitionRequestStatus,
    pub raw_payload_json: Option<String>,
}

impl AcquisitionRequest {
    pub fn effective_task_id(&self) -> String {
        self.task_id
            .clone()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| format!("request::{}", normalize_component(&self.request_fingerprint())))
    }

    pub fn request_fingerprint(&self) -> String {
        let excluded = self
            .excluded_providers
            .iter()
            .map(|value| normalize_component(value))
            .collect::<Vec<_>>()
            .join(",");

        format!(
            "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
            self.scope.as_str(),
            normalize_component(self.source_track_id.as_deref().unwrap_or_default()),
            normalize_component(self.source_album_id.as_deref().unwrap_or_default()),
            normalize_component(self.source_artist_id.as_deref().unwrap_or_default()),
            normalize_component(&self.artist),
            normalize_component(self.album.as_deref().unwrap_or_default()),
            normalize_component(&self.title),
            self.track_number.unwrap_or_default(),
            self.disc_number.unwrap_or_default(),
            self.year.unwrap_or_default(),
            self.duration_secs.unwrap_or_default(),
            normalize_component(self.isrc.as_deref().unwrap_or_default()),
            normalize_component(
                self.musicbrainz_recording_id
                    .as_deref()
                    .unwrap_or_default(),
            ),
            normalize_component(
                self.musicbrainz_release_id
                    .as_deref()
                    .unwrap_or_default(),
            ),
            self.canonical_artist_id.unwrap_or_default(),
            self.canonical_release_id.unwrap_or_default(),
            self.strategy_name(),
            normalize_component(self.quality_policy.as_deref().unwrap_or_default()),
            excluded,
            normalize_component(self.edition_policy.as_deref().unwrap_or_default()),
            self.confirmation_policy.as_str(),
        )
    }

    pub fn strategy_name(&self) -> &'static str {
        match self.strategy {
            AcquisitionStrategy::Standard => "standard",
            AcquisitionStrategy::HighQualityOnly => "high_quality_only",
            AcquisitionStrategy::ObscureFallbackHeavy => "obscure_fallback_heavy",
            AcquisitionStrategy::DiscographyBatch => "discography_batch",
            AcquisitionStrategy::SingleTrackPriority => "single_track_priority",
            AcquisitionStrategy::MetadataRepairOnly => "metadata_repair_only",
            AcquisitionStrategy::RedownloadReplaceIfBetter => "redownload_replace_if_better",
        }
    }

    pub fn to_track_task(&self) -> TrackTask {
        TrackTask {
            task_id: self.effective_task_id(),
            source: self.source.clone(),
            desired_track_id: self.desired_track_id,
            source_operation_id: self.source_operation_id.clone(),
            target: NormalizedTrack {
                spotify_track_id: self.source_track_id.clone(),
                source_album_id: self.source_album_id.clone(),
                source_artist_id: self.source_artist_id.clone(),
                source_playlist: None,
                artist: self.artist.clone(),
                album_artist: Some(self.artist.clone()),
                title: self.title.clone(),
                album: self.album.clone(),
                track_number: self.track_number,
                disc_number: self.disc_number,
                year: self.year,
                duration_secs: self.duration_secs,
                isrc: self.isrc.clone(),
                musicbrainz_recording_id: self.musicbrainz_recording_id.clone(),
                musicbrainz_release_id: self.musicbrainz_release_id.clone(),
                canonical_artist_id: self.canonical_artist_id,
                canonical_release_id: self.canonical_release_id,
            },
            strategy: self.strategy,
        }
    }
}

fn normalize_component(value: &str) -> String {
    value
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_request() -> AcquisitionRequest {
        AcquisitionRequest {
            id: None,
            scope: AcquisitionScope::Track,
            source: TrackTaskSource::Manual,
            source_name: "manual".to_string(),
            source_track_id: Some("spotify:track:1".to_string()),
            source_album_id: None,
            source_artist_id: None,
            artist: "Artist".to_string(),
            album: Some("Album".to_string()),
            title: "Song".to_string(),
            track_number: Some(1),
            disc_number: Some(1),
            year: Some(2024),
            duration_secs: Some(42.0),
            isrc: Some("US1234567890".to_string()),
            musicbrainz_recording_id: Some("mb-recording".to_string()),
            musicbrainz_release_id: Some("mb-release".to_string()),
            canonical_artist_id: Some(7),
            canonical_release_id: Some(9),
            strategy: AcquisitionStrategy::Standard,
            quality_policy: Some("lossless_preferred".to_string()),
            excluded_providers: vec!["yt_dlp".to_string(), "usenet".to_string()],
            edition_policy: Some("standard_only".to_string()),
            confirmation_policy: ConfirmationPolicy::Automatic,
            desired_track_id: Some(11),
            source_operation_id: Some("op-1".to_string()),
            task_id: Some("task-1".to_string()),
            request_signature: None,
            status: AcquisitionRequestStatus::Pending,
            raw_payload_json: None,
        }
    }

    #[test]
    fn request_fingerprint_is_deterministic() {
        let request = sample_request();
        assert_eq!(request.request_fingerprint(), request.request_fingerprint());
    }

    #[test]
    fn request_translates_to_track_task() {
        let request = sample_request();
        let task = request.to_track_task();
        assert_eq!(task.task_id, "task-1");
        assert_eq!(task.target.artist, "Artist");
        assert!(task.target.source_album_id.is_none());
        assert_eq!(task.target.musicbrainz_release_id.as_deref(), Some("mb-release"));
        assert_eq!(task.desired_track_id, Some(11));
        assert_eq!(task.source_operation_id.as_deref(), Some("op-1"));
    }
}
